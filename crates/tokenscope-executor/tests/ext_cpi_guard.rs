//! CPI Guard on the direct (top-level) path. The CPI path, where the restriction bites, is covered
//! through the escrow in `crates/tokenscope-adapter/tests/extensions.rs`.

mod common;

use common::*;
use spl_token_2022_interface::{extension::cpi_guard, instruction as ti};
use tokenscope_executor::{MintPlan, derive_address};

fn setup() -> Env {
    let e = env(&MintPlan::new(6, vec![]));
    e.h.mint_to(&e.mint, &e.a, 100).unwrap();
    e
}

fn run_all(e: &Env, ixs: &[solana_instruction::Instruction]) {
    for ix in ixs {
        let rep = e.h.execute_logged(ix);
        assert_eq!(rep.error, None, "{rep:?}");
    }
}

#[test]
fn owner_enables_and_disables_the_guard_and_state_is_decoded() {
    let e = setup();
    assert_eq!(e.h.token_cpi_guard(&e.a).unwrap(), None, "no extension yet");
    run_all(
        &e,
        &e.h.cpi_guard_instructions(&e.a, &e.alice, true).unwrap(),
    );
    assert_eq!(e.h.token_cpi_guard(&e.a).unwrap(), Some(true));
    // Enabling again needs no reallocation and is accepted.
    let again = e.h.cpi_guard_instructions(&e.a, &e.alice, true).unwrap();
    assert_eq!(again.len(), 1);
    run_all(&e, &again);
    run_all(
        &e,
        &e.h.cpi_guard_instructions(&e.a, &e.alice, false).unwrap(),
    );
    assert_eq!(e.h.token_cpi_guard(&e.a).unwrap(), Some(false));
}

#[test]
fn only_the_account_owner_can_toggle_the_guard() {
    let e = setup();
    run_all(
        &e,
        &e.h.cpi_guard_instructions(&e.a, &e.alice, true).unwrap(),
    );
    let mallory = derive_address("mallory");
    let disable = cpi_guard::instruction::disable_cpi_guard(&tok(), &e.a, &mallory, &[]).unwrap();
    let rep = e.h.execute_logged(&disable);
    assert_eq!(code(&rep), Some(OWNER_MISMATCH), "{rep:?}");
    assert_eq!(
        e.h.token_cpi_guard(&e.a).unwrap(),
        Some(true),
        "guard unchanged"
    );
}

#[test]
fn the_guard_does_not_restrict_direct_top_level_operations() {
    let e = setup();
    run_all(
        &e,
        &e.h.cpi_guard_instructions(&e.a, &e.alice, true).unwrap(),
    );
    // Owner-signed top-level transfer still works: the guard only restricts CPI callers.
    assert_eq!(code(&e.transfer(&e.a, &e.b, &e.alice, 10)), None);
    assert_eq!((e.amount(&e.a), e.amount(&e.b)), (90, 10));
    let carol = derive_address("carol");
    let approve =
        e.h.execute_logged(&ti::approve(&tok(), &e.a, &carol, &e.alice, &[], 5).unwrap());
    assert_eq!(approve.error, None, "{approve:?}");
}
