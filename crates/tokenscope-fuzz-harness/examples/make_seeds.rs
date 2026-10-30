//! Regenerates the seed corpus under `fuzz/corpus/<target>/` from real engine output.
//!
//! Run: `cargo run -p tokenscope-fuzz-harness --example make_seeds` (needs the built programs).

use std::{fs, path::Path};

use tokenscope_core::Engine;
use tokenscope_model::{Campaign, CorpusCase, Scenario};
use tokenscope_scenarios::{builtin_suite, extension_suite};

fn write(root: &Path, target: &str, name: &str, body: &str) {
    let dir = root.join(target);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(name), body).unwrap();
}

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/corpus");
    let engine = Engine::new();
    let target = "demo-escrow-vulnerable";
    let scenarios: Vec<Scenario> = builtin_suite(target)
        .into_iter()
        .chain(extension_suite(target))
        .collect();

    for s in &scenarios {
        for t in ["parse_scenario", "shrink_scenario"] {
            write(&root, t, &format!("{}.json", s.id()), &s.to_json());
        }
    }
    for s in scenarios.iter().filter(|s| s.actions.len() <= 3).take(6) {
        write(
            &root,
            "replay_scenario",
            &format!("{}.json", s.id()),
            &s.to_json(),
        );
    }

    let runs: Vec<_> = scenarios
        .iter()
        .take(5)
        .map(|s| engine.run_scenario(s))
        .collect();
    for r in &runs {
        if let Some(t) = &r.trace {
            write(
                &root,
                "evaluate_trace",
                &format!("{}.json", r.scenario_id),
                &t.to_json(),
            );
        }
        for f in &r.findings {
            write(
                &root,
                "parse_finding",
                &format!("{}.json", f.id),
                &f.to_json(),
            );
        }
    }
    let campaign = Campaign::new(target.into(), 0, runs.into_iter().take(3).collect());
    write(
        &root,
        "parse_campaign",
        &format!("{}.json", campaign.id),
        &campaign.to_json(),
    );

    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/corpus");
    for e in fs::read_dir(corpus).unwrap() {
        let p = e.unwrap().path();
        let text = fs::read_to_string(&p).unwrap();
        let case = CorpusCase::from_json(&text).unwrap();
        write(
            &root,
            "parse_corpus_case",
            &format!("{}.json", case.id),
            &text,
        );
    }
}
