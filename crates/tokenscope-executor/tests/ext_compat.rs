//! Extension compatibility, initialization order and configuration failures, observed on the real
//! Token-2022 program. Where the runtime differs from the documentation the test asserts the
//! runtime and says so.

mod common;

use common::*;
use solana_pubkey::Pubkey;
use spl_token_2022_interface::{
    extension::{ExtensionType, pausable},
    instruction as ti,
};
use tokenscope_executor::{Harness, MintExt, MintPlan, derive_address};

fn sorted(mut v: Vec<ExtensionType>) -> Vec<u16> {
    let mut n: Vec<u16> = v.drain(..).map(u16::from).collect();
    n.sort_unstable();
    n
}

#[test]
fn valid_combinations_initialize_and_report_exactly_their_extensions() {
    let delegate = derive_address("issuer");
    let hook = derive_address("hook-program");
    let combos: Vec<Vec<MintExt>> = vec![
        vec![
            MintExt::TransferFee {
                basis_points: 100,
                maximum_fee: 50,
            },
            MintExt::Pausable,
        ],
        vec![
            MintExt::TransferFee {
                basis_points: 100,
                maximum_fee: 50,
            },
            MintExt::DefaultAccountState { frozen: true },
        ],
        vec![
            MintExt::TransferHook { program_id: hook },
            MintExt::PermanentDelegate { delegate },
        ],
        vec![
            MintExt::Pausable,
            MintExt::MintCloseAuthority,
            MintExt::InterestBearing { rate: 50 },
        ],
        vec![
            MintExt::ScaledUiAmount {
                multiplier_milli: 1500,
            },
            MintExt::TransferFee {
                basis_points: 5,
                maximum_fee: 5,
            },
        ],
        vec![
            MintExt::TransferFee {
                basis_points: 100,
                maximum_fee: 50,
            },
            MintExt::TransferHook { program_id: hook },
            MintExt::DefaultAccountState { frozen: false },
            MintExt::Pausable,
            MintExt::PermanentDelegate { delegate },
        ],
    ];
    for (i, exts) in combos.into_iter().enumerate() {
        let h = Harness::new();
        let want: Vec<ExtensionType> = exts.iter().map(MintExt::extension_type).collect();
        let mint = h
            .create_mint_ext(&format!("c{i}"), &MintPlan::new(6, exts))
            .unwrap_or_else(|e| panic!("combo {i}: {e}"));
        assert_eq!(
            sorted(h.mint_extension_types(&mint).unwrap()),
            sorted(want),
            "combo {i}"
        );
    }
}

#[test]
fn scaled_ui_amount_with_interest_bearing_is_rejected_by_the_runtime() {
    let h = Harness::new();
    let plan = MintPlan::new(
        6,
        vec![
            MintExt::ScaledUiAmount {
                multiplier_milli: 2000,
            },
            MintExt::InterestBearing { rate: 100 },
        ],
    );
    let err = h.create_mint_ext("bad", &plan).unwrap_err().to_string();
    assert!(err.contains(&format!("Custom({INVALID_COMBO})")), "{err}");
    // State integrity: the failed InitializeMint2 left no usable mint behind.
    let mint = derive_address("mint/bad");
    assert!(
        h.mint_supply(&mint).is_err(),
        "mint must remain uninitialized"
    );
    // Each extension alone is fine.
    assert!(
        h.create_mint_ext(
            "s",
            &MintPlan::new(
                6,
                vec![MintExt::ScaledUiAmount {
                    multiplier_milli: 2000
                }]
            )
        )
        .is_ok()
    );
    assert!(
        h.create_mint_ext(
            "i",
            &MintPlan::new(6, vec![MintExt::InterestBearing { rate: 100 }])
        )
        .is_ok()
    );
}

#[test]
fn non_transferable_with_transfer_fee_is_accepted_contrary_to_the_docs_example() {
    // The Solana docs use NonTransferable + TransferFeeConfig as an example of an incompatible
    // pair. The interface crate's combination check (extension/mod.rs) only rejects
    // confidential-transfer combinations and ScaledUiAmount + InterestBearingConfig, and the
    // bundled program accepts this pair. Transfers are still blocked by NonTransferable.
    let e = env(&MintPlan::new(
        6,
        vec![
            MintExt::NonTransferable,
            MintExt::TransferFee {
                basis_points: 100,
                maximum_fee: 10,
            },
        ],
    ));
    e.h.mint_to(&e.mint, &e.a, 10).unwrap();
    let rep = e.transfer(&e.a, &e.b, &e.alice, 1);
    assert_eq!(code(&rep), Some(NON_TRANSFERABLE), "{rep:?}");
    assert_eq!((e.amount(&e.a), e.amount(&e.b)), (10, 0));
}

#[test]
fn extension_initializers_must_run_before_initialize_mint() {
    let h = Harness::new();
    let plan = MintPlan::new(6, vec![MintExt::Pausable]);
    let (mint, mut ixs) = h.mint_init_instructions("order", &plan).unwrap();
    let init_mint = ixs.pop().unwrap();
    let pause_init = ixs.pop().unwrap();
    let create = ixs.pop().unwrap();
    assert_eq!(h.execute_logged(&create).error, None);
    // Wrong order: InitializeMint2 on an account sized for an uninitialized extension fails.
    let wrong = h.execute_logged(&init_mint);
    assert_eq!(
        wrong.error.as_deref(),
        Some("Failure(InvalidAccountData)"),
        "{wrong:?}"
    );
    assert!(
        h.mint_supply(&mint).is_err(),
        "mint still uninitialized after the rejected step"
    );
    // Correct order on the same account succeeds.
    assert_eq!(h.execute_logged(&pause_init).error, None);
    assert_eq!(h.execute_logged(&init_mint).error, None);
    assert_eq!(h.mint_paused(&mint).unwrap(), Some(false));
}

#[test]
fn duplicate_extension_initialization_before_initialize_mint_is_accepted() {
    // Observed: re-running an extension initializer before InitializeMint succeeds (it rewrites
    // the extension state). Recorded so integrations do not assume a "duplicate" error.
    let h = Harness::new();
    let (_, ixs) = h
        .mint_init_instructions("dup", &MintPlan::new(6, vec![MintExt::Pausable]))
        .unwrap();
    assert_eq!(h.execute_logged(&ixs[0]).error, None);
    assert_eq!(h.execute_logged(&ixs[1]).error, None);
    assert_eq!(h.execute_logged(&ixs[1]).error, None);
}

#[test]
fn undersized_mint_cannot_hold_an_extension() {
    let h = Harness::new();
    let small = h.create_owned_account("small", 82, &tok()).unwrap();
    let rep =
        h.execute_logged(&pausable::instruction::initialize(&tok(), &small, &h.payer()).unwrap());
    assert_eq!(
        rep.error.as_deref(),
        Some("Failure(InvalidAccountData)"),
        "{rep:?}"
    );
}

#[test]
fn token_account_without_required_account_extension_space_is_rejected() {
    let h = Harness::new();
    let mint = h
        .create_mint_ext(
            "m",
            &MintPlan::new(
                6,
                vec![MintExt::TransferFee {
                    basis_points: 1,
                    maximum_fee: 1,
                }],
            ),
        )
        .unwrap();
    let base_only = h.create_owned_account("base", 165, &tok()).unwrap();
    let rep = h.execute_logged(
        &ti::initialize_account3(&tok(), &base_only, &mint, &derive_address("alice")).unwrap(),
    );
    assert_eq!(
        rep.error.as_deref(),
        Some("Failure(InvalidAccountData)"),
        "{rep:?}"
    );
    // The harness sizes accounts from the mint's extensions, so the same mint works through it.
    assert!(
        h.create_token_account("ok", &mint, &derive_address("alice"))
            .is_ok()
    );
}

#[test]
fn harness_derives_every_required_account_extension_from_the_mint() {
    let hook = derive_address("hook-program");
    let e = env(&MintPlan::new(
        6,
        vec![
            MintExt::TransferFee {
                basis_points: 1,
                maximum_fee: 1,
            },
            MintExt::TransferHook { program_id: hook },
            MintExt::Pausable,
            MintExt::NonTransferable,
        ],
    ));
    let have = sorted(e.h.token_account_extension_types(&e.a).unwrap());
    let want = sorted(vec![
        ExtensionType::TransferFeeAmount,
        ExtensionType::TransferHookAccount,
        ExtensionType::PausableAccount,
        ExtensionType::NonTransferableAccount,
        // Observed: NonTransferable mints also force ImmutableOwner on their token accounts.
        ExtensionType::ImmutableOwner,
    ]);
    assert_eq!(have, want);
}

#[test]
fn mint_close_authority_can_close_only_an_empty_mint() {
    let h = Harness::new();
    let mint = h
        .create_mint_ext("m", &MintPlan::new(6, vec![MintExt::MintCloseAuthority]))
        .unwrap();
    let dest: Pubkey = derive_address("dest");
    let close = ti::close_account(&tok(), &mint, &dest, &h.payer(), &[]).unwrap();
    assert_eq!(h.execute_logged(&close).error, None, "empty mint closes");

    let m2 = h
        .create_mint_ext("m2", &MintPlan::new(6, vec![MintExt::MintCloseAuthority]))
        .unwrap();
    let acct = h
        .create_token_account("a", &m2, &derive_address("alice"))
        .unwrap();
    h.mint_to(&m2, &acct, 1).unwrap();
    let close2 = ti::close_account(&tok(), &m2, &dest, &h.payer(), &[]).unwrap();
    assert!(
        h.execute_logged(&close2).error.is_some(),
        "mint with supply must not close"
    );
    assert_eq!(h.mint_supply(&m2).unwrap(), 1);
}
