//! Delegates and the permanent delegate against the real Token-2022 program.

mod common;

use common::*;
use spl_token_2022_interface::instruction as ti;
use tokenscope_executor::{MintExt, MintPlan, derive_address};

fn plain() -> Env {
    let e = env(&MintPlan::new(6, vec![]));
    e.h.mint_to(&e.mint, &e.a, 100).unwrap();
    e
}

fn approve(
    e: &Env,
    delegate: &solana_pubkey::Pubkey,
    owner: &solana_pubkey::Pubkey,
    amount: u64,
) -> tokenscope_executor::ExecReport {
    e.h.execute_logged(&ti::approve(&tok(), &e.a, delegate, owner, &[], amount).unwrap())
}

#[test]
fn allowance_is_spent_down_and_cannot_be_exceeded() {
    let e = plain();
    let carol = derive_address("carol");
    assert_eq!(approve(&e, &carol, &e.alice, 30).error, None);
    let v = e.h.token_account(&e.a).unwrap();
    assert_eq!((v.delegate, v.delegated_amount), (Some(carol), 30));

    assert_eq!(code(&e.transfer(&e.a, &e.b, &carol, 20)), None);
    let v = e.h.token_account(&e.a).unwrap();
    assert_eq!(
        (v.amount, v.delegated_amount),
        (80, 10),
        "owner balance and allowance both decrease"
    );
    assert_eq!(e.amount(&e.b), 20);

    let rep = e.transfer(&e.a, &e.b, &carol, 20);
    assert_eq!(code(&rep), Some(INSUFFICIENT), "{rep:?}");
    assert_eq!(
        (
            e.amount(&e.a),
            e.amount(&e.b),
            e.h.token_account(&e.a).unwrap().delegated_amount
        ),
        (80, 20, 10)
    );
}

#[test]
fn only_the_owner_can_approve_and_strangers_cannot_spend() {
    let e = plain();
    let (carol, mallory) = (derive_address("carol"), derive_address("mallory"));
    approve(&e, &carol, &e.alice, 30);
    assert_eq!(
        code(&approve(&e, &mallory, &carol, 5)),
        Some(OWNER_MISMATCH),
        "a delegate cannot approve others"
    );
    assert_eq!(
        code(&e.transfer(&e.a, &e.b, &mallory, 1)),
        Some(OWNER_MISMATCH)
    );
    assert_eq!(
        e.h.token_account(&e.a).unwrap().delegate,
        Some(carol),
        "delegate unchanged by the rejected attempts"
    );
}

#[test]
fn revocation_removes_spending_power() {
    let e = plain();
    let carol = derive_address("carol");
    approve(&e, &carol, &e.alice, 30);
    let rev =
        e.h.execute_logged(&ti::revoke(&tok(), &e.a, &e.alice, &[]).unwrap());
    assert_eq!(rev.error, None);
    let v = e.h.token_account(&e.a).unwrap();
    assert_eq!((v.delegate, v.delegated_amount), (None, 0));
    assert_eq!(
        code(&e.transfer(&e.a, &e.b, &carol, 1)),
        Some(OWNER_MISMATCH)
    );
    assert_eq!(e.amount(&e.a), 100);
}

#[test]
fn owner_transfers_do_not_consume_the_delegates_allowance() {
    let e = plain();
    let carol = derive_address("carol");
    approve(&e, &carol, &e.alice, 30);
    assert_eq!(code(&e.transfer(&e.a, &e.b, &e.alice, 25)), None);
    assert_eq!(e.h.token_account(&e.a).unwrap().delegated_amount, 30);
}

#[test]
fn permanent_delegate_can_move_and_burn_from_any_account_without_approval() {
    let p_delegate = derive_address("issuer");
    let e = env(&MintPlan::new(
        6,
        vec![MintExt::PermanentDelegate {
            delegate: p_delegate,
        }],
    ));
    e.h.mint_to(&e.mint, &e.a, 100).unwrap();

    // The owner (alice) never signs and never approved anyone, yet her balance can be taken.
    let v = e.h.token_account(&e.a).unwrap();
    assert_eq!(v.delegate, None, "no approval is visible on the account");
    let rep = e.transfer(&e.a, &e.b, &p_delegate, 40);
    assert_eq!(code(&rep), None, "{rep:?}");
    assert_eq!((e.amount(&e.a), e.amount(&e.b)), (60, 40));
    assert_eq!(
        e.h.token_account(&e.a).unwrap().delegate,
        None,
        "still nothing visible after the transfer"
    );

    let burn =
        e.h.execute_logged(&ti::burn(&tok(), &e.a, &e.mint, &p_delegate, &[], 10).unwrap());
    assert_eq!(burn.error, None, "{burn:?}");
    assert_eq!(
        (e.amount(&e.a), e.h.mint_supply(&e.mint).unwrap()),
        (50, 90)
    );

    // A stranger still cannot.
    assert_eq!(
        code(&e.transfer(&e.a, &e.b, &derive_address("mallory"), 1)),
        Some(OWNER_MISMATCH)
    );
}

#[test]
fn without_the_extension_the_same_actor_has_no_power_control() {
    let e = plain();
    let issuer = derive_address("issuer");
    assert_eq!(
        code(&e.transfer(&e.a, &e.b, &issuer, 40)),
        Some(OWNER_MISMATCH)
    );
    assert_eq!(e.amount(&e.a), 100);
}

#[test]
fn permanent_delegate_transfers_still_pay_transfer_fees() {
    let d = derive_address("issuer");
    let e = env(&MintPlan::new(
        6,
        vec![
            MintExt::PermanentDelegate { delegate: d },
            MintExt::TransferFee {
                basis_points: 200,
                maximum_fee: u64::MAX,
            },
        ],
    ));
    e.h.mint_to(&e.mint, &e.a, 1_000).unwrap();
    assert_eq!(code(&e.transfer(&e.a, &e.b, &d, 500)), None);
    assert_eq!(
        (
            e.amount(&e.a),
            e.amount(&e.b),
            e.h.token_account(&e.b).unwrap().withheld
        ),
        (500, 490, Some(10))
    );
}
