//! Regression test for the reproduced transfer-fee failure, driven by the committed corpus in
//! `fixtures/corpus` (created with `tokenscope corpus add`).

use std::path::Path;

use tokenscope_core::{
    Engine,
    corpus::{Corpus, replay_case},
};
use tokenscope_model::{
    Action, MintSpec, OutcomeClass, SCHEMA_VERSION, Scenario, TransferFeeParams,
};
use tokenscope_shrinker::ShrinkConfig;

fn corpus() -> Corpus {
    Corpus::open(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/corpus"))
}

#[test]
fn every_committed_case_replays_as_expected() {
    let cases = corpus().list().unwrap();
    assert!(
        cases.len() >= 2,
        "expected the vulnerable and corrected regression cases"
    );
    let engine = Engine::new();
    for c in &cases {
        let r = replay_case(&engine, c);
        assert!(
            r.ok,
            "regression in {}: expected {:?}, got {:?}",
            c.id, c.expect, r.actual
        );
    }
}

#[test]
fn corpus_pins_the_bug_on_vulnerable_and_the_fix_on_corrected() {
    let cases = corpus().list().unwrap();
    let vuln = cases
        .iter()
        .find(|c| c.scenario.target == "demo-escrow-vulnerable")
        .unwrap();
    let fixed = cases
        .iter()
        .find(|c| c.scenario.target == "demo-escrow-corrected")
        .unwrap();
    assert_eq!(vuln.expect.outcome, OutcomeClass::InvariantViolation);
    assert!(vuln.expect.violated.contains(&"INV-ACCT-001".to_string()));
    assert_eq!(fixed.expect.outcome, OutcomeClass::Pass);
    // Same scenario content except for the target.
    let mut retargeted = vuln.scenario.clone();
    retargeted.target = fixed.scenario.target.clone();
    assert_eq!(retargeted, fixed.scenario);
}

#[test]
fn committed_minimized_case_is_what_minimization_derives_from_the_noisy_original() {
    let noisy = Scenario {
        schema_version: SCHEMA_VERSION,
        name: "noisy".into(),
        seed: 12,
        target: "demo-escrow-vulnerable".into(),
        mint: MintSpec {
            decimals: 6,
            transfer_fee: Some(TransferFeeParams {
                basis_points: 777,
                maximum_fee: 123_456,
            }),
        },
        initial_balance: 9_999_999,
        actions: vec![
            Action::Withdraw,
            Action::Deposit { amount: 50_000 },
            Action::Deposit { amount: 3 },
            Action::Withdraw,
            Action::Deposit { amount: 4_000 },
        ],
    };
    let engine = Engine::new();
    let finding = engine
        .run_scenario(&noisy)
        .findings
        .into_iter()
        .find(|f| f.invariant_id == "INV-ACCT-001")
        .unwrap();
    let (minimized, _) = engine.minimize(&finding, ShrinkConfig::default()).unwrap();
    let committed = corpus()
        .list()
        .unwrap()
        .into_iter()
        .find(|c| c.scenario.target == "demo-escrow-vulnerable")
        .unwrap();
    assert_eq!(minimized.minimized.unwrap(), committed.scenario);
}
