//! Replay, shrinking and corpus behavior against real SVM execution.

use tokenscope_core::{
    Engine,
    corpus::{Corpus, CorpusError, replay_case},
};
use tokenscope_model::{
    Action, CorpusCase, Expectation, Finding, MintSpec, OutcomeClass, SCHEMA_VERSION, Scenario,
    TransferFeeParams,
};
use tokenscope_shrinker::ShrinkConfig;

const VULN: &str = "demo-escrow-vulnerable";
const FIXED: &str = "demo-escrow-corrected";

fn noisy(target: &str) -> Scenario {
    Scenario {
        schema_version: SCHEMA_VERSION,
        name: "noisy".into(),
        seed: 12,
        target: target.into(),
        mint: MintSpec {
            decimals: 6,
            transfer_fee: Some(TransferFeeParams {
                basis_points: 777,
                maximum_fee: 123_456,
            }),
        },
        initial_balance: 9_999_999,
        extensions: vec![],
        actions: vec![
            Action::Withdraw,
            Action::Deposit { amount: 50_000 },
            Action::Deposit { amount: 3 },
            Action::Withdraw,
            Action::Deposit { amount: 4_000 },
        ],
    }
}

fn acct_finding(engine: &Engine) -> Finding {
    let run = engine.run_scenario(&noisy(VULN));
    run.findings
        .into_iter()
        .find(|f| f.invariant_id == "INV-ACCT-001")
        .expect("vulnerable run must violate INV-ACCT-001")
}

#[test]
fn replay_reproduces_a_real_finding_and_not_on_the_corrected_target() {
    let engine = Engine::new();
    let f = acct_finding(&engine);
    let replay = engine.replay_finding(&f);
    assert!(replay.reproduced);
    assert_eq!(replay.run.outcome, OutcomeClass::InvariantViolation);

    let mut fixed = f.clone();
    fixed.reproduction.scenario.target = FIXED.into();
    assert!(!engine.replay_finding(&fixed).reproduced);
}

#[test]
fn replay_is_deterministic() {
    let engine = Engine::new();
    let f = acct_finding(&engine);
    assert_eq!(engine.replay_finding(&f), engine.replay_finding(&f));
}

#[test]
fn minimization_reaches_the_smallest_failing_scenario_and_preserves_the_failure() {
    let engine = Engine::new();
    let f = acct_finding(&engine);
    let (minimized_finding, report) = engine.minimize(&f, ShrinkConfig::default()).unwrap();
    let m = minimized_finding
        .minimized
        .clone()
        .expect("minimized scenario recorded");

    // The smallest scenario that still loses tokens to the fee: 1 unit at 1 bp with a cap of 1.
    assert_eq!(m.actions, vec![Action::Deposit { amount: 1 }]);
    assert_eq!(m.initial_balance, 1);
    assert_eq!(m.mint.decimals, 0);
    assert_eq!(
        m.mint.transfer_fee,
        Some(TransferFeeParams {
            basis_points: 1,
            maximum_fee: 1
        })
    );
    assert!(!report.budget_exhausted, "{report:?}");

    // Same failure predicate holds on the minimized scenario (real execution)...
    let again = engine.run_scenario(&m);
    assert!(
        again
            .findings
            .iter()
            .any(|x| x.invariant_id == "INV-ACCT-001")
    );
    // ...and the corrected escrow passes the very same minimized scenario.
    let mut on_fixed = m.clone();
    on_fixed.target = FIXED.into();
    assert_eq!(engine.run_scenario(&on_fixed).outcome, OutcomeClass::Pass);
    // The original artifact is untouched.
    assert_eq!(
        minimized_finding.reproduction.scenario,
        f.reproduction.scenario
    );
}

#[test]
fn minimizing_a_finding_that_no_longer_reproduces_is_an_error() {
    let engine = Engine::new();
    let mut f = acct_finding(&engine);
    f.reproduction.scenario.target = FIXED.into();
    assert!(engine.minimize(&f, ShrinkConfig::default()).is_err());
}

fn case(target: &str, expect: Expectation) -> CorpusCase {
    let mut s = noisy(target);
    s.actions = vec![Action::Deposit { amount: 1_000 }];
    CorpusCase::new("test case", s, expect)
}

#[test]
fn corpus_round_trips_and_replays_against_real_execution() {
    let dir = tempfile::tempdir().unwrap();
    let corpus = Corpus::open(dir.path().join("corpus"));
    assert!(
        corpus.list().unwrap().is_empty(),
        "missing directory is an empty corpus"
    );

    let bad = case(
        VULN,
        Expectation {
            outcome: OutcomeClass::InvariantViolation,
            violated: vec!["INV-ACCT-001".into(), "INV-SOLV-001".into()],
        },
    );
    let good = case(
        FIXED,
        Expectation {
            outcome: OutcomeClass::Pass,
            violated: vec![],
        },
    );
    corpus.add(&bad).unwrap();
    corpus.add(&good).unwrap();

    let listed = corpus.list().unwrap();
    assert_eq!(listed.len(), 2);
    assert!(listed.windows(2).all(|w| w[0].id < w[1].id), "sorted by id");
    assert_eq!(corpus.get(&bad.id).unwrap(), bad);

    let engine = Engine::new();
    for c in &listed {
        let r = replay_case(&engine, c);
        assert!(r.ok, "{r:?}");
    }
}

#[test]
fn replay_detects_a_regression() {
    // Expectation claims the vulnerable escrow passes; replay must say it does not.
    let wrong = case(
        VULN,
        Expectation {
            outcome: OutcomeClass::Pass,
            violated: vec![],
        },
    );
    let r = replay_case(&Engine::new(), &wrong);
    assert!(!r.ok);
    assert_eq!(r.actual.outcome, OutcomeClass::InvariantViolation);
}

#[test]
fn corpus_reports_bad_files_and_missing_cases() {
    let dir = tempfile::tempdir().unwrap();
    let corpus = Corpus::open(dir.path());
    assert!(matches!(
        corpus.get("rc_nope"),
        Err(CorpusError::NotFound(_))
    ));
    std::fs::write(dir.path().join("README.txt"), "ignored").unwrap();
    assert!(corpus.list().unwrap().is_empty());
    std::fs::write(dir.path().join("broken.json"), "{not json").unwrap();
    assert!(matches!(corpus.list(), Err(CorpusError::Invalid { .. })));
}
