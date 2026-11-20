//! P0 acceptance tests: the same valid scenarios expose the vulnerable escrow's accounting bug
//! and pass against the corrected escrow, using real SVM execution and real Token-2022 state.

use tokenscope_core::Engine;
use tokenscope_model::{Action, OutcomeClass, Scenario};
use tokenscope_scenarios::{builtin_suite, generate};

const VULN: &str = "demo-escrow-vulnerable";
const FIXED: &str = "demo-escrow-corrected";

fn by_name<'a>(c: &'a tokenscope_model::Campaign, name: &str) -> &'a tokenscope_model::RunRecord {
    c.runs
        .iter()
        .find(|r| r.scenario.name == name)
        .unwrap_or_else(|| panic!("no scenario {name}"))
}

#[test]
fn fee_scenario_fails_vulnerable_and_passes_corrected() {
    let engine = Engine::new();
    let v = engine.run_scenario(&builtin_suite(VULN)[1]);
    let c = engine.run_scenario(&builtin_suite(FIXED)[1]);
    assert_eq!(v.scenario.name, "fee-2pct-deposit");
    assert_eq!(v.outcome, OutcomeClass::InvariantViolation, "{v:#?}");
    let ids: Vec<_> = v.findings.iter().map(|f| f.invariant_id.as_str()).collect();
    assert_eq!(ids, ["INV-ACCT-001", "INV-SOLV-001"]);
    assert_eq!(c.outcome, OutcomeClass::Pass, "{c:#?}");
    assert!(c.findings.is_empty());
}

#[test]
fn builtin_suite_vulnerable_vs_corrected() {
    let engine = Engine::new();
    let v = engine.run_campaign(VULN, 0, &builtin_suite(VULN));
    let c = engine.run_campaign(FIXED, 0, &builtin_suite(FIXED));

    // Corrected escrow never violates an invariant and never fails setup.
    assert_eq!(
        c.totals().invariant_violations,
        0,
        "{:#?}",
        c.findings().collect::<Vec<_>>()
    );
    assert_eq!(c.totals().setup_errors, 0);

    // Vulnerable escrow: control passes; every fee scenario that moves tokens violates.
    assert_eq!(by_name(&v, "control-no-fee").outcome, OutcomeClass::Pass);
    for name in [
        "fee-2pct-deposit",
        "fee-2pct-deposit-withdraw",
        "fee-rounds-up-tiny-deposit",
        "fee-capped-by-maximum",
        "fee-100pct-consumes-deposit",
        "two-deposits-accumulate",
        "large-amount",
    ] {
        assert_eq!(
            by_name(&v, name).outcome,
            OutcomeClass::InvariantViolation,
            "{name}"
        );
        assert_eq!(
            by_name(&c, name).outcome,
            OutcomeClass::Pass,
            "corrected: {name}"
        );
    }
    // A zero-amount deposit has a zero fee, so it exposes nothing.
    assert_eq!(
        by_name(&v, "zero-amount-deposit").outcome,
        OutcomeClass::Pass
    );
    // Frozen source: the real Token-2022 rejects the deposit; that is an execution failure.
    for run in [&v, &c] {
        assert_eq!(
            by_name(run, "frozen-source-deposit-rejected").outcome,
            OutcomeClass::ExecutionFailure
        );
    }
    // After a thaw the fee bug is exposed again on the vulnerable flavor only.
    assert_eq!(
        by_name(&v, "freeze-thaw-then-deposit").outcome,
        OutcomeClass::InvariantViolation
    );
    assert_eq!(
        by_name(&c, "freeze-thaw-then-deposit").outcome,
        OutcomeClass::Pass
    );
    // Authorization holds on both flavors: the rejected attack is the desired behavior.
    for run in [&v, &c] {
        let r = by_name(run, "unauthorized-withdraw-rejected");
        assert_eq!(r.outcome, OutcomeClass::Pass);
        assert!(r.findings.iter().all(|f| f.invariant_id != "INV-AUTH-001"));
    }
    // Withdrawing with nothing recorded is a valid zero-amount transfer.
    assert_eq!(
        by_name(&v, "withdraw-with-nothing-recorded").outcome,
        OutcomeClass::Pass
    );
    // Rejected instructions are execution failures, not findings.
    for run in [&v, &c] {
        let r = by_name(run, "failure-insufficient-funds");
        assert_eq!(r.outcome, OutcomeClass::ExecutionFailure);
        assert!(r.findings.is_empty());
    }
    assert_eq!(v.totals().setup_errors, 0);
}

#[test]
fn locked_funds_after_vulnerable_deposit_are_visible_as_failed_withdraw() {
    let r = Engine::new().run_scenario(&builtin_suite(VULN)[2]);
    assert_eq!(r.scenario.name, "fee-2pct-deposit-withdraw");
    let trace = r.trace.unwrap();
    assert!(matches!(
        trace.steps[1].status,
        tokenscope_model::StepStatus::Failed { .. }
    ));
}

#[test]
fn setup_problems_are_never_reported_as_violations() {
    let engine = Engine::new();
    let mut s: Scenario = builtin_suite(VULN)[1].clone();
    s.target = "no-such-target".into();
    let r = engine.run_scenario(&s);
    assert_eq!(r.outcome, OutcomeClass::SetupError);
    assert!(r.findings.is_empty() && r.trace.is_none());

    let missing =
        Engine::with_program_dir("/nonexistent-dir").run_scenario(&builtin_suite(VULN)[1]);
    assert_eq!(missing.outcome, OutcomeClass::SetupError);
    assert!(missing.setup_error.unwrap().contains("build-programs.sh"));

    let mut invalid = builtin_suite(VULN)[1].clone();
    invalid.actions.clear();
    assert_eq!(
        engine.run_scenario(&invalid).outcome,
        OutcomeClass::SetupError
    );
}

#[test]
fn campaigns_are_deterministic() {
    let engine = Engine::new();
    let suite = builtin_suite(VULN);
    assert_eq!(
        engine.run_campaign(VULN, 0, &suite),
        engine.run_campaign(VULN, 0, &suite)
    );
}

#[test]
fn seeded_generation_runs_deterministically_and_corrected_never_violates() {
    let engine = Engine::new();
    let a = engine.run_campaign(VULN, 99, &generate(99, 25, VULN));
    let b = engine.run_campaign(VULN, 99, &generate(99, 25, VULN));
    assert_eq!(a, b);
    assert_eq!(a.totals().setup_errors, 0);
    let c = engine.run_campaign(FIXED, 99, &generate(99, 25, FIXED));
    assert_eq!(
        c.totals().invariant_violations,
        0,
        "{:#?}",
        c.findings().collect::<Vec<_>>()
    );
    // Any vulnerable finding must come from a scenario that actually deposits under a fee.
    for f in a.findings() {
        let fee = f.reproduction.scenario.mint.transfer_fee.is_some();
        let deposits = f
            .reproduction
            .scenario
            .actions
            .iter()
            .any(|x| matches!(x, Action::Deposit { amount } if *amount > 0));
        assert!(fee && deposits, "unexpected finding {f:#?}");
    }
}

struct MockVaultAdapter;

impl tokenscope_core::TargetAdapter for MockVaultAdapter {
    fn target_id(&self) -> &str {
        "custom-vault-target"
    }

    fn run(
        &self,
        scenario: &Scenario,
    ) -> Result<tokenscope_model::ExecutionTrace, tokenscope_executor::ExecError> {
        let snap_before = tokenscope_model::Snapshot {
            vault_amount: 500,
            escrow_recorded: 500,
            depositor_balance: 1000,
            ..tokenscope_model::Snapshot::default()
        };
        let snap_after = tokenscope_model::Snapshot {
            vault_amount: 1500,
            escrow_recorded: 1500,
            depositor_balance: 0,
            ..tokenscope_model::Snapshot::default()
        };
        let step = tokenscope_model::StepRecord {
            index: 0,
            action: scenario
                .actions
                .first()
                .cloned()
                .unwrap_or(Action::Deposit { amount: 1000 }),
            before: snap_before,
            after: snap_after,
            status: tokenscope_model::StepStatus::Ok,
            logs: vec!["Program log: Vault credited deposit".into()],
            compute_units: 450,
            raw_before: None,
            raw_after: None,
        };
        Ok(tokenscope_model::ExecutionTrace {
            schema_version: 1,
            scenario_id: scenario.id(),
            target: "custom-vault-target".into(),
            initial: snap_before,
            steps: vec![step],
            accounts: vec![],
            raw_initial: None,
        })
    }
}

#[test]
fn second_target_adapter_can_be_executed_and_generates_valid_campaign() {
    let engine = Engine::new();
    let adapter = MockVaultAdapter;
    let s = builtin_suite(VULN)[0].clone();
    let r = engine.run_scenario_with_adapter(&s, &adapter);
    assert_eq!(r.outcome, OutcomeClass::Pass);
    assert!(r.trace.is_some());

    let c = engine.run_campaign_with_adapter("custom-vault-target", 42, &[s], &adapter);
    assert_eq!(c.runs.len(), 1);
    assert_eq!(c.totals().passed, 1);
    assert_eq!(c.totals().invariant_violations, 0);
}
