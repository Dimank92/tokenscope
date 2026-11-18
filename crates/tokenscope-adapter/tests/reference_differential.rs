//! Differential testing between the independent TokenReferenceModel and real Mollusk SVM execution.
//!
//! Asserts that state transitions predicted by the pure reference model agree with decoded
//! on-chain account data on real Solana VM execution.

use tokenscope_adapter::{EscrowAdapter, EscrowFlavor};
use tokenscope_model::{
    Action, MintSpec, Scenario, StepStatus, TokenReferenceModel, TransferFeeParams,
};

#[test]
fn differential_test_corrected_escrow_matches_reference_model() {
    let scenario = Scenario {
        schema_version: 1,
        name: "diff-corrected-test".into(),
        seed: 42,
        target: "demo-escrow-corrected".into(),
        mint: MintSpec {
            decimals: 2,
            transfer_fee: Some(TransferFeeParams {
                basis_points: 250, // 2.5% fee
                maximum_fee: 100,
            }),
        },
        initial_balance: 10_000,
        extensions: vec![],
        actions: vec![
            Action::Deposit { amount: 1_000 },
            Action::Deposit { amount: 2_000 },
            Action::Withdraw,
        ],
    };

    let adapter = EscrowAdapter::new(EscrowFlavor::Corrected);
    let trace = adapter.run(&scenario).expect("run should succeed");

    let mut model = TokenReferenceModel::from_scenario(&scenario);

    assert_eq!(trace.steps.len(), 3);
    for step in &trace.steps {
        assert_eq!(step.status, StepStatus::Ok);
        let ref_snap = model
            .step(step.action, true)
            .expect("reference model step should succeed");

        assert_eq!(
            step.after.depositor_balance, ref_snap.depositor_balance,
            "depositor balance mismatch"
        );
        assert_eq!(
            step.after.vault_amount, ref_snap.vault_amount,
            "vault amount mismatch"
        );
        assert_eq!(
            step.after.vault_withheld, ref_snap.vault_withheld,
            "vault withheld fee mismatch"
        );
        assert_eq!(
            step.after.escrow_recorded, ref_snap.escrow_recorded,
            "escrow recorded balance mismatch"
        );
        assert_eq!(
            step.after.destination_balance, ref_snap.destination_balance,
            "destination balance mismatch"
        );
    }
}

#[test]
fn differential_test_vulnerable_escrow_diverges_from_reference_model() {
    let scenario = Scenario {
        schema_version: 1,
        name: "diff-vulnerable-test".into(),
        seed: 42,
        target: "demo-escrow-vulnerable".into(),
        mint: MintSpec {
            decimals: 2,
            transfer_fee: Some(TransferFeeParams {
                basis_points: 250, // 2.5% fee
                maximum_fee: 100,
            }),
        },
        initial_balance: 10_000,
        extensions: vec![],
        actions: vec![Action::Deposit { amount: 1_000 }],
    };

    let adapter = EscrowAdapter::new(EscrowFlavor::Vulnerable);
    let trace = adapter.run(&scenario).expect("run should succeed");

    let mut model = TokenReferenceModel::from_scenario(&scenario);
    let ref_snap = model
        .step(Action::Deposit { amount: 1_000 }, true) // true = reference expected
        .expect("reference model step should succeed");

    let step = &trace.steps[0];
    assert_eq!(step.status, StepStatus::Ok);

    // Physical balances match the reference model
    assert_eq!(step.after.depositor_balance, ref_snap.depositor_balance);
    assert_eq!(step.after.vault_amount, ref_snap.vault_amount);
    assert_eq!(step.after.vault_withheld, ref_snap.vault_withheld);

    // BUT the recorded liability diverges: vulnerable recorded 1000, reference says 975
    assert_ne!(step.after.escrow_recorded, ref_snap.escrow_recorded);
    assert_eq!(step.after.escrow_recorded, 1_000);
    assert_eq!(ref_snap.escrow_recorded, 975);
}

#[test]
fn differential_test_freeze_and_atomicity_match_svm() {
    let scenario = Scenario {
        schema_version: 1,
        name: "diff-freeze-atomicity".into(),
        seed: 7,
        target: "demo-escrow-corrected".into(),
        mint: MintSpec {
            decimals: 0,
            transfer_fee: None,
        },
        initial_balance: 5_000,
        extensions: vec![],
        actions: vec![
            Action::FreezeDepositor,
            Action::Deposit { amount: 500 }, // should fail because frozen
            Action::ThawDepositor,
            Action::Deposit { amount: 500 }, // should succeed now
        ],
    };

    let adapter = EscrowAdapter::new(EscrowFlavor::Corrected);
    let trace = adapter.run(&scenario).expect("run should succeed");

    let mut model = TokenReferenceModel::from_scenario(&scenario);

    // Step 0: Freeze
    assert_eq!(trace.steps[0].status, StepStatus::Ok);
    let snap0 = model.step(trace.steps[0].action, true).unwrap();
    assert_eq!(
        trace.steps[0].after.depositor_frozen,
        snap0.depositor_frozen
    );
    assert!(snap0.depositor_frozen);

    // Step 1: Deposit while frozen -> failed on SVM
    assert!(matches!(trace.steps[1].status, StepStatus::Failed { .. }));
    let res = model.step(trace.steps[1].action, true);
    assert!(res.is_err());
    // Failure leaves state unchanged (atomic rollback)
    assert_eq!(trace.steps[1].after, trace.steps[1].before);
    assert_eq!(model.snapshot(), snap0);

    // Step 2: Thaw
    assert_eq!(trace.steps[2].status, StepStatus::Ok);
    let snap2 = model.step(trace.steps[2].action, true).unwrap();
    assert_eq!(
        trace.steps[2].after.depositor_frozen,
        snap2.depositor_frozen
    );
    assert!(!snap2.depositor_frozen);

    // Step 3: Deposit succeeds
    assert_eq!(trace.steps[3].status, StepStatus::Ok);
    let snap3 = model.step(trace.steps[3].action, true).unwrap();
    assert_eq!(trace.steps[3].after.vault_amount, snap3.vault_amount);
    assert_eq!(snap3.vault_amount, 500);
}
