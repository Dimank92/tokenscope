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
        ..Snapshot::default()
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
        raw_before: None,
        raw_after: None,
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
        extensions: vec![],
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

fn unauthorized(status_ok: bool, before: Snapshot, after: Snapshot) -> StepRecord {
    step(1, Action::UnauthorizedWithdraw, before, after, status_ok)
}

#[test]
fn auth_001_holds_when_the_attack_is_rejected_without_state_change() {
    let b = snap(1_000, None, 1_000, 9_000);
    let t = trace(vec![unauthorized(false, b, b)]);
    assert_eq!(verdicts(&t, "INV-AUTH-001"), vec![Verdict::Held]);
    // A rejected, expected-to-fail attack is not an execution failure.
    assert_eq!(classify(&t, &evaluate(&t)), OutcomeClass::Pass);
}

#[test]
fn auth_001_flags_a_successful_unauthorized_withdrawal() {
    let before = snap(1_000, None, 1_000, 9_000);
    let mut after = snap(0, None, 0, 9_000);
    after.attacker_balance = 1_000;
    let t = trace(vec![unauthorized(true, before, after)]);
    match &verdicts(&t, "INV-AUTH-001")[0] {
        Verdict::Violated { observed } => assert!(
            observed.contains("attacker balance 0 -> 1000"),
            "{observed}"
        ),
        v => panic!("expected violation, got {v:?}"),
    }
    assert_eq!(
        classify(&t, &evaluate(&t)),
        OutcomeClass::InvariantViolation
    );
}

#[test]
fn auth_001_flags_a_rejection_that_still_changed_state() {
    let before = snap(1_000, None, 1_000, 9_000);
    let after = snap(1_000, None, 0, 9_000);
    let t = trace(vec![unauthorized(false, before, after)]);
    assert!(matches!(
        verdicts(&t, "INV-AUTH-001")[0],
        Verdict::Violated { .. }
    ));
}

#[test]
fn auth_001_is_not_applicable_to_other_actions() {
    assert_eq!(
        verdicts(&trace(vec![corrected_deposit()]), "INV-AUTH-001"),
        vec![Verdict::NotApplicable]
    );
}

#[test]
fn ordinary_failed_steps_still_classify_as_execution_failures() {
    let b = snap(0, None, 0, 10);
    let t = trace(vec![step(0, DEP, b, b, false)]);
    assert_eq!(classify(&t, &evaluate(&t)), OutcomeClass::ExecutionFailure);
}

fn step_of(action: Action, ok: bool, before: Snapshot, after: Snapshot) -> StepRecord {
    step(0, action, before, after, ok)
}

#[test]
fn cust_001_flags_a_vault_decrease_outside_withdraw() {
    let before = snap(1_000, None, 1_000, 9_000);
    let mut after = before;
    after.vault_amount = 400;
    after.issuer_balance = 600;
    let drain = step_of(
        Action::PermanentDelegateDrain { amount: 600 },
        true,
        before,
        after,
    );
    match &verdicts(&trace(vec![drain]), "INV-CUST-001")[0] {
        Verdict::Violated { observed } => assert!(observed.contains("1000 to 400"), "{observed}"),
        v => panic!("expected violation, got {v:?}"),
    }
}

#[test]
fn cust_001_excludes_withdraw_and_failed_steps_and_holds_for_growth() {
    let before = snap(1_000, None, 1_000, 9_000);
    let drained = snap(0, None, 0, 9_000);
    let w = step_of(Action::Withdraw, true, before, drained);
    assert_eq!(
        verdicts(&trace(vec![w]), "INV-CUST-001"),
        vec![Verdict::NotApplicable]
    );
    let failed = step_of(
        Action::PermanentDelegateDrain { amount: 5 },
        false,
        before,
        before,
    );
    assert_eq!(
        verdicts(&trace(vec![failed]), "INV-CUST-001"),
        vec![Verdict::NotApplicable]
    );
    assert_eq!(
        verdicts(&trace(vec![corrected_deposit()]), "INV-CUST-001"),
        vec![Verdict::Held]
    );
}

#[test]
fn auth_001_covers_every_must_reject_action() {
    use tokenscope_model::Mismatch;
    let b = snap(1_000, None, 1_000, 9_000);
    let must_reject = [
        Action::UnauthorizedWithdraw,
        Action::MismatchedDeposit {
            mismatch: Mismatch::WrongVault,
            amount: 5,
        },
        Action::MismatchedWithdraw {
            mismatch: Mismatch::WrongAuthority,
        },
        Action::ReinitEscrow,
    ];
    for a in must_reject {
        assert!(a.expects_rejection());
        let rejected = trace(vec![step_of(a, false, b, b)]);
        assert_eq!(
            verdicts(&rejected, "INV-AUTH-001"),
            vec![Verdict::Held],
            "{a:?}"
        );
        assert_eq!(
            classify(&rejected, &evaluate(&rejected)),
            OutcomeClass::Pass,
            "{a:?}"
        );
        let mut changed = b;
        changed.escrow_recorded = 0;
        let leaked = trace(vec![step_of(a, true, b, changed)]);
        assert!(
            matches!(
                verdicts(&leaked, "INV-AUTH-001")[0],
                Verdict::Violated { .. }
            ),
            "{a:?}"
        );
    }
    assert!(!Action::Deposit { amount: 1 }.expects_rejection());
}

#[test]
fn evaluations_use_step_position_not_the_self_reported_index() {
    // Regression found by fuzzing: a trace whose step `index` fields do not match their positions
    // used to make `findings` index out of bounds.
    let mut t = trace(vec![vulnerable_deposit(), vulnerable_deposit()]);
    t.steps[0].index = 99;
    t.steps[1].index = 7;
    let evals = evaluate(&t);
    let positions: std::collections::BTreeSet<_> = evals.iter().map(|e| e.step_index).collect();
    assert_eq!(positions, [0, 1].into_iter().collect());
    let f = findings(&scenario(), &t, &evals);
    assert_eq!(
        f.len(),
        2,
        "one finding per violated invariant, from the first violating step"
    );
    assert!(
        f.iter().all(|x| x.observed.starts_with("step 0")),
        "{:?}",
        f.iter().map(|x| &x.observed).collect::<Vec<_>>()
    );
}

#[test]
fn findings_ignore_evaluations_that_point_outside_the_trace() {
    let t = trace(vec![vulnerable_deposit()]);
    let mut evals = evaluate(&t);
    for e in &mut evals {
        e.step_index = 42;
    }
    assert!(findings(&scenario(), &t, &evals).is_empty());
}

#[test]
fn registry_default_matches_catalog_and_evaluates_identically() {
    let t = trace(vec![vulnerable_deposit()]);
    let reg = InvariantRegistry::default();
    assert_eq!(reg.len(), 6);
    assert_eq!(reg.catalog().as_slice(), catalog().as_slice());

    let evals_free = evaluate(&t);
    let evals_reg = reg.evaluate(&t);
    assert_eq!(evals_free, evals_reg);
}

#[test]
fn custom_invariant_can_be_registered_and_evaluated() {
    let custom_def = InvariantDef {
        id: "INV-CUSTOM-001",
        name: "Custom Test Invariant",
        description: "Checks that vault is never 42",
        preconditions: &[],
        expected: "vault != 42",
        exceptions: &[],
        level: InvariantLevel::Custom,
    };

    let mut reg = InvariantRegistry::empty();
    assert!(reg.is_empty());

    reg.register(FnInvariant::new(custom_def, |step| {
        if step.after.vault_amount == 42 {
            Verdict::Violated {
                observed: "vault reached 42".into(),
            }
        } else {
            Verdict::Held
        }
    }));

    assert_eq!(reg.len(), 1);

    let mut step_rec = vulnerable_deposit();
    step_rec.after.vault_amount = 42;
    let t = trace(vec![step_rec]);

    let evals = reg.evaluate(&t);
    assert_eq!(evals.len(), 1);
    assert_eq!(evals[0].invariant_id, "INV-CUSTOM-001");
    assert_eq!(
        evals[0].verdict,
        Verdict::Violated {
            observed: "vault reached 42".into()
        }
    );
}
