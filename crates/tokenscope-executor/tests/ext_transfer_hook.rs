//! Transfer hooks against the real Token-2022 program and the demo hook program
//! (`programs/demo-hook`, built by `scripts/build-programs.sh`).
//!
//! Hook semantics checked against the Solana docs ("Transfer Hook Integration Guide"): a hook is not
//! optional at the account level; every transfer needs the hook program, the `ExtraAccountMetaList`
//! PDA and the extra accounts; missing accounts fail on-chain with no fallback; the token program
//! sets `TransferHookAccount.transferring` only during its CPI so direct `Execute` calls are
//! rejected by a well-written hook.

mod common;

use common::*;
use solana_instruction::AccountMeta;
use spl_token_2022_interface::{extension::pausable, instruction as ti};
use tokenscope_executor::{
    ExecReport, HookFixture, MintExt, MintPlan, demo_hook_program_id, derive_address,
};

const HOOK_REJECTED: u32 = 100;
const HOOK_NOT_TRANSFERRING: u32 = 101;
/// Observed code when an extra account required by the validation list is absent or replaced
/// (an account-resolution error from the interface crates; the runtime logs "Error: Unknown").
const ACCOUNT_RESOLUTION: u32 = 0xa261_c2c0;

fn hook_elf() -> Vec<u8> {
    let dir = std::env::var("TOKENSCOPE_PROGRAM_DIR").unwrap_or_else(|_| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/sbf-programs").into()
    });
    std::fs::read(format!("{dir}/demo_hook.so"))
        .unwrap_or_else(|e| panic!("{dir}/demo_hook.so: {e} (run scripts/build-programs.sh)"))
}

struct Hooked {
    e: Env,
    hook: HookFixture,
}

fn hooked(extra: Vec<MintExt>) -> Hooked {
    let mut exts = vec![MintExt::TransferHook {
        program_id: demo_hook_program_id(),
    }];
    exts.extend(extra);
    let mut e = env(&MintPlan::new(6, exts));
    e.h.mint_to(&e.mint, &e.a, 1_000).unwrap();
    let hook = e.h.install_demo_hook(&hook_elf(), &e.mint).unwrap();
    Hooked { e, hook }
}

impl Hooked {
    /// A transfer carrying the listed extra accounts (0 = counter, 1 = validation PDA, 2 = program).
    fn transfer(&self, amount: u64, keep: &[usize]) -> ExecReport {
        let mut ix = self
            .e
            .h
            .transfer_checked_ix(&self.e.a, &self.e.mint, &self.e.b, &self.e.alice, amount, 6)
            .unwrap();
        let extras = self.e.h.hook_extra_accounts(&self.hook);
        ix.accounts.extend(keep.iter().map(|&i| extras[i].clone()));
        self.e.h.execute_logged(&ix)
    }

    fn state(&self) -> (u64, u64, u64) {
        (
            self.e.amount(&self.e.a),
            self.e.amount(&self.e.b),
            self.e.h.hook_counter(&self.hook).unwrap().invocations,
        )
    }
}

#[test]
fn transfer_with_all_required_accounts_invokes_the_hook() {
    let t = hooked(vec![]);
    assert_eq!(t.transfer(10, &[0, 1, 2]).error, None);
    let c = t.e.h.hook_counter(&t.hook).unwrap();
    assert_eq!((c.invocations, c.last_amount, c.reject), (1, 10, false));
    assert_eq!((t.e.amount(&t.e.a), t.e.amount(&t.e.b)), (990, 10));
    assert_eq!(t.transfer(25, &[0, 1, 2]).error, None);
    assert_eq!(t.state(), (965, 35, 2));
}

#[test]
fn the_hook_is_not_optional_a_plain_transfer_fails() {
    let t = hooked(vec![]);
    let before = t.state();
    let rep = t.transfer(10, &[]);
    assert_eq!(
        rep.error.as_deref(),
        Some("Failure(NotEnoughAccountKeys)"),
        "{rep:?}"
    );
    assert_eq!(before, t.state(), "no fallback transfer happened");
}

#[test]
fn missing_or_substituted_extra_accounts_fail_atomically() {
    let t = hooked(vec![]);
    let before = t.state();

    let no_counter = t.transfer(10, &[1, 2]);
    assert_eq!(
        code(&no_counter),
        Some(ACCOUNT_RESOLUTION),
        "{no_counter:?}"
    );
    let no_validation = t.transfer(10, &[0, 2]);
    assert_eq!(
        no_validation.error.as_deref(),
        Some("Failure(NotEnoughAccountKeys)"),
        "{no_validation:?}"
    );
    let no_program = t.transfer(10, &[0, 1]);
    assert_eq!(
        no_program.error.as_deref(),
        Some("Failure(NotEnoughAccountKeys)"),
        "{no_program:?}"
    );

    // A same-shaped decoy counter owned by the hook program is not the account the list requires.
    let decoy =
        t.e.h
            .create_owned_account("decoy", 17, &demo_hook_program_id())
            .unwrap();
    let mut ix =
        t.e.h
            .transfer_checked_ix(&t.e.a, &t.e.mint, &t.e.b, &t.e.alice, 10, 6)
            .unwrap();
    let extras = t.e.h.hook_extra_accounts(&t.hook);
    ix.accounts.extend([
        AccountMeta::new(decoy, false),
        extras[1].clone(),
        extras[2].clone(),
    ]);
    let rep = t.e.h.execute_logged(&ix);
    assert_eq!(code(&rep), Some(ACCOUNT_RESOLUTION), "{rep:?}");

    assert_eq!(
        before,
        t.state(),
        "every rejected transfer left balances and the counter unchanged"
    );
}

#[test]
fn hook_rejection_fails_the_transfer_atomically() {
    let t = hooked(vec![]);
    assert_eq!(t.transfer(10, &[0, 1, 2]).error, None);
    t.e.h.set_hook_reject(&t.hook, true).unwrap();
    let before = t.state();
    let rep = t.transfer(10, &[0, 1, 2]);
    assert_eq!(code(&rep), Some(HOOK_REJECTED), "{rep:?}");
    assert_eq!(
        before,
        t.state(),
        "balances and counter unchanged after the hook rejected"
    );
    t.e.h.set_hook_reject(&t.hook, false).unwrap();
    assert_eq!(
        t.transfer(10, &[0, 1, 2]).error,
        None,
        "works again once the hook accepts"
    );
    assert_eq!(t.state(), (980, 20, 2));
}

#[test]
fn direct_execute_outside_a_transfer_is_rejected_by_the_hook() {
    let t = hooked(vec![]);
    let rep = t.e.h.execute_logged(
        &t.e.h
            .direct_execute_ix(&t.hook, &t.e.a, &t.e.mint, &t.e.b, &t.e.alice, 5),
    );
    assert_eq!(code(&rep), Some(HOOK_NOT_TRANSFERRING), "{rep:?}");
    assert_eq!(t.e.h.hook_counter(&t.hook).unwrap().invocations, 0);
}

#[test]
fn hook_combined_with_transfer_fee_charges_the_fee_and_invokes_the_hook() {
    let t = hooked(vec![MintExt::TransferFee {
        basis_points: 200,
        maximum_fee: u64::MAX,
    }]);
    assert_eq!(t.transfer(500, &[0, 1, 2]).error, None);
    assert_eq!((t.e.amount(&t.e.a), t.e.amount(&t.e.b)), (500, 490));
    assert_eq!(t.e.h.token_account(&t.e.b).unwrap().withheld, Some(10));
    assert_eq!(
        t.e.h.hook_counter(&t.hook).unwrap().last_amount,
        500,
        "the hook sees the nominal amount"
    );
}

#[test]
fn pause_blocks_the_transfer_before_the_hook_runs_and_mint_to_never_invokes_it() {
    let t = hooked(vec![MintExt::Pausable]);
    let p = t.e.h.payer();
    t.e.h
        .execute(&pausable::instruction::pause(&tok(), &t.e.mint, &p, &[]).unwrap())
        .unwrap();
    let before = t.state();
    let rep = t.transfer(10, &[0, 1, 2]);
    assert_eq!(code(&rep), Some(PAUSED), "{rep:?}");
    assert_eq!(before, t.state(), "hook was not invoked and nothing moved");

    t.e.h
        .execute(&pausable::instruction::resume(&tok(), &t.e.mint, &p, &[]).unwrap())
        .unwrap();
    t.e.h
        .execute(&ti::mint_to(&tok(), &t.e.mint, &t.e.a, &p, &[], 1).unwrap())
        .unwrap();
    assert_eq!(
        t.e.h.hook_counter(&t.hook).unwrap().invocations,
        0,
        "minting does not call the hook"
    );
}

#[test]
fn hook_program_that_is_not_installed_fails_the_transfer() {
    let e = env(&MintPlan::new(
        6,
        vec![MintExt::TransferHook {
            program_id: derive_address("not-loaded"),
        }],
    ));
    e.h.mint_to(&e.mint, &e.a, 100).unwrap();
    let rep = e.transfer(&e.a, &e.b, &e.alice, 10);
    assert!(rep.error.is_some(), "{rep:?}");
    assert_eq!((e.amount(&e.a), e.amount(&e.b)), (100, 0));
}
