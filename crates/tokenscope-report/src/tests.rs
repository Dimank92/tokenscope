use tokenscope_model::{
    Action, Campaign, Finding, MintSpec, OutcomeClass, Reproduction, RunRecord, SCHEMA_VERSION,
    Scenario, Severity, StateDelta,
};

use crate::*;

fn scenario(name: &str) -> Scenario {
    Scenario {
        schema_version: SCHEMA_VERSION,
        name: name.into(),
        seed: 4,
        target: "t".into(),
        mint: MintSpec {
            decimals: 0,
            transfer_fee: None,
        },
        initial_balance: 5,
        extensions: vec![],
        actions: vec![Action::Deposit { amount: 5 }],
    }
}

fn finding(s: &Scenario) -> Finding {
    let sid = s.id();
    Finding {
        schema_version: SCHEMA_VERSION,
        id: Finding::compute_id("t", &sid, "INV-ACCT-001"),
        target: "t".into(),
        scenario_id: sid,
        seed: 4,
        invariant_id: "INV-ACCT-001".into(),
        class: OutcomeClass::InvariantViolation,
        expected: "e".into(),
        observed: "o".into(),
        deltas: vec![StateDelta {
            label: "vault_amount".into(),
            account: "A".into(),
            before: 0,
            after: 4,
        }],
        logs: vec![],
        severity: Severity::Medium,
        severity_rationale: "r".into(),
        reproduction: Reproduction {
            command: "tokenscope replay x.json".into(),
            scenario: s.clone(),
        },
        minimized: None,
    }
}

fn campaign() -> Campaign {
    let bad = scenario("bad");
    let good = scenario("good");
    let mut good = good;
    good.initial_balance = 6;
    let runs = vec![
        RunRecord {
            scenario_id: bad.id(),
            findings: vec![finding(&bad)],
            scenario: bad,
            outcome: OutcomeClass::InvariantViolation,
            setup_error: None,
            trace: None,
        },
        RunRecord {
            scenario_id: good.id(),
            findings: vec![],
            scenario: good,
            outcome: OutcomeClass::Pass,
            setup_error: None,
            trace: None,
        },
    ];
    Campaign::new("t".into(), 4, runs)
}

#[test]
fn json_report_matches_campaign_contents() {
    let c = campaign();
    let r: Report = serde_json::from_str(&render_json(&c)).unwrap();
    assert_eq!(r.campaign_id, c.id);
    assert_eq!(r.totals, c.totals());
    assert_eq!(r.findings, c.findings().cloned().collect::<Vec<_>>());
    assert_eq!(r.runs.len(), 2);
    assert_eq!(r.runs[0].finding_ids, vec![r.findings[0].id.clone()]);
    assert_eq!(r.schema_version, 1);
}

#[test]
fn rendering_is_deterministic() {
    assert_eq!(render_json(&campaign()), render_json(&campaign()));
    assert_eq!(render_text(&campaign()), render_text(&campaign()));
    assert_eq!(render_markdown(&campaign()), render_markdown(&campaign()));
    assert_eq!(render_sarif(&campaign()), render_sarif(&campaign()));
}

#[test]
fn markdown_report_renders_headers_and_tables() {
    let c = campaign();
    let md = render_markdown(&c);
    assert!(md.contains("# TokenScope Campaign Report"));
    assert!(md.contains(&c.id));
    assert!(md.contains("INV-ACCT-001"));
    assert!(md.contains("State Deltas"));
    assert!(md.contains("tokenscope replay x.json"));
}

#[test]
fn sarif_report_is_valid_json_and_contains_schema_and_results() {
    let c = campaign();
    let sarif = render_sarif(&c);
    let v: serde_json::Value = serde_json::from_str(&sarif).unwrap();
    assert_eq!(v["version"], "2.1.0");
    assert_eq!(v["runs"][0]["tool"]["driver"]["name"], "TokenScope");
    assert_eq!(v["runs"][0]["results"].as_array().unwrap().len(), 1);
    assert_eq!(v["runs"][0]["results"][0]["ruleId"], "INV-ACCT-001");
}

#[test]
fn text_report_shows_counts_findings_evidence_and_caveat() {
    let c = campaign();
    let t = render_text(&c);
    assert!(t.contains(&c.id));
    assert!(t.contains("invariant violations 1"));
    assert!(t.contains("INV-ACCT-001"));
    assert!(t.contains("vault_amount 0 -> 4 (A)"));
    assert!(t.contains("tokenscope replay x.json"));
    assert!(t.contains("not proof of exploitability"));
}

#[test]
fn artifacts_are_written_and_reloadable() {
    let dir = tempfile::tempdir().unwrap();
    let c = campaign();
    let a = write_artifacts(&c, dir.path()).unwrap();
    assert_eq!(
        Campaign::from_json(&std::fs::read_to_string(&a.campaign).unwrap()).unwrap(),
        c
    );
    assert_eq!(a.findings.len(), 1);
    let f = Finding::from_json(&std::fs::read_to_string(&a.findings[0]).unwrap()).unwrap();
    assert_eq!(&f, c.findings().next().unwrap());
    let s = Scenario::from_json(&std::fs::read_to_string(&a.scenarios[0]).unwrap()).unwrap();
    assert_eq!(s, f.reproduction.scenario);
    assert!(
        a.report_json.exists()
            && a.report_text.exists()
            && a.report_markdown.exists()
            && a.report_sarif.exists()
    );
}
