use tokenscope_model::{
    AccountRef, Action, ExecutionTrace, MintSpec, OutcomeClass, SCHEMA_VERSION, Scenario, Snapshot,
    StepRecord, StepStatus,
};

use crate::*;

fn snap(vault: u64, withheld: Option<u64>, recorded: u64, depositor: u64) -> Snapshot {
    Snapshot {
        vault_amount: vault,
        vault_withheld: withheld,
        escrow_recorded: recorded,
        depositor_balance: depositor,
        destination_balance: 0,
    }
}

fn step(index: usize, action: Action, before: Snapshot, after: Snapshot, ok: bool) -> StepRecord {
    StepRecord {
        index,
        action,
        before,
        after,
        status: if ok {
            StepStatus::Ok
        } else {
            StepStatus::Failed { error: "e".into() }
        },
        logs: vec!["log".into()],
        compute_units: 1,
    }
}

fn trace(steps: Vec<StepRecord>) -> ExecutionTrace {
    let accounts = ["escrow_state", "vault", "depositor_source"]
        .iter()
        .map(|l| AccountRef {
            label: (*l).into(),
            address: format!("addr-{l}"),
        })
        .collect();
    let mut t = ExecutionTrace::new(
        "sc_x".into(),
        "demo-escrow-vulnerable".into(),
        snap(0, Some(0), 0, 10_000),
        accounts,
    );
    t.steps = steps;
    t
}

fn scenario() -> Scenario {
    Scenario {
        schema_version: SCHEMA_VERSION,
        name: "s".into(),
        seed: 5,
        target: "demo-escrow-vulnerable".into(),
        mint: MintSpec {
            decimals: 6,
            transfer_fee: None,
        },
        initial_balance: 10_000,
        actions: vec![Action::Deposit { amount: 1_000 }],
    }
}

const DEP: Action = Action::Deposit { amount: 1_000 };

fn verdicts(t: &ExecutionTrace, id: &str) -> Vec<Verdict> {
    evaluate(t)
        .into_iter()
        .filter(|e| e.invariant_id == id)
        .map(|e| e.verdict)
        .collect()
}

/// Deposit of 1000 at 2%: vault gets 980, withheld 20, depositor -1000.
fn vulnerable_deposit() -> StepRecord {
    step(
        0,
        DEP,
        snap(0, Some(0), 0, 10_000),
        snap(980, Some(20), 1_000, 9_000),
        true,
    )
}

fn corrected_deposit() -> StepRecord {
    step(
        0,
        DEP,
        snap(0, Some(0), 0, 10_000),
        snap(980, Some(20), 980, 9_000),
        true,
    )
}

#[test]
fn acct_001_flags_vulnerable_and_passes_corrected() {
    assert!(matches!(
        verdicts(&trace(vec![vulnerable_deposit()]), "INV-ACCT-001")[0],
        Verdict::Violated { .. }
    ));
    assert_eq!(
        verdicts(&trace(vec![corrected_deposit()]), "INV-ACCT-001"),
        vec![Verdict::Held]
    );
}

#[test]
fn solv_001_detects_shortfall() {
    match &verdicts(&trace(vec![vulnerable_deposit()]), "INV-SOLV-001")[0] {
        Verdict::Violated { observed } => {
            assert!(observed.contains("shortfall of 20"), "{observed}")
        }
        v => panic!("expected violation, got {v:?}"),
    }
    assert_eq!(
        verdicts(&trace(vec![corrected_deposit()]), "INV-SOLV-001"),
        vec![Verdict::Held]
    );
}

#[test]
fn conservation_holds_for_both_flavors_because_the_fee_is_accounted_for() {
    for s in [vulnerable_deposit(), corrected_deposit()] {
        assert_eq!(
            verdicts(&trace(vec![s]), "INV-CONS-001"),
            vec![Verdict::Held]
        );
    }
}

#[test]
fn conservation_flags_missing_tokens() {
    let bad = step(
        0,
        DEP,
        snap(0, Some(0), 0, 10_000),
        snap(900, Some(20), 900, 9_000),
        true,
    );
    assert!(matches!(
        verdicts(&trace(vec![bad]), "INV-CONS-001")[0],
        Verdict::Violated { .. }
    ));
}

#[test]
fn no_fee_extension_counts_as_zero_withheld() {
    let s = step(
        0,
        DEP,
        snap(0, None, 0, 10_000),
        snap(1_000, None, 1_000, 9_000),
        true,
    );
    let t = trace(vec![s]);
    for id in ["INV-ACCT-001", "INV-SOLV-001", "INV-CONS-001"] {
        assert_eq!(verdicts(&t, id), vec![Verdict::Held], "{id}");
    }
}

#[test]
fn atomicity_applies_only_to_failed_steps() {
    let before = snap(980, Some(20), 1_000, 9_000);
    let clean = step(1, Action::Withdraw, before, before, false);
    assert_eq!(
        verdicts(&trace(vec![clean]), "INV-ATOM-001"),
        vec![Verdict::Held]
    );

    let partial = step(
        1,
        Action::Withdraw,
        before,
        snap(0, Some(20), 1_000, 9_000),
        false,
    );
    assert!(matches!(
        verdicts(&trace(vec![partial]), "INV-ATOM-001")[0],
        Verdict::Violated { .. }
    ));

    assert_eq!(
        verdicts(&trace(vec![vulnerable_deposit()]), "INV-ATOM-001"),
        vec![Verdict::NotApplicable]
    );
}

#[test]
fn failed_steps_are_not_applicable_to_accounting_invariants() {
    let b = snap(0, Some(0), 0, 10);
    let failed = step(0, DEP, b, b, false);
    let t = trace(vec![failed]);
    for id in ["INV-ACCT-001", "INV-SOLV-001", "INV-CONS-001"] {
        assert_eq!(verdicts(&t, id), vec![Verdict::NotApplicable], "{id}");
    }
}

#[test]
fn classification_separates_pass_execution_failure_and_violation() {
    let ok = trace(vec![corrected_deposit()]);
    assert_eq!(classify(&ok, &evaluate(&ok)), OutcomeClass::Pass);

    let b = snap(0, Some(0), 0, 10);
    let failed = trace(vec![step(0, DEP, b, b, false)]);
    assert_eq!(
        classify(&failed, &evaluate(&failed)),
        OutcomeClass::ExecutionFailure
    );

    let bad = trace(vec![vulnerable_deposit()]);
    assert_eq!(
        classify(&bad, &evaluate(&bad)),
        OutcomeClass::InvariantViolation
    );
}

#[test]
fn findings_carry_evidence_and_validate() {
    let t = trace(vec![vulnerable_deposit()]);
    let f = findings(&scenario(), &t, &evaluate(&t));
    let ids: Vec<_> = f.iter().map(|x| x.invariant_id.as_str()).collect();
    assert_eq!(ids, ["INV-ACCT-001", "INV-SOLV-001"]);
    for finding in &f {
        finding.validate().unwrap();
        assert_eq!(finding.class, OutcomeClass::InvariantViolation);
        assert_eq!(finding.seed, 5);
        assert_eq!(finding.logs, vec!["log".to_string()]);
        assert!(
            finding
                .severity_rationale
                .to_lowercase()
                .contains("exploitab")
        );
        let vault = finding
            .deltas
            .iter()
            .find(|d| d.label == "vault_amount")
            .unwrap();
        assert_eq!(
            (vault.before, vault.after, vault.account.as_str()),
            (0, 980, "addr-vault")
        );
    }
}

#[test]
fn one_finding_per_invariant_even_if_violated_repeatedly() {
    let second = step(
        1,
        DEP,
        snap(980, Some(20), 1_000, 9_000),
        snap(1_960, Some(40), 2_000, 8_000),
        true,
    );
    let t = trace(vec![vulnerable_deposit(), second]);
    let f = findings(&scenario(), &t, &evaluate(&t));
    assert_eq!(f.len(), 2);
    assert!(f[0].observed.starts_with("step 0"));
}

#[test]
fn catalog_ids_are_unique_and_documented() {
    let c = catalog();
    for (i, a) in c.iter().enumerate() {
        assert!(!a.preconditions.is_empty() && !a.exceptions.is_empty() && !a.expected.is_empty());
        assert!(c.iter().skip(i + 1).all(|b| b.id != a.id));
    }
}
