//! Extension, authorization and boundary scenarios on real SVM execution, both escrow flavors.

use tokenscope_core::Engine;
use tokenscope_model::{Campaign, OutcomeClass, RunRecord};
use tokenscope_scenarios::extension_suite;

fn campaign(target: &str) -> Campaign {
    Engine::new().run_campaign(target, 0, &extension_suite(target))
}

fn run<'a>(c: &'a Campaign, name: &str) -> &'a RunRecord {
    c.runs
        .iter()
        .find(|r| r.scenario.name == name)
        .unwrap_or_else(|| panic!("no scenario {name}"))
}

fn ids(r: &RunRecord) -> Vec<&str> {
    r.findings.iter().map(|f| f.invariant_id.as_str()).collect()
}

#[test]
fn extension_suite_outcomes_on_both_flavors() {
    let v = campaign("demo-escrow-vulnerable");
    let c = campaign("demo-escrow-corrected");
    assert_eq!(v.totals().setup_errors, 0);
    assert_eq!(c.totals().setup_errors, 0);

    // Operations the real runtime or escrow rejects: execution failures, never findings.
    for name in [
        "pausable-deposit-gated",
        "cpi-guard-blocks-escrow-deposit",
        "default-frozen-vault-blocks-deposit",
        "delegate-allowance-lifecycle",
        "transfer-hook-mint-unsupported-by-escrow",
    ] {
        for camp in [&v, &c] {
            let r = run(camp, name);
            assert_eq!(
                r.outcome,
                OutcomeClass::ExecutionFailure,
                "{} {name}",
                camp.target
            );
            assert!(r.findings.is_empty(), "{name}: {:?}", ids(r));
        }
    }

    // Adversarial operations are rejected without state change, so these pass on both.
    for name in [
        "mismatched-accounts-rejected",
        "repeated-withdrawals",
        "u64-max-deposit-no-fee",
    ] {
        for camp in [&v, &c] {
            assert_eq!(
                run(camp, name).outcome,
                OutcomeClass::Pass,
                "{} {name}",
                camp.target
            );
        }
    }

    // The permanent delegate breaks the escrow's custody assumption on BOTH flavors.
    for camp in [&v, &c] {
        let r = run(camp, "permanent-delegate-drains-vault");
        assert_eq!(
            r.outcome,
            OutcomeClass::InvariantViolation,
            "{}",
            camp.target
        );
        let mut got = ids(r);
        got.sort();
        assert_eq!(got, ["INV-CUST-001", "INV-SOLV-001"], "{}", camp.target);
        let cust = r
            .findings
            .iter()
            .find(|f| f.invariant_id == "INV-CUST-001")
            .unwrap();
        assert!(
            cust.observed.contains("PermanentDelegateDrain"),
            "{}",
            cust.observed
        );
        assert!(
            cust.severity_rationale
                .to_lowercase()
                .contains("not necessarily a program bug")
        );
    }

    // The fee bug at the u64 boundary: vulnerable only.
    let r = run(&v, "u64-max-deposit-with-fee");
    assert_eq!(r.outcome, OutcomeClass::InvariantViolation);
    assert!(ids(r).contains(&"INV-ACCT-001"));
    assert_eq!(
        run(&c, "u64-max-deposit-with-fee").outcome,
        OutcomeClass::Pass
    );

    // No other finding anywhere: ACCT/AUTH/ATOM/CONS never fire on the corrected flavor.
    let other: Vec<_> = c
        .findings()
        .filter(|f| f.invariant_id != "INV-CUST-001" && f.invariant_id != "INV-SOLV-001")
        .collect();
    assert!(other.is_empty(), "{other:#?}");
}

#[test]
fn extension_campaigns_are_deterministic() {
    assert_eq!(
        campaign("demo-escrow-corrected"),
        campaign("demo-escrow-corrected")
    );
}
