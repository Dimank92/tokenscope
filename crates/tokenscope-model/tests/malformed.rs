//! Malformed-input robustness for every persisted-format parser.
//!
//! A coverage-guided fuzz harness (cargo-fuzz) needs a nightly toolchain, which this project does
//! not pin; instead these property tests feed arbitrary text and *mutated valid documents*
//! (truncation, byte replacement, byte deletion) to each parser and require that it never panics.
//! Whatever it accepts must also pass validation, i.e. be internally consistent.

use proptest::prelude::*;
use tokenscope_model::{
    Action, Campaign, CorpusCase, Expectation, Finding, MintSpec, OutcomeClass, Reproduction,
    RunRecord, SCHEMA_VERSION, Scenario, Severity, StateDelta, TransferFeeParams,
};

fn scenario() -> Scenario {
    Scenario {
        schema_version: SCHEMA_VERSION,
        name: "s".into(),
        seed: 9,
        target: "demo-escrow-vulnerable".into(),
        mint: MintSpec {
            decimals: 6,
            transfer_fee: Some(TransferFeeParams {
                basis_points: 200,
                maximum_fee: 500,
            }),
        },
        initial_balance: 100,
        extensions: vec![],
        actions: vec![Action::Deposit { amount: 10 }, Action::UnauthorizedWithdraw],
    }
}

fn finding() -> Finding {
    let s = scenario();
    let sid = s.id();
    Finding {
        schema_version: SCHEMA_VERSION,
        id: Finding::compute_id(&s.target, &sid, "INV-ACCT-001"),
        target: s.target.clone(),
        scenario_id: sid,
        seed: 9,
        invariant_id: "INV-ACCT-001".into(),
        class: OutcomeClass::InvariantViolation,
        expected: "e".into(),
        observed: "o".into(),
        deltas: vec![StateDelta {
            label: "l".into(),
            account: "a".into(),
            before: 1,
            after: 2,
        }],
        logs: vec!["log".into()],
        severity: Severity::Low,
        severity_rationale: "r".into(),
        reproduction: Reproduction {
            command: "c".into(),
            scenario: s,
        },
        minimized: None,
    }
}

fn campaign() -> Campaign {
    let s = scenario();
    Campaign::new(
        s.target.clone(),
        1,
        vec![RunRecord {
            scenario_id: s.id(),
            scenario: s,
            outcome: OutcomeClass::Pass,
            setup_error: None,
            trace: None,
            findings: vec![],
        }],
    )
}

fn corpus_case() -> CorpusCase {
    CorpusCase::new(
        "d",
        scenario(),
        Expectation {
            outcome: OutcomeClass::Pass,
            violated: vec![],
        },
    )
}

fn mutate(doc: &str, kind: u8, at: usize, byte: u8) -> String {
    let mut b = doc.as_bytes().to_vec();
    let i = at % b.len();
    match kind % 3 {
        0 => b.truncate(i),
        1 => b[i] = byte,
        _ => {
            b.remove(i);
        }
    }
    String::from_utf8_lossy(&b).into_owned()
}

fn check_all(input: &str) {
    if let Ok(s) = Scenario::from_json(input) {
        assert!(s.validate().is_ok());
    }
    if let Ok(f) = Finding::from_json(input) {
        assert!(f.validate().is_ok());
    }
    if let Ok(c) = Campaign::from_json(input) {
        assert_eq!(c.schema_version, SCHEMA_VERSION);
    }
    if let Ok(c) = CorpusCase::from_json(input) {
        assert!(c.validate().is_ok());
    }
}

proptest! {
    #[test]
    fn arbitrary_text_never_panics_any_parser(input in ".{0,300}") {
        check_all(&input);
    }

    #[test]
    fn mutated_valid_documents_never_panic(kind in any::<u8>(), at in any::<usize>(), byte in any::<u8>()) {
        for doc in [scenario().to_json(), finding().to_json(), campaign().to_json(), corpus_case().to_json()] {
            check_all(&mutate(&doc, kind, at, byte));
        }
    }
}

#[test]
fn unmutated_documents_round_trip() {
    assert_eq!(
        Scenario::from_json(&scenario().to_json()).unwrap(),
        scenario()
    );
    assert_eq!(Finding::from_json(&finding().to_json()).unwrap(), finding());
    assert_eq!(
        Campaign::from_json(&campaign().to_json()).unwrap(),
        campaign()
    );
    assert_eq!(
        CorpusCase::from_json(&corpus_case().to_json()).unwrap(),
        corpus_case()
    );
}

#[test]
fn unknown_action_kinds_are_rejected_not_ignored() {
    let json = scenario()
        .to_json()
        .replace("unauthorized_withdraw", "drain_everything");
    assert!(Scenario::from_json(&json).is_err());
}
