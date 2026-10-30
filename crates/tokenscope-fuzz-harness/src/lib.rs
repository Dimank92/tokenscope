//! Bodies of the fuzz targets.
//!
//! Each function takes arbitrary bytes, must never panic, and asserts internal consistency of
//! whatever it accepts. The `fuzz/` cargo-fuzz package wraps them in libFuzzer entry points; the
//! tests in this crate also run them on stable over a seed corpus and deterministic mutations, so
//! regressions are caught without any fuzzing toolchain.

use tokenscope_invariants::{classify, evaluate, findings};
use tokenscope_model::{Campaign, CorpusCase, ExecutionTrace, Finding, Scenario};
use tokenscope_report::{build_report, render_json, render_text};
use tokenscope_shrinker::{ShrinkConfig, shrink};

/// Names of all targets (the `fuzz/fuzz_targets/<name>.rs` files).
pub const TARGETS: [&str; 7] = [
    "parse_scenario",
    "parse_finding",
    "parse_campaign",
    "parse_corpus_case",
    "evaluate_trace",
    "shrink_scenario",
    "replay_scenario",
];

/// Run a target by name (used by the tests).
pub fn run_target(name: &str, data: &[u8]) {
    match name {
        "parse_scenario" => parse_scenario(data),
        "parse_finding" => parse_finding(data),
        "parse_campaign" => parse_campaign(data),
        "parse_corpus_case" => parse_corpus_case(data),
        "evaluate_trace" => evaluate_trace(data),
        "shrink_scenario" => shrink_scenario(data),
        "replay_scenario" => replay_scenario(data),
        other => panic!("unknown fuzz target {other}"),
    }
}

fn text(data: &[u8]) -> Option<&str> {
    std::str::from_utf8(data).ok()
}

/// Scenario parser: accepted input validates, round-trips and has a stable ID.
pub fn parse_scenario(data: &[u8]) {
    let Some(t) = text(data) else { return };
    if let Ok(s) = Scenario::from_json(t) {
        assert!(s.validate().is_ok());
        let back = Scenario::from_json(&s.to_json()).expect("serialized scenario must parse");
        assert_eq!(back, s);
        assert_eq!(back.id(), s.id());
    }
}

/// Finding parser: accepted input validates and round-trips.
pub fn parse_finding(data: &[u8]) {
    let Some(t) = text(data) else { return };
    if let Ok(f) = Finding::from_json(t) {
        assert!(f.validate().is_ok());
        assert_eq!(Finding::from_json(&f.to_json()).expect("round trip"), f);
    }
}

/// Campaign parser plus report rendering: accepted campaigns render JSON and text without panics
/// and the report is consistent with the campaign.
pub fn parse_campaign(data: &[u8]) {
    let Some(t) = text(data) else { return };
    if let Ok(c) = Campaign::from_json(t) {
        let r = build_report(&c);
        assert_eq!(r.totals, c.totals());
        assert_eq!(r.runs.len(), c.runs.len());
        let _ = render_json(&c);
        let _ = render_text(&c);
        assert_eq!(Campaign::from_json(&c.to_json()).expect("round trip"), c);
    }
}

/// Corpus-case parser: accepted input validates and round-trips.
pub fn parse_corpus_case(data: &[u8]) {
    let Some(t) = text(data) else { return };
    if let Ok(c) = CorpusCase::from_json(t) {
        assert!(c.validate().is_ok());
        assert_eq!(CorpusCase::from_json(&c.to_json()).expect("round trip"), c);
    }
}

/// Invariant engine on an arbitrary (possibly nonsensical) trace: must not panic, and findings must
/// only come from violated evaluations.
pub fn evaluate_trace(data: &[u8]) {
    let Some(t) = text(data) else { return };
    let Ok(trace) = ExecutionTrace::from_json(t) else {
        return;
    };
    assert_eq!(
        ExecutionTrace::from_json(&trace.to_json()).expect("round trip"),
        trace
    );
    let evals = evaluate(&trace);
    let class = classify(&trace, &evals);
    let scenario = tokenscope_scenarios::builtin_suite(&trace.target)
        .into_iter()
        .next();
    if let Some(s) = scenario {
        for f in findings(&s, &trace, &evals) {
            assert!(f.validate().is_ok());
            assert_ne!(class, tokenscope_model::OutcomeClass::Pass);
        }
    }
}

/// Shrinker on an arbitrary valid scenario with a synthetic threshold predicate derived from the
/// input: the result must be valid, satisfy the predicate and never grow the input.
pub fn shrink_scenario(data: &[u8]) {
    let Some(t) = text(data) else { return };
    let Ok(s) = Scenario::from_json(t) else {
        return;
    };
    let threshold = (data.iter().map(|b| u64::from(*b)).sum::<u64>() % 500) + 1;
    let pred = |x: &Scenario| {
        x.actions
            .iter()
            .filter_map(|a| a.amount())
            .sum::<u64>()
            .saturating_add(x.actions.len() as u64)
            >= threshold
    };
    if let Ok(r) = shrink(&s, ShrinkConfig { max_attempts: 150 }, pred) {
        assert!(r.minimized.validate().is_ok());
        assert!(pred(&r.minimized));
        assert!(r.minimized.actions.len() <= s.actions.len());
        assert!(r.minimized.extensions.len() <= s.extensions.len());
    }
}

/// Real execution of an arbitrary valid scenario (slow: boots the SVM). Bounded to short
/// scenarios. Must never panic; the outcome must be self-consistent. Without the `engine` feature
/// this target only parses (and does nothing else).
pub fn replay_scenario(data: &[u8]) {
    let Some(t) = text(data) else { return };
    let Ok(mut s) = Scenario::from_json(t) else {
        return;
    };
    if s.actions.len() > 6 {
        s.actions.truncate(6);
    }
    #[cfg(feature = "engine")]
    {
        let run = tokenscope_core::Engine::new().run_scenario(&s);
        match run.outcome {
            tokenscope_model::OutcomeClass::SetupError => {
                assert!(
                    run.trace.is_none() && run.findings.is_empty() && run.setup_error.is_some()
                );
            }
            _ => assert!(run.trace.is_some() && run.setup_error.is_none()),
        }
    }
}
