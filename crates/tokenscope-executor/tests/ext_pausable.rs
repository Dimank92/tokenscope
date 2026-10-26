//! `Pausable` against the real Token-2022 program.

mod common;

use common::*;
use spl_token_2022_interface::{extension::pausable, instruction as ti};
use tokenscope_executor::{MintExt, MintPlan};

fn setup() -> Env {
    let e = env(&MintPlan::new(6, vec![MintExt::Pausable]));
    e.h.mint_to(&e.mint, &e.a, 100).unwrap();
    e
}

fn pause(e: &Env, authority: &solana_pubkey::Pubkey) -> tokenscope_executor::ExecReport {
    e.h.execute_logged(&pausable::instruction::pause(&tok(), &e.mint, authority, &[]).unwrap())
}

fn resume(e: &Env, authority: &solana_pubkey::Pubkey) -> tokenscope_executor::ExecReport {
    e.h.execute_logged(&pausable::instruction::resume(&tok(), &e.mint, authority, &[]).unwrap())
}

#[test]
fn paused_mint_blocks_transfer_mint_and_burn_atomically() {
    let e = setup();
    let p = e.h.payer();
    assert_eq!(e.h.mint_paused(&e.mint).unwrap(), Some(false));
    assert_eq!(pause(&e, &p).error, None);
    assert_eq!(
        e.h.mint_paused(&e.mint).unwrap(),
        Some(true),
        "flag decoded from mint data"
    );

    let state = |e: &Env| {
        (
            e.amount(&e.a),
            e.amount(&e.b),
            e.h.mint_supply(&e.mint).unwrap(),
        )
    };
    let before = state(&e);
    let t = e.transfer(&e.a, &e.b, &e.alice, 10);
    let m =
        e.h.execute_logged(&ti::mint_to(&tok(), &e.mint, &e.a, &p, &[], 1).unwrap());
    let b =
        e.h.execute_logged(&ti::burn(&tok(), &e.a, &e.mint, &e.alice, &[], 1).unwrap());
    for (name, rep) in [("transfer", &t), ("mint", &m), ("burn", &b)] {
        assert_eq!(code(rep), Some(PAUSED), "{name}: {rep:?}");
        assert!(log_contains(rep, "paused"), "{name}");
    }
    assert_eq!(
        before,
        state(&e),
        "no partial state change from any rejected operation"
    );
}

#[test]
fn resume_restores_operations() {
    let e = setup();
    let p = e.h.payer();
    pause(&e, &p);
    assert_eq!(resume(&e, &p).error, None);
    assert_eq!(e.h.mint_paused(&e.mint).unwrap(), Some(false));
    assert_eq!(code(&e.transfer(&e.a, &e.b, &e.alice, 10)), None);
    assert_eq!((e.amount(&e.a), e.amount(&e.b)), (90, 10));
}

#[test]
fn only_the_pause_authority_can_pause_or_resume() {
    let e = setup();
    let p = e.h.payer();
    let rep = pause(&e, &e.alice);
    assert_eq!(code(&rep), Some(OWNER_MISMATCH), "{rep:?}");
    assert_eq!(
        e.h.mint_paused(&e.mint).unwrap(),
        Some(false),
        "unauthorized pause changed nothing"
    );
    assert_eq!(code(&e.transfer(&e.a, &e.b, &e.alice, 1)), None);

    pause(&e, &p);
    let rep = resume(&e, &e.bob);
    assert_eq!(code(&rep), Some(OWNER_MISMATCH), "{rep:?}");
    assert_eq!(
        e.h.mint_paused(&e.mint).unwrap(),
        Some(true),
        "unauthorized resume changed nothing"
    );
}

#[test]
fn pausing_is_idempotent_and_only_gates_value_movement() {
    // Observed behavior: pausing twice succeeds; approve and freeze are not gated by the pause.
    let e = setup();
    let p = e.h.payer();
    assert_eq!(pause(&e, &p).error, None);
    assert_eq!(pause(&e, &p).error, None);
    let carol = tokenscope_executor::derive_address("carol");
    let approve =
        e.h.execute_logged(&ti::approve(&tok(), &e.a, &carol, &e.alice, &[], 5).unwrap());
    assert_eq!(approve.error, None, "{approve:?}");
    assert_eq!(
        e.h.execute_logged(&e.h.freeze_ix(&e.b, &e.mint, true).unwrap())
            .error,
        None
    );
}

#[test]
fn pause_with_transfer_fees_keeps_withheld_fees_untouched() {
    let e = env(&MintPlan::new(
        6,
        vec![
            MintExt::Pausable,
            MintExt::TransferFee {
                basis_points: 200,
                maximum_fee: u64::MAX,
            },
        ],
    ));
    e.h.mint_to(&e.mint, &e.a, 1_000).unwrap();
    assert_eq!(code(&e.transfer(&e.a, &e.b, &e.alice, 500)), None);
    let withheld_before = e.h.token_account(&e.b).unwrap().withheld;
    assert_eq!(withheld_before, Some(10));
    pause(&e, &e.h.payer());
    assert_eq!(code(&e.transfer(&e.a, &e.b, &e.alice, 100)), Some(PAUSED));
    assert_eq!(e.h.token_account(&e.b).unwrap().withheld, withheld_before);
    assert_eq!(e.amount(&e.b), 490);
}
