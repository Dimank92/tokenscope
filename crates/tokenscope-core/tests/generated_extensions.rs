//! Randomly generated extension/adversarial scenarios against the real SVM on both flavors.
//! These checks encode properties that must hold for *every* generated scenario.

use std::collections::BTreeSet;

use tokenscope_core::Engine;
use tokenscope_model::OutcomeClass;
use tokenscope_scenarios::{builtin_suite, extension_suite, generate_extended, generate_guided};

const VULN: &str = "demo-escrow-vulnerable";
const FIXED: &str = "demo-escrow-corrected";

#[test]
fn generated_extension_scenarios_never_break_the_harness_and_respect_flavor_properties() {
    let engine = Engine::new();
    let v = engine.run_campaign(VULN, 7, &generate_extended(7, 120, VULN));
    let c = engine.run_campaign(FIXED, 7, &generate_extended(7, 120, FIXED));

    for camp in [&v, &c] {
        let setup: Vec<_> = camp
            .runs
            .iter()
            .filter(|r| r.outcome == OutcomeClass::SetupError)
            .map(|r| (&r.scenario.name, &r.setup_error))
            .collect();
        assert!(
            setup.is_empty(),
            "{}: generated scenarios must never be setup errors: {setup:#?}",
            camp.target
        );
    }

    // The corrected escrow may only be flagged for the permanent-delegate custody break (and its
    // solvency consequence). Fee accounting, authorization, atomicity and conservation hold.
    let allowed: BTreeSet<&str> = ["INV-CUST-001", "INV-SOLV-001"].into_iter().collect();
    for f in c.findings() {
        assert!(
            allowed.contains(f.invariant_id.as_str()),
            "unexpected corrected-flavor finding: {f:#?}"
        );
        let sc = &f.reproduction.scenario;
        assert!(
            sc.extensions
                .contains(&tokenscope_model::MintExtension::PermanentDelegate),
            "custody finding without a permanent delegate: {sc:#?}"
        );
    }
    // The vulnerable escrow shows the same plus fee-accounting findings, which only occur with a fee.
    for f in v.findings().filter(|f| f.invariant_id == "INV-ACCT-001") {
        assert!(f.reproduction.scenario.mint.transfer_fee.is_some());
    }
    // Atomicity, authorization and conservation never fire on either flavor.
    for camp in [&v, &c] {
        for f in camp.findings() {
            assert!(
                !["INV-ATOM-001", "INV-AUTH-001", "INV-CONS-001"]
                    .contains(&f.invariant_id.as_str()),
                "{}: {f:#?}",
                camp.target
            );
        }
    }
    assert!(v.totals().invariant_violations > 0 && v.totals().passed > 0);
}

#[test]
fn guided_campaigns_are_deterministic_and_complement_the_suites() {
    let engine = Engine::new();
    let seeds: Vec<_> = builtin_suite(VULN)
        .into_iter()
        .chain(extension_suite(VULN))
        .collect();
    let batch = generate_guided(21, 30, VULN, &seeds);
    assert!(!batch.scenarios.is_empty());
    let a = engine.run_campaign(VULN, 21, &batch.scenarios);
    let b = engine.run_campaign(VULN, 21, &batch.scenarios);
    assert_eq!(
        a, b,
        "replay of a guided campaign is byte-for-byte deterministic"
    );
    assert_eq!(a.totals().setup_errors, 0);
    // A guided scenario that violated an invariant also reproduces after a JSON round trip.
    if let Some(f) = a.findings().next() {
        let s = tokenscope_model::Scenario::from_json(&f.reproduction.scenario.to_json()).unwrap();
        assert!(
            engine
                .run_scenario(&s)
                .findings
                .iter()
                .any(|g| g.id == f.id)
        );
    }
}
