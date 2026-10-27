//! End-to-end: real escrow programs (built by `scripts/build-programs.sh`) running against the real
//! Token-2022 program in the Mollusk SVM.

use tokenscope_adapter::{EscrowAdapter, EscrowFlavor};
use tokenscope_executor::ExecError;
use tokenscope_model::{Action, MintSpec, SCHEMA_VERSION, Scenario, StepStatus, TransferFeeParams};

fn scenario(target: &str, fee: Option<(u16, u64)>, balance: u64, actions: Vec<Action>) -> Scenario {
    Scenario {
        schema_version: SCHEMA_VERSION,
        name: "t".into(),
        seed: 0,
        target: target.into(),
        mint: MintSpec {
            decimals: 6,
            transfer_fee: fee.map(|(basis_points, maximum_fee)| TransferFeeParams {
                basis_points,
                maximum_fee,
            }),
        },
        initial_balance: balance,
        extensions: vec![],
        actions,
    }
}

fn run(
    flavor: EscrowFlavor,
    fee: Option<(u16, u64)>,
    balance: u64,
    actions: Vec<Action>,
) -> tokenscope_model::ExecutionTrace {
    EscrowAdapter::new(flavor)
        .run(&scenario(flavor.target_id(), fee, balance, actions))
        .expect("setup must succeed (did you run scripts/build-programs.sh?)")
}

const DEP: Action = Action::Deposit { amount: 1_000 };

#[test]
fn vulnerable_escrow_credits_nominal_amount_while_vault_receives_net() {
    let t = run(
        EscrowFlavor::Vulnerable,
        Some((200, u64::MAX)),
        10_000,
        vec![DEP],
    );
    let s = &t.steps[0];
    assert_eq!(s.status, StepStatus::Ok, "logs: {:?}", s.logs);
    assert_eq!(
        s.after.escrow_recorded, 1_000,
        "bug: nominal amount credited"
    );
    assert_eq!(
        s.after.vault_amount, 980,
        "real Token-2022 delivered the net amount"
    );
    assert_eq!(s.after.vault_withheld, Some(20));
    assert_eq!(s.after.depositor_balance, 9_000);
}

#[test]
fn corrected_escrow_credits_the_amount_actually_received() {
    let t = run(
        EscrowFlavor::Corrected,
        Some((200, u64::MAX)),
        10_000,
        vec![DEP],
    );
    let s = &t.steps[0];
    assert_eq!(s.status, StepStatus::Ok, "logs: {:?}", s.logs);
    assert_eq!(s.after.escrow_recorded, 980);
    assert_eq!(s.after.vault_amount, 980);
}

#[test]
fn without_a_fee_both_flavors_agree() {
    for f in [EscrowFlavor::Vulnerable, EscrowFlavor::Corrected] {
        let t = run(f, None, 10_000, vec![DEP]);
        assert_eq!(t.steps[0].after.escrow_recorded, 1_000);
        assert_eq!(t.steps[0].after.vault_amount, 1_000);
    }
}

#[test]
fn vulnerable_withdraw_of_over_credited_balance_fails_and_state_is_unchanged() {
    let t = run(
        EscrowFlavor::Vulnerable,
        Some((200, u64::MAX)),
        10_000,
        vec![DEP, Action::Withdraw],
    );
    let w = &t.steps[1];
    assert!(
        matches!(w.status, StepStatus::Failed { .. }),
        "withdraw of 1000 from a vault of 980 must fail"
    );
    assert_eq!(w.before, w.after, "failed instruction persists nothing");
    assert_eq!(w.after.escrow_recorded, 1_000);
}

#[test]
fn corrected_withdraw_drains_the_vault() {
    let t = run(
        EscrowFlavor::Corrected,
        Some((200, u64::MAX)),
        10_000,
        vec![DEP, Action::Withdraw],
    );
    let w = &t.steps[1];
    assert_eq!(w.status, StepStatus::Ok, "logs: {:?}", w.logs);
    assert_eq!(w.after.vault_amount, 0);
    assert_eq!(w.after.escrow_recorded, 0);
    // The withdrawal is itself a fee-bearing transfer: 980 -> fee ceil(980*2%) = 20 -> 960 arrives.
    assert_eq!(w.after.destination_balance, 960);
}

#[test]
fn deposit_beyond_balance_is_an_execution_failure_with_no_state_change() {
    for f in [EscrowFlavor::Vulnerable, EscrowFlavor::Corrected] {
        let t = run(f, Some((200, u64::MAX)), 10, vec![DEP]);
        let s = &t.steps[0];
        assert!(matches!(s.status, StepStatus::Failed { .. }));
        assert_eq!(s.before, s.after);
    }
}

#[test]
fn tiny_deposit_fully_eaten_by_fee_exposes_the_bug_at_the_boundary() {
    // 1 unit at 1 bps: fee rounds up to 1, vault receives 0, vulnerable still credits 1.
    let v = run(
        EscrowFlavor::Vulnerable,
        Some((1, u64::MAX)),
        10,
        vec![Action::Deposit { amount: 1 }],
    );
    assert_eq!(
        (
            v.steps[0].after.escrow_recorded,
            v.steps[0].after.vault_amount
        ),
        (1, 0)
    );
    let c = run(
        EscrowFlavor::Corrected,
        Some((1, u64::MAX)),
        10,
        vec![Action::Deposit { amount: 1 }],
    );
    assert_eq!(
        (
            c.steps[0].after.escrow_recorded,
            c.steps[0].after.vault_amount
        ),
        (0, 0)
    );
}

#[test]
fn wrong_target_and_missing_program_are_setup_errors() {
    let a = EscrowAdapter::new(EscrowFlavor::Vulnerable);
    let err = a
        .run(&scenario("demo-escrow-corrected", None, 1, vec![DEP]))
        .unwrap_err();
    assert!(matches!(err, ExecError::Setup { ref step, .. } if step == "select-target"));

    let missing = EscrowAdapter::with_program_dir(EscrowFlavor::Vulnerable, "/nonexistent-dir");
    let err = missing
        .run(&scenario("demo-escrow-vulnerable", None, 1, vec![DEP]))
        .unwrap_err();
    assert!(matches!(err, ExecError::Setup { ref step, .. } if step == "load-program"));
}

#[test]
fn runs_are_deterministic() {
    let a = run(
        EscrowFlavor::Vulnerable,
        Some((200, u64::MAX)),
        10_000,
        vec![DEP, Action::Withdraw],
    );
    let b = run(
        EscrowFlavor::Vulnerable,
        Some((200, u64::MAX)),
        10_000,
        vec![DEP, Action::Withdraw],
    );
    assert_eq!(a, b);
}

#[test]
fn frozen_source_account_rejects_deposits_atomically_until_thawed() {
    for flavor in [EscrowFlavor::Vulnerable, EscrowFlavor::Corrected] {
        let t = run(
            flavor,
            Some((200, u64::MAX)),
            10_000,
            vec![Action::FreezeDepositor, DEP, Action::ThawDepositor, DEP],
        );
        assert_eq!(t.steps[0].status, StepStatus::Ok);
        assert!(
            t.steps[0].after.depositor_frozen,
            "freeze decoded from real account state"
        );
        let rejected = &t.steps[1];
        assert!(
            matches!(rejected.status, StepStatus::Failed { .. }),
            "{flavor:?}"
        );
        assert!(
            rejected
                .logs
                .iter()
                .any(|l| l.to_lowercase().contains("frozen")),
            "{:?}",
            rejected.logs
        );
        assert_eq!(
            rejected.before, rejected.after,
            "failed deposit persisted nothing"
        );
        assert!(!t.steps[2].after.depositor_frozen);
        assert_eq!(
            t.steps[3].status,
            StepStatus::Ok,
            "deposit works after thaw"
        );
        assert_eq!(t.steps[3].after.vault_amount, 980);
    }
}

#[test]
fn unauthorized_withdrawal_is_rejected_by_both_flavors_and_changes_nothing() {
    for flavor in [EscrowFlavor::Vulnerable, EscrowFlavor::Corrected] {
        let t = run(
            flavor,
            None,
            10_000,
            vec![DEP, Action::UnauthorizedWithdraw],
        );
        let a = &t.steps[1];
        match &a.status {
            StepStatus::Failed { error } => assert!(
                error.contains("Custom(5)"),
                "expected EscrowError::Unauthorized, got {error}"
            ),
            StepStatus::Ok => panic!("unauthorized withdrawal must not succeed on {flavor:?}"),
        }
        assert_eq!(a.before, a.after);
        assert_eq!(a.after.attacker_balance, 0);
        assert_eq!(a.after.vault_amount, 1_000);
    }
}
