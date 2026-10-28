//! Extensions and adversarial operations through the escrow's real CPI path, on both flavors.
//! Expected error codes were observed on the real programs (Token-2022 `TokenError` values and the
//! escrow's `EscrowError` values 1..=7).

use spl_token_2022_interface::error::TokenError;
use tokenscope_adapter::{EscrowAdapter, EscrowFlavor};
use tokenscope_model::{
    Action, ExecutionTrace, MintExtension, MintSpec, Mismatch, SCHEMA_VERSION, Scenario,
    StepRecord, StepStatus, TransferFeeParams,
};

const FLAVORS: [EscrowFlavor; 2] = [EscrowFlavor::Vulnerable, EscrowFlavor::Corrected];

fn scenario(
    flavor: EscrowFlavor,
    fee: Option<(u16, u64)>,
    exts: Vec<MintExtension>,
    balance: u64,
    actions: Vec<Action>,
) -> Scenario {
    Scenario {
        schema_version: SCHEMA_VERSION,
        name: "ext".into(),
        seed: 0,
        target: flavor.target_id().into(),
        mint: MintSpec {
            decimals: 6,
            transfer_fee: fee.map(|(basis_points, maximum_fee)| TransferFeeParams {
                basis_points,
                maximum_fee,
            }),
        },
        initial_balance: balance,
        extensions: exts,
        actions,
    }
}

fn run(
    flavor: EscrowFlavor,
    fee: Option<(u16, u64)>,
    exts: Vec<MintExtension>,
    balance: u64,
    actions: Vec<Action>,
) -> ExecutionTrace {
    EscrowAdapter::new(flavor)
        .run(&scenario(flavor, fee, exts, balance, actions))
        .expect("setup must succeed (run scripts/build-programs.sh first)")
}

fn custom(step: &StepRecord) -> Option<u32> {
    match &step.status {
        StepStatus::Failed { error } => error
            .strip_prefix("Failure(Custom(")?
            .strip_suffix("))")?
            .parse()
            .ok(),
        StepStatus::Ok => None,
    }
}

fn dep(amount: u64) -> Action {
    Action::Deposit { amount }
}

#[test]
fn pausable_mint_gates_deposits_and_pause_state_is_observed() {
    for f in FLAVORS {
        let t = run(
            f,
            None,
            vec![MintExtension::Pausable],
            10_000,
            vec![Action::Pause, dep(100), Action::Resume, dep(100)],
        );
        assert_eq!(t.steps[0].after.mint_paused, Some(true), "{f:?}");
        assert_eq!(
            custom(&t.steps[1]),
            Some(TokenError::MintPaused as u32),
            "{f:?}"
        );
        assert_eq!(
            t.steps[1].before, t.steps[1].after,
            "paused deposit persisted nothing"
        );
        assert_eq!(t.steps[2].after.mint_paused, Some(false));
        assert_eq!(t.steps[3].status, StepStatus::Ok);
        assert_eq!(
            (
                t.steps[3].after.vault_amount,
                t.steps[3].after.escrow_recorded
            ),
            (100, 100)
        );
    }
}

#[test]
fn pause_without_the_extension_is_a_rejected_instruction() {
    let t = run(
        EscrowFlavor::Corrected,
        None,
        vec![],
        100,
        vec![Action::Pause],
    );
    assert!(matches!(t.steps[0].status, StepStatus::Failed { .. }));
    assert_eq!(t.steps[0].after.mint_paused, None);
}

#[test]
fn cpi_guard_blocks_the_escrows_owner_signed_cpi_transfer_until_disabled() {
    for f in FLAVORS {
        let t = run(
            f,
            None,
            vec![],
            10_000,
            vec![
                Action::EnableCpiGuard,
                dep(100),
                Action::DisableCpiGuard,
                dep(100),
            ],
        );
        assert!(
            t.steps[0].after.source_cpi_guard,
            "enable (top-level, with reallocate) succeeded"
        );
        assert_eq!(
            custom(&t.steps[1]),
            Some(TokenError::CpiGuardTransferBlocked as u32),
            "{f:?}"
        );
        assert!(
            t.steps[1]
                .logs
                .iter()
                .any(|l| l.contains("CPI Guard is enabled")),
            "{:?}",
            t.steps[1].logs
        );
        assert_eq!(t.steps[1].before, t.steps[1].after);
        assert!(!t.steps[2].after.source_cpi_guard);
        assert_eq!(
            t.steps[3].status,
            StepStatus::Ok,
            "deposit works once the guard is disabled"
        );
        assert_eq!(t.steps[3].after.vault_amount, 100);
    }
}

#[test]
fn default_frozen_vault_blocks_deposits_until_thawed() {
    for f in FLAVORS {
        let t = run(
            f,
            None,
            vec![MintExtension::DefaultAccountState { frozen: true }],
            10_000,
            vec![dep(100), Action::ThawVault, dep(100)],
        );
        assert!(
            t.initial.vault_frozen && !t.initial.depositor_frozen,
            "only the vault starts frozen"
        );
        assert_eq!(
            t.initial.depositor_balance, 10_000,
            "setup thawed the depositor and minted"
        );
        assert_eq!(
            custom(&t.steps[0]),
            Some(TokenError::AccountFrozen as u32),
            "{f:?}"
        );
        assert_eq!(t.steps[0].before, t.steps[0].after);
        assert!(!t.steps[1].after.vault_frozen);
        assert_eq!(t.steps[2].status, StepStatus::Ok);
        assert_eq!(t.steps[2].after.vault_amount, 100);
    }
}

#[test]
fn permanent_delegate_can_drain_the_vault_without_the_escrows_authority() {
    for f in FLAVORS {
        let t = run(
            f,
            None,
            vec![MintExtension::PermanentDelegate],
            10_000,
            vec![dep(1_000), Action::PermanentDelegateDrain { amount: 400 }],
        );
        let d = &t.steps[1];
        assert_eq!(d.status, StepStatus::Ok, "{f:?}: {:?}", d.logs);
        assert_eq!((d.after.vault_amount, d.after.issuer_balance), (600, 400));
        assert_eq!(
            d.after.escrow_recorded, 1_000,
            "the escrow still believes it holds 1000"
        );
    }
}

#[test]
fn drain_attempt_without_a_permanent_delegate_is_rejected_by_token_2022() {
    let t = run(
        EscrowFlavor::Corrected,
        None,
        vec![],
        10_000,
        vec![dep(1_000), Action::PermanentDelegateDrain { amount: 400 }],
    );
    assert_eq!(custom(&t.steps[1]), Some(TokenError::OwnerMismatch as u32));
    assert_eq!(t.steps[1].after.vault_amount, 1_000);
}

#[test]
fn delegate_allowance_lifecycle_is_observed_and_does_not_touch_the_escrow() {
    for f in FLAVORS {
        let t = run(
            f,
            None,
            vec![],
            10_000,
            vec![
                Action::ApproveDelegate { amount: 300 },
                Action::DelegateTransfer { amount: 200 },
                Action::DelegateTransfer { amount: 200 },
                Action::RevokeDelegate,
            ],
        );
        assert_eq!(t.steps[0].after.delegate_allowance, 300);
        assert_eq!(
            (
                t.steps[1].after.delegate_allowance,
                t.steps[1].after.depositor_balance
            ),
            (100, 9_800)
        );
        assert_eq!(
            custom(&t.steps[2]),
            Some(TokenError::InsufficientFunds as u32),
            "allowance cannot be exceeded"
        );
        assert_eq!(t.steps[2].before, t.steps[2].after);
        assert_eq!(t.steps[3].after.delegate_allowance, 0);
        assert!(
            t.steps
                .iter()
                .all(|s| s.after.vault_amount == 0 && s.after.escrow_recorded == 0)
        );
    }
}

#[test]
fn transfer_hook_mints_are_incompatible_with_an_escrow_that_does_not_forward_extra_accounts() {
    for f in FLAVORS {
        let t = run(
            f,
            None,
            vec![MintExtension::TransferHook],
            10_000,
            vec![dep(100)],
        );
        assert_eq!(t.initial.hook_invocations, Some(0));
        let s = &t.steps[0];
        assert_eq!(
            s.status,
            StepStatus::Failed {
                error: "Failure(NotEnoughAccountKeys)".into()
            },
            "{f:?}"
        );
        assert_eq!(s.before, s.after);
        assert_eq!(s.after.hook_invocations, Some(0), "the hook never ran");
    }
}

#[test]
fn mismatched_accounts_and_reinitialization_are_rejected_without_state_change() {
    let expected = [
        (
            Action::MismatchedDeposit {
                mismatch: Mismatch::WrongMint,
                amount: 5,
            },
            7,
        ),
        (
            Action::MismatchedDeposit {
                mismatch: Mismatch::WrongVault,
                amount: 5,
            },
            4,
        ),
        (
            Action::MismatchedDeposit {
                mismatch: Mismatch::WrongTokenProgram,
                amount: 5,
            },
            6,
        ),
        (
            Action::MismatchedWithdraw {
                mismatch: Mismatch::WrongMint,
            },
            7,
        ),
        (
            Action::MismatchedWithdraw {
                mismatch: Mismatch::WrongVault,
            },
            4,
        ),
        (
            Action::MismatchedWithdraw {
                mismatch: Mismatch::WrongTokenProgram,
            },
            6,
        ),
        (
            Action::MismatchedWithdraw {
                mismatch: Mismatch::WrongAuthority,
            },
            4,
        ),
        (Action::ReinitEscrow, 1),
        (Action::UnauthorizedWithdraw, 5),
    ];
    for f in FLAVORS {
        let mut actions = vec![dep(100)];
        actions.extend(expected.iter().map(|(a, _)| *a));
        let t = run(f, None, vec![], 10_000, actions);
        for (i, (action, code)) in expected.iter().enumerate() {
            let s = &t.steps[i + 1];
            assert_eq!(&s.action, action);
            assert_eq!(custom(s), Some(*code), "{f:?} {action:?}");
            assert_eq!(s.before, s.after, "{f:?} {action:?} must not change state");
            assert!(action.expects_rejection());
        }
        assert_eq!(
            (
                t.steps[0].after.vault_amount,
                t.steps[0].after.escrow_recorded
            ),
            (100, 100)
        );
    }
}

#[test]
fn repeated_withdrawals_are_idempotent_after_the_first() {
    for f in FLAVORS {
        let t = run(
            f,
            None,
            vec![],
            10_000,
            vec![
                dep(1_000),
                Action::Withdraw,
                Action::Withdraw,
                Action::Withdraw,
            ],
        );
        assert_eq!(
            (
                t.steps[1].after.vault_amount,
                t.steps[1].after.destination_balance
            ),
            (0, 1_000)
        );
        for s in &t.steps[2..] {
            assert_eq!(
                s.status,
                StepStatus::Ok,
                "zero-amount withdrawal is a valid transfer"
            );
            assert_eq!(
                (
                    s.after.vault_amount,
                    s.after.escrow_recorded,
                    s.after.destination_balance
                ),
                (0, 0, 1_000)
            );
        }
    }
}

#[test]
fn u64_boundary_amounts_are_handled_without_overflow() {
    for f in FLAVORS {
        // No fee: the full balance deposits and withdraws exactly.
        let t = run(
            f,
            None,
            vec![],
            u64::MAX,
            vec![dep(u64::MAX), Action::Withdraw],
        );
        assert_eq!(t.steps[0].after.vault_amount, u64::MAX);
        assert_eq!(t.steps[0].after.escrow_recorded, u64::MAX);
        assert_eq!(t.steps[1].status, StepStatus::Ok);
        assert_eq!(t.steps[1].after.destination_balance, u64::MAX);
        // With a 1% fee the fee is computed without overflow (u128 math in Token-2022).
        let t = run(
            f,
            Some((100, u64::MAX)),
            vec![],
            u64::MAX,
            vec![dep(u64::MAX)],
        );
        let fee = (u128::from(u64::MAX) * 100).div_ceil(10_000) as u64;
        assert_eq!(t.steps[0].after.vault_amount, u64::MAX - fee, "{f:?}");
        let credited = t.steps[0].after.escrow_recorded;
        match f {
            EscrowFlavor::Vulnerable => {
                assert_eq!(credited, u64::MAX, "bug: nominal amount credited")
            }
            EscrowFlavor::Corrected => assert_eq!(credited, u64::MAX - fee),
        }
    }
}
