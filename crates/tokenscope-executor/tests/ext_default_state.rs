//! `DefaultAccountState` against the real Token-2022 program.

mod common;

use common::*;
use spl_token_2022_interface::{
    extension::default_account_state, instruction as ti, state::AccountState,
};
use tokenscope_executor::{MintExt, MintPlan};

fn frozen_plan() -> MintPlan {
    MintPlan::new(6, vec![MintExt::DefaultAccountState { frozen: true }])
}

#[test]
fn frozen_default_creates_frozen_accounts_and_blocks_minting_atomically() {
    let e = env(&frozen_plan());
    assert!(e.h.token_account(&e.a).unwrap().frozen && e.h.token_account(&e.b).unwrap().frozen);
    let p = e.h.payer();
    let rep =
        e.h.execute_logged(&ti::mint_to(&tok(), &e.mint, &e.a, &p, &[], 5).unwrap());
    assert_eq!(code(&rep), Some(FROZEN), "{rep:?}");
    assert_eq!(e.amount(&e.a), 0, "failed mint persisted nothing");
    assert_eq!(e.h.mint_supply(&e.mint).unwrap(), 0);
}

#[test]
fn initialized_default_is_the_control_and_creates_usable_accounts() {
    let e = env(&MintPlan::new(
        6,
        vec![MintExt::DefaultAccountState { frozen: false }],
    ));
    assert!(!e.h.token_account(&e.a).unwrap().frozen);
    e.h.mint_to(&e.mint, &e.a, 5).unwrap();
}

#[test]
fn only_the_freeze_authority_can_thaw() {
    let e = env(&frozen_plan());
    let by_owner =
        e.h.execute_logged(&ti::thaw_account(&tok(), &e.a, &e.mint, &e.alice, &[]).unwrap());
    assert_eq!(
        code(&by_owner),
        Some(OWNER_MISMATCH),
        "the account owner is not the freeze authority"
    );
    assert!(
        e.h.token_account(&e.a).unwrap().frozen,
        "still frozen after the rejected thaw"
    );
    e.h.execute(&e.h.freeze_ix(&e.a, &e.mint, false).unwrap())
        .unwrap();
    assert!(!e.h.token_account(&e.a).unwrap().frozen);
}

#[test]
fn transfers_need_both_accounts_thawed_and_failures_change_nothing() {
    let e = env(&frozen_plan());
    let thaw = |acct| {
        e.h.execute(&e.h.freeze_ix(acct, &e.mint, false).unwrap())
            .unwrap()
    };
    thaw(&e.a);
    e.h.mint_to(&e.mint, &e.a, 100).unwrap();

    // Destination still frozen: rejected, nothing moves.
    let before = (
        e.amount(&e.a),
        e.amount(&e.b),
        e.h.mint_supply(&e.mint).unwrap(),
    );
    let rep = e.transfer(&e.a, &e.b, &e.alice, 10);
    assert_eq!(code(&rep), Some(FROZEN), "{rep:?}");
    assert!(log_contains(&rep, "frozen"));
    assert_eq!(
        before,
        (
            e.amount(&e.a),
            e.amount(&e.b),
            e.h.mint_supply(&e.mint).unwrap()
        )
    );
    assert!(e.h.token_account(&e.b).unwrap().frozen && !e.h.token_account(&e.a).unwrap().frozen);

    thaw(&e.b);
    assert_eq!(code(&e.transfer(&e.a, &e.b, &e.alice, 10)), None);
    assert_eq!((e.amount(&e.a), e.amount(&e.b)), (90, 10));
}

#[test]
fn updating_the_default_only_affects_accounts_created_afterwards() {
    let e = env(&frozen_plan());
    let p = e.h.payer();
    let update = default_account_state::instruction::update_default_account_state(
        &tok(),
        &e.mint,
        &p,
        &[],
        &AccountState::Initialized,
    )
    .unwrap();
    let rep = e.h.execute_logged(&update);
    assert_eq!(rep.error, None, "{rep:?}");
    let carol = tokenscope_executor::derive_address("carol");
    let c = e.h.create_token_account("c", &e.mint, &carol).unwrap();
    assert!(
        !e.h.token_account(&c).unwrap().frozen,
        "new account follows the updated default"
    );
    assert!(
        e.h.token_account(&e.a).unwrap().frozen,
        "existing accounts keep their state"
    );
}

#[test]
fn frozen_default_requires_a_freeze_authority() {
    let mut plan = frozen_plan();
    plan.freeze_authority = false;
    let err = tokenscope_executor::Harness::new()
        .create_mint_ext("m", &plan)
        .unwrap_err()
        .to_string();
    assert!(err.contains(&format!("Custom({CANNOT_FREEZE})")), "{err}");
}
