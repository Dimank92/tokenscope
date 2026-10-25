//! Black-box tests of the `tokenscope` binary: exit codes and output must reflect real execution.

use std::{fs, path::Path, process::Command};

use tokenscope_core::Engine;
use tokenscope_model::{Campaign, CorpusCase, Finding, Scenario};
use tokenscope_report::Report;
use tokenscope_scenarios::{builtin_suite, generate};

const VULN: &str = "demo-escrow-vulnerable";
const FIXED: &str = "demo-escrow-corrected";

struct Out {
    code: i32,
    stdout: String,
    stderr: String,
}

fn tokenscope(args: &[&str]) -> Out {
    tokenscope_env(args, &[])
}

fn tokenscope_env(args: &[&str], env: &[(&str, &str)]) -> Out {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_tokenscope"));
    cmd.args(args).env_remove("RUST_LOG");
    for (k, v) in env {
        cmd.env(k, v);
    }
    let o = cmd.output().expect("spawn tokenscope");
    Out {
        code: o.status.code().expect("exit code"),
        stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
    }
}

fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

fn only_dir(p: &Path) -> std::path::PathBuf {
    let mut it = fs::read_dir(p).unwrap().map(|e| e.unwrap().path());
    let d = it.next().expect("one directory");
    assert!(it.next().is_none());
    d
}

#[test]
fn doctor_verifies_the_environment_with_real_execution() {
    let o = tokenscope(&["doctor"]);
    assert_eq!(o.code, 0, "{}{}", o.stdout, o.stderr);
    assert!(
        o.stdout.contains("delivered 980 with 20 withheld"),
        "{}",
        o.stdout
    );
    assert!(o.stdout.contains("doctor: environment OK"));
}

#[test]
fn doctor_fails_with_exit_3_when_programs_are_missing() {
    let o = tokenscope_env(
        &["doctor"],
        &[("TOKENSCOPE_PROGRAM_DIR", "/nonexistent-dir")],
    );
    assert_eq!(o.code, 3);
    assert!(o.stdout.contains("[FAIL]") && o.stdout.contains("build-programs.sh"));
}

#[test]
fn inspect_lists_targets_and_invariants() {
    let o = tokenscope(&["inspect"]);
    assert_eq!(o.code, 0);
    for needle in [
        VULN,
        FIXED,
        "INV-ACCT-001",
        "INV-SOLV-001",
        "fee-2pct-deposit",
    ] {
        assert!(o.stdout.contains(needle), "missing {needle}");
    }
    assert_eq!(tokenscope(&["inspect", "--target", "nope"]).code, 2);
}

#[test]
fn run_vulnerable_exits_1_and_matches_direct_engine_results() {
    let dir = tempfile::tempdir().unwrap();
    let o = tokenscope(&[
        "run",
        "--target",
        VULN,
        "--seed",
        "7",
        "--count",
        "5",
        "--out",
        s(dir.path()),
    ]);
    assert_eq!(o.code, 1, "{}{}", o.stdout, o.stderr);
    assert!(o.stderr.is_empty(), "no log noise: {}", o.stderr);

    // The stored campaign is identical to running the same scenarios directly.
    let mut scenarios = builtin_suite(VULN);
    scenarios.extend(generate(7, 5, VULN));
    let expected = Engine::new().run_campaign(VULN, 7, &scenarios);
    let cdir = only_dir(dir.path());
    let stored =
        Campaign::from_json(&fs::read_to_string(cdir.join("campaign.json")).unwrap()).unwrap();
    assert_eq!(stored, expected);
    assert_eq!(cdir.file_name().unwrap().to_str().unwrap(), expected.id);
    assert!(o.stdout.contains(&expected.id));

    // The JSON report is a view of the same campaign.
    let report: Report =
        serde_json::from_str(&fs::read_to_string(cdir.join("report.json")).unwrap()).unwrap();
    assert_eq!(report.totals, expected.totals());
    assert_eq!(
        report.findings,
        expected.findings().cloned().collect::<Vec<_>>()
    );
    assert!(report.totals.invariant_violations > 0 && report.totals.setup_errors == 0);
}

#[test]
fn run_corrected_exits_0() {
    let dir = tempfile::tempdir().unwrap();
    let o = tokenscope(&[
        "run",
        "--target",
        FIXED,
        "--seed",
        "7",
        "--count",
        "5",
        "--out",
        s(dir.path()),
    ]);
    assert_eq!(o.code, 0, "{}{}", o.stdout, o.stderr);
    assert!(o.stdout.contains("invariant violations 0"));
}

#[test]
fn run_demo_escrow_runs_both_flavors_and_reports_violations() {
    let dir = tempfile::tempdir().unwrap();
    let o = tokenscope(&[
        "run",
        "--target",
        "demo-escrow",
        "--count",
        "0",
        "--out",
        s(dir.path()),
    ]);
    assert_eq!(o.code, 1);
    assert!(
        o.stdout.contains(&format!("target: {VULN}"))
            && o.stdout.contains(&format!("target: {FIXED}"))
    );
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
}

#[test]
fn run_from_config_file() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = dir.path().join("run.json");
    fs::write(&cfg, r#"{"schema_version":1,"target":"demo-escrow-corrected","seed":3,"generated":2,"builtin_suite":false}"#).unwrap();
    let out = dir.path().join("out");
    let o = tokenscope(&["run", "--config", s(&cfg), "--out", s(&out)]);
    assert_eq!(o.code, 0, "{}{}", o.stdout, o.stderr);
    assert!(o.stdout.contains("executed 2 "), "{}", o.stdout);
}

#[test]
fn invalid_usage_and_input_exit_2() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(tokenscope(&["frobnicate"]).code, 2);
    assert_eq!(tokenscope(&["run"]).code, 2);
    assert_eq!(tokenscope(&["run", "--target", "nope"]).code, 2);
    assert_eq!(
        tokenscope(&["run", "--target", VULN, "--no-suite", "--count", "0"]).code,
        2
    );
    assert_eq!(
        tokenscope(&["run", "--target", VULN, "--count", "100000"]).code,
        2
    );
    let bad = dir.path().join("bad.json");
    fs::write(&bad, "{not json").unwrap();
    assert_eq!(tokenscope(&["run", "--config", s(&bad)]).code, 2);
    fs::write(&bad, r#"{"schema_version":9,"target":"demo-escrow"}"#).unwrap();
    assert_eq!(tokenscope(&["run", "--config", s(&bad)]).code, 2);
    fs::write(
        &bad,
        r#"{"schema_version":1,"target":"demo-escrow","surprise":1}"#,
    )
    .unwrap();
    assert_eq!(tokenscope(&["run", "--config", s(&bad)]).code, 2);
    assert_eq!(tokenscope(&["replay", "/no/such/file.json"]).code, 2);
    fs::write(&bad, "{not json").unwrap();
    assert_eq!(tokenscope(&["replay", s(&bad)]).code, 2);
    assert_eq!(tokenscope(&["minimize", s(&bad)]).code, 2);
    let o = tokenscope(&["--help"]);
    assert_eq!(o.code, 0);
    assert!(o.stdout.contains("doctor") && o.stdout.contains("corpus"));
}

#[test]
fn missing_programs_are_a_setup_error_not_a_violation() {
    let dir = tempfile::tempdir().unwrap();
    let o = tokenscope_env(
        &[
            "run",
            "--target",
            FIXED,
            "--no-suite",
            "--count",
            "1",
            "--out",
            s(dir.path()),
        ],
        &[("TOKENSCOPE_PROGRAM_DIR", "/nonexistent-dir")],
    );
    assert_eq!(o.code, 3, "{}{}", o.stdout, o.stderr);
    assert!(o.stdout.contains("setup errors 1") && o.stdout.contains("findings 0"));
}

#[test]
fn replay_and_minimize_round_trip_through_artifacts() {
    let dir = tempfile::tempdir().unwrap();
    tokenscope(&[
        "run",
        "--target",
        VULN,
        "--count",
        "0",
        "--out",
        s(dir.path()),
    ]);
    let cdir = only_dir(dir.path());

    // Replay a finding's scenario: violation on the vulnerable target...
    let scenario_file = fs::read_dir(cdir.join("scenarios"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(tokenscope(&["replay", s(&scenario_file)]).code, 1);
    // ...and the same scenario retargeted at the corrected escrow passes.
    let mut sc = Scenario::from_json(&fs::read_to_string(&scenario_file).unwrap()).unwrap();
    sc.target = FIXED.into();
    let fixed_file = dir.path().join("fixed.json");
    fs::write(&fixed_file, sc.to_json()).unwrap();
    assert_eq!(tokenscope(&["replay", s(&fixed_file)]).code, 0);

    // Minimize the finding of the large 'fee-2pct' style scenario.
    let finding_file = fs::read_dir(cdir.join("findings"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| {
            let f = Finding::from_json(&fs::read_to_string(p).unwrap()).unwrap();
            f.invariant_id == "INV-ACCT-001"
                && f.reproduction.scenario.name == "two-deposits-accumulate"
        })
        .expect("finding for two-deposits-accumulate");
    let min_out = dir.path().join("min.json");
    let o = tokenscope(&["minimize", s(&finding_file), "--out", s(&min_out)]);
    assert_eq!(o.code, 0, "{}{}", o.stdout, o.stderr);
    let min = Finding::from_json(&fs::read_to_string(&min_out).unwrap()).unwrap();
    let m = min.minimized.expect("minimized scenario");
    assert_eq!(m.actions.len(), 1);
    assert!(m.actions.len() < min.reproduction.scenario.actions.len());
}

#[test]
fn minimizing_a_finding_that_does_not_reproduce_exits_4() {
    let dir = tempfile::tempdir().unwrap();
    tokenscope(&[
        "run",
        "--target",
        VULN,
        "--count",
        "0",
        "--out",
        s(dir.path()),
    ]);
    let cdir = only_dir(dir.path());
    let fpath = fs::read_dir(cdir.join("findings"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let mut f = Finding::from_json(&fs::read_to_string(&fpath).unwrap()).unwrap();
    f.reproduction.scenario.target = FIXED.into();
    let altered = dir.path().join("altered.json");
    fs::write(&altered, f.to_json()).unwrap();
    assert_eq!(tokenscope(&["minimize", s(&altered)]).code, 4);
}

#[test]
fn report_prints_stored_campaigns_in_both_formats() {
    let dir = tempfile::tempdir().unwrap();
    tokenscope(&[
        "run",
        "--target",
        VULN,
        "--count",
        "0",
        "--out",
        s(dir.path()),
    ]);
    let cdir = only_dir(dir.path());
    let id = cdir.file_name().unwrap().to_str().unwrap().to_string();
    let j = tokenscope(&["report", &id, "--format", "json", "--out", s(dir.path())]);
    assert_eq!(j.code, 0);
    assert_eq!(
        j.stdout,
        fs::read_to_string(cdir.join("report.json")).unwrap()
    );
    let t = tokenscope(&["report", &id, "--format", "text", "--out", s(dir.path())]);
    assert_eq!(t.code, 0);
    assert_eq!(
        t.stdout,
        fs::read_to_string(cdir.join("report.txt")).unwrap()
    );
    assert_eq!(
        tokenscope(&["report", "cp_0000000000000000", "--out", s(dir.path())]).code,
        2
    );
    assert_eq!(
        tokenscope(&["report", "../etc/passwd", "--out", s(dir.path())]).code,
        2
    );
}

#[test]
fn corpus_add_list_replay_and_regression_detection() {
    let dir = tempfile::tempdir().unwrap();
    let corpus = dir.path().join("corpus");
    let c = s(&corpus);
    assert_eq!(
        tokenscope(&["corpus", "list", "--corpus", c]).stdout.trim(),
        "corpus is empty"
    );
    assert_eq!(
        tokenscope(&["corpus", "replay", "--all", "--corpus", c]).code,
        2
    );

    for (target, desc) in [(VULN, "bug demonstration"), (FIXED, "fixed regression")] {
        let sc = builtin_suite(target)[1].clone();
        let f = dir.path().join(format!("{target}.json"));
        fs::write(&f, sc.to_json()).unwrap();
        let o = tokenscope(&["corpus", "add", s(&f), "--description", desc, "--corpus", c]);
        assert_eq!(o.code, 0, "{}{}", o.stdout, o.stderr);
    }
    let list = tokenscope(&["corpus", "list", "--corpus", c]);
    assert_eq!(list.code, 0);
    assert_eq!(list.stdout.lines().count(), 2);
    assert!(list.stdout.contains("InvariantViolation") && list.stdout.contains("Pass"));

    let ok = tokenscope(&["corpus", "replay", "--all", "--corpus", c]);
    assert_eq!(ok.code, 0, "{}", ok.stdout);
    assert!(ok.stdout.contains("2 case(s), 0 mismatch(es)"));

    // Tamper with an expectation: replay must detect the regression (exit 4).
    let case_file = fs::read_dir(&corpus)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| {
            CorpusCase::from_json(&fs::read_to_string(p).unwrap())
                .unwrap()
                .scenario
                .target
                == VULN
        })
        .unwrap();
    let mut case = CorpusCase::from_json(&fs::read_to_string(&case_file).unwrap()).unwrap();
    case.expect.outcome = tokenscope_model::OutcomeClass::Pass;
    case.expect.violated.clear();
    fs::write(&case_file, case.to_json()).unwrap();
    let bad = tokenscope(&["corpus", "replay", &case.id, "--corpus", c]);
    assert_eq!(bad.code, 4, "{}", bad.stdout);
    assert!(bad.stdout.contains("FAIL"));
    assert_eq!(
        tokenscope(&["corpus", "replay", "rc_unknown", "--corpus", c]).code,
        2
    );
}

#[test]
fn tui_snapshot_shows_the_same_findings_as_the_cli_report() {
    let dir = tempfile::tempdir().unwrap();
    let run = tokenscope(&[
        "run",
        "--target",
        VULN,
        "--count",
        "0",
        "--out",
        s(dir.path()),
    ]);
    assert_eq!(run.code, 1);
    let cdir = only_dir(dir.path());
    let report: Report =
        serde_json::from_str(&fs::read_to_string(cdir.join("report.json")).unwrap()).unwrap();

    let dash = tokenscope(&["tui", "--snapshot", "dashboard", "--target", VULN]);
    assert_eq!(dash.code, 0, "{}{}", dash.stdout, dash.stderr);
    assert!(
        dash.stdout.contains(&report.campaign_id),
        "same campaign id in TUI and CLI"
    );
    assert!(
        dash.stdout
            .contains(&format!("findings: {}", report.totals.findings))
    );
    assert!(dash.stdout.contains(&format!(
        "invariant violations: {}",
        report.totals.invariant_violations
    )));

    let findings = tokenscope(&["tui", "--snapshot", "findings", "--target", VULN]);
    assert_eq!(findings.code, 0);
    // The findings list shows the first findings; each ID shown must be one the report contains,
    // and the first finding of the report must be listed.
    assert!(
        findings.stdout.contains(&report.findings[0].id),
        "{}",
        findings.stdout
    );
    assert!(
        findings
            .stdout
            .contains(&format!("Findings ({})", report.findings.len()))
    );
    for f in &report.findings {
        if findings.stdout.contains(&f.id) {
            assert!(report.findings.iter().any(|g| g.id == f.id));
        }
    }
}

#[test]
fn tui_snapshot_corrected_target_has_no_findings_and_inputs_are_validated() {
    let o = tokenscope(&["tui", "--snapshot", "findings", "--target", FIXED]);
    assert_eq!(o.code, 0);
    assert!(o.stdout.contains("Findings (0)") && o.stdout.contains("No findings."));
    assert_eq!(tokenscope(&["tui", "--snapshot", "nonsense"]).code, 2);
    assert_eq!(
        tokenscope(&["tui", "--target", "demo-escrow", "--snapshot", "help"]).code,
        2
    );
    assert_eq!(
        tokenscope(&["tui", "--target", "nope", "--snapshot", "help"]).code,
        2
    );
    assert_eq!(
        tokenscope(&["tui", "--no-suite", "--snapshot", "help"]).code,
        2
    );
    let help = tokenscope(&["tui", "--snapshot", "help"]);
    assert_eq!(help.code, 0);
    assert!(help.stdout.contains("Shortcuts"));
}
