//! CPI Guard on the CPI path using the demo forwarder program (`programs/demo-forwarder`).
//!
//! Solana Token-2022 specification for CPI Guard:
//! * Top-level (stack height 1) operations are unaffected.
//! * In CPI (stack height > 1), owner-signed operations that move funds, burn, approve delegates,
//!   close accounts to third parties, or change authorities are strictly blocked.
//! * Disabling CPI Guard from within CPI is forbidden (`CpiGuardSettingsLocked`).
//! * Delegate transfers via CPI are explicitly allowed and decrement the delegate allowance.

mod common;

use common::*;
use spl_token_2022_interface::{
    error::TokenError,
    extension::cpi_guard,
    instruction::{self as ti, AuthorityType},
};
use tokenscope_executor::{ExecReport, MintPlan, derive_address, forward_instruction};

fn forwarder_elf() -> Vec<u8> {
    let dir = std::env::var("TOKENSCOPE_PROGRAM_DIR").unwrap_or_else(|_| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/sbf-programs").into()
    });
    std::fs::read(format!("{dir}/demo_forwarder.so"))
        .unwrap_or_else(|e| panic!("{dir}/demo_forwarder.so: {e} (run scripts/build-programs.sh)"))
}

fn setup_guarded() -> (Env, solana_pubkey::Pubkey) {
    let mut e = env(&MintPlan::new(6, vec![]));
    let elf = forwarder_elf();
    let fwd = e.h.install_demo_forwarder(&elf);

    // Fund alice's account.
    e.h.mint_to(&e.mint, &e.a, 1_000).unwrap();

    // Enable CPI Guard on alice's account.
    for ix in e.h.cpi_guard_instructions(&e.a, &e.alice, true).unwrap() {
        let rep = e.h.execute_logged(&ix);
        assert!(rep.error.is_none(), "enable cpi guard: {:?}", rep.error);
    }
    assert_eq!(e.h.token_cpi_guard(&e.a).unwrap(), Some(true));

    (e, fwd)
}

fn exec_forwarded(
    e: &Env,
    fwd: &solana_pubkey::Pubkey,
    target_ix: &solana_instruction::Instruction,
) -> ExecReport {
    let forward_ix = forward_instruction(fwd, target_ix);
    e.h.execute_logged(&forward_ix)
}

#[test]
fn cpi_transfer_blocked_when_cpi_guard_is_enabled() {
    let (e, fwd) = setup_guarded();

    // Owner Alice attempts transfer from a to b via CPI forwarder.
    let transfer_ix =
        e.h.transfer_checked_ix(&e.a, &e.mint, &e.b, &e.alice, 100, 6)
            .unwrap();
    let rep = exec_forwarded(&e, &fwd, &transfer_ix);

    assert_eq!(
        code(&rep),
        Some(TokenError::CpiGuardTransferBlocked as u32),
        "owner CPI transfer must fail with error 42: {rep:?}"
    );
    assert!(
        log_contains(&rep, "CPI Guard is enabled"),
        "must log CPI Guard restriction"
    );
    assert_eq!(e.amount(&e.a), 1_000, "balance must be untouched");
    assert_eq!(e.amount(&e.b), 0, "destination balance must be untouched");
}

#[test]
fn cpi_burn_blocked_when_cpi_guard_is_enabled() {
    let (e, fwd) = setup_guarded();

    // Owner Alice attempts burn via CPI forwarder.
    let burn_ix = ti::burn_checked(&tok(), &e.a, &e.mint, &e.alice, &[], 50, 6).unwrap();
    let rep = exec_forwarded(&e, &fwd, &burn_ix);

    assert_eq!(
        code(&rep),
        Some(TokenError::CpiGuardBurnBlocked as u32),
        "owner CPI burn must fail with error 43: {rep:?}"
    );
    assert!(
        log_contains(&rep, "CPI Guard is enabled"),
        "must log CPI Guard restriction"
    );
    assert_eq!(e.amount(&e.a), 1_000, "balance must be untouched");
}

#[test]
fn cpi_approve_blocked_when_cpi_guard_is_enabled() {
    let (e, fwd) = setup_guarded();
    let carol = derive_address("carol");

    // Owner Alice attempts to approve delegate Carol via CPI forwarder.
    let approve_ix = ti::approve(&tok(), &e.a, &carol, &e.alice, &[], 200).unwrap();
    let rep = exec_forwarded(&e, &fwd, &approve_ix);

    assert_eq!(
        code(&rep),
        Some(TokenError::CpiGuardApproveBlocked as u32),
        "owner CPI approve must fail with error 45: {rep:?}"
    );
    assert!(
        log_contains(&rep, "CPI Guard is enabled"),
        "must log CPI Guard restriction"
    );
    assert_eq!(
        e.h.token_account(&e.a).unwrap().delegate,
        None,
        "delegate must not be set"
    );
}

#[test]
fn cpi_close_account_blocked_when_cpi_guard_is_enabled() {
    let (e, fwd) = setup_guarded();
    let carol = derive_address("carol");

    // Create an empty account with CPI Guard enabled.
    let empty_acct =
        e.h.create_token_account("empty", &e.mint, &e.alice)
            .unwrap();
    for ix in
        e.h.cpi_guard_instructions(&empty_acct, &e.alice, true)
            .unwrap()
    {
        e.h.execute(&ix).unwrap();
    }

    // Owner Alice attempts to close account directing lamports to Carol (not owner) via CPI.
    let close_ix = ti::close_account(&tok(), &empty_acct, &carol, &e.alice, &[]).unwrap();
    let rep = exec_forwarded(&e, &fwd, &close_ix);

    assert_eq!(
        code(&rep),
        Some(TokenError::CpiGuardCloseAccountBlocked as u32),
        "owner CPI close account to third party must fail with error 44: {rep:?}"
    );
    assert!(
        log_contains(&rep, "CPI Guard is enabled"),
        "must log CPI Guard restriction"
    );
    assert!(
        e.h.token_account(&empty_acct).is_ok(),
        "account must remain open"
    );
}

#[test]
fn cpi_set_authority_blocked_when_cpi_guard_is_enabled() {
    let (e, fwd) = setup_guarded();
    let mallory = derive_address("mallory");

    // Owner Alice attempts to change owner to Mallory via CPI forwarder.
    let set_auth_ix = ti::set_authority(
        &tok(),
        &e.a,
        Some(&mallory),
        AuthorityType::AccountOwner,
        &e.alice,
        &[],
    )
    .unwrap();
    let rep = exec_forwarded(&e, &fwd, &set_auth_ix);

    assert_eq!(
        code(&rep),
        Some(TokenError::CpiGuardSetAuthorityBlocked as u32),
        "owner CPI set authority must fail with error 46: {rep:?}"
    );
    assert!(
        log_contains(&rep, "CPI Guard is enabled"),
        "must log CPI Guard restriction"
    );
    assert_eq!(
        e.h.token_account(&e.a).unwrap().owner,
        e.alice,
        "owner must remain unchanged"
    );
}

#[test]
fn cpi_disabling_guard_via_cpi_is_locked() {
    let (e, fwd) = setup_guarded();

    // Owner Alice attempts to disable CPI Guard from within CPI.
    let disable_ix =
        cpi_guard::instruction::disable_cpi_guard(&tok(), &e.a, &e.alice, &[]).unwrap();
    let rep = exec_forwarded(&e, &fwd, &disable_ix);

    assert_eq!(
        code(&rep),
        Some(TokenError::CpiGuardSettingsLocked as u32),
        "disabling CPI Guard via CPI must fail with error 41: {rep:?}"
    );
    assert!(
        log_contains(&rep, "CPI Guard status cannot be changed in CPI"),
        "must log settings locked message"
    );
    assert_eq!(
        e.h.token_cpi_guard(&e.a).unwrap(),
        Some(true),
        "guard must remain enabled"
    );
}

#[test]
fn delegate_cpi_transfer_is_allowed_under_cpi_guard() {
    let (e, fwd) = setup_guarded();
    let carol = derive_address("carol");

    // Top-level approval: Owner Alice approves delegate Carol for 200 tokens.
    let approve_rep =
        e.h.execute_logged(&ti::approve(&tok(), &e.a, &carol, &e.alice, &[], 200).unwrap());
    assert!(
        approve_rep.error.is_none(),
        "top-level approve must succeed"
    );

    // Delegate Carol transfers 150 tokens via CPI forwarder.
    let transfer_ix =
        e.h.transfer_checked_ix(&e.a, &e.mint, &e.b, &carol, 150, 6)
            .unwrap();
    let rep = exec_forwarded(&e, &fwd, &transfer_ix);

    assert!(
        rep.error.is_none(),
        "delegate CPI transfer must succeed under CPI Guard: {:?}",
        rep.error
    );
    assert_eq!(e.amount(&e.a), 850, "source balance must decrease by 150");
    assert_eq!(
        e.amount(&e.b),
        150,
        "destination balance must increase by 150"
    );
    assert_eq!(
        e.h.token_account(&e.a).unwrap().delegated_amount,
        50,
        "delegate allowance must decrease by 150 to 50"
    );
}

#[test]
fn cpi_transfer_succeeds_when_cpi_guard_is_disabled() {
    let (e, fwd) = setup_guarded();

    // Top-level disable CPI Guard.
    for ix in e.h.cpi_guard_instructions(&e.a, &e.alice, false).unwrap() {
        e.h.execute(&ix).unwrap();
    }
    assert_eq!(e.h.token_cpi_guard(&e.a).unwrap(), Some(false));

    // Owner Alice transfers via CPI forwarder.
    let transfer_ix =
        e.h.transfer_checked_ix(&e.a, &e.mint, &e.b, &e.alice, 100, 6)
            .unwrap();
    let rep = exec_forwarded(&e, &fwd, &transfer_ix);

    assert!(
        rep.error.is_none(),
        "owner CPI transfer must succeed when CPI Guard is disabled: {:?}",
        rep.error
    );
    assert_eq!(e.amount(&e.a), 900);
    assert_eq!(e.amount(&e.b), 100);
}
