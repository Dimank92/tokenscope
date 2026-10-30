//! Runs every fuzz-target body on stable over the seed corpus and deterministic mutations, plus
//! random bytes. This is NOT coverage-guided fuzzing; it guards the targets and the seeds.

use std::{fs, path::Path};

use proptest::prelude::*;
use tokenscope_fuzz_harness::{TARGETS, run_target};
use tokenscope_scenarios::SplitMix64;

fn seeds(target: &str) -> Vec<Vec<u8>> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fuzz/corpus")
        .join(target);
    let mut files: Vec<_> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .collect();
    files.sort();
    files.iter().map(|p| fs::read(p).unwrap()).collect()
}

fn mutate(data: &[u8], rng: &mut SplitMix64) -> Vec<u8> {
    let mut b = data.to_vec();
    if b.is_empty() {
        return b;
    }
    let i = rng.below(b.len() as u64) as usize;
    match rng.below(5) {
        0 => b.truncate(i),
        1 => b[i] = rng.next_u64() as u8,
        2 => {
            b.remove(i);
        }
        3 => b.insert(i, rng.next_u64() as u8),
        _ => {
            // Replace a digit run with a boundary number to probe numeric edge cases.
            let s = String::from_utf8_lossy(&b).into_owned();
            let pos = s
                .char_indices()
                .filter(|(_, c)| c.is_ascii_digit())
                .map(|(p, _)| p)
                .nth(rng.below(8) as usize);
            if let Some(p) = pos {
                let repl = [
                    "0",
                    "1",
                    "18446744073709551615",
                    "18446744073709551616",
                    "-1",
                    "4294967296",
                ][rng.below(6) as usize];
                let mut t = s.clone();
                t.insert_str(p, repl);
                return t.into_bytes();
            }
        }
    }
    b
}

#[test]
fn every_seed_is_accepted_by_its_own_target_and_survives_mutation() {
    for target in TARGETS {
        let seeds = seeds(target);
        assert!(!seeds.is_empty(), "{target} has no seeds");
        // The slow target runs the real SVM: fewer mutants.
        let mutants = if target == "replay_scenario" { 2 } else { 60 };
        let mut rng = SplitMix64::new(0xF00D);
        for seed in &seeds {
            run_target(target, seed);
            for _ in 0..mutants {
                run_target(target, &mutate(seed, &mut rng));
            }
        }
    }
}

#[test]
fn seeds_exercise_the_accepting_paths() {
    // Guard against seeds silently rotting: the real seeds must parse.
    use tokenscope_model::{Campaign, CorpusCase, ExecutionTrace, Finding, Scenario};
    let ok = |t: &str, f: &dyn Fn(&str) -> bool| {
        seeds(t).iter().all(|s| f(std::str::from_utf8(s).unwrap()))
    };
    assert!(ok("parse_scenario", &|s| Scenario::from_json(s).is_ok()));
    assert!(ok("parse_finding", &|s| Finding::from_json(s).is_ok()));
    assert!(ok("parse_campaign", &|s| Campaign::from_json(s).is_ok()));
    assert!(ok("parse_corpus_case", &|s| CorpusCase::from_json(s).is_ok()));
    assert!(ok("evaluate_trace", &|s| ExecutionTrace::from_json(s).is_ok()));
}

#[test]
fn evaluate_trace_survives_extreme_numeric_values() {
    let text = seeds("evaluate_trace")
        .into_iter()
        .map(|b| String::from_utf8(b).unwrap())
        .find(|t| t.contains("\"vault_amount\": 980") && t.contains("\"escrow_recorded\": 1000"))
        .expect("a seed trace with the fee-bug numbers");
    for v in ["18446744073709551615", "0", "1"] {
        let mutated = text
            .replace("\"vault_amount\": 980", &format!("\"vault_amount\": {v}"))
            .replace(
                "\"escrow_recorded\": 1000",
                &format!("\"escrow_recorded\": {v}"),
            );
        assert_ne!(
            mutated, text,
            "the substitution must actually change the trace"
        );
        run_target("evaluate_trace", mutated.as_bytes());
    }
}

proptest! {
    #[test]
    fn random_bytes_never_panic_the_parsing_targets(data in proptest::collection::vec(any::<u8>(), 0..400)) {
        for target in ["parse_scenario", "parse_finding", "parse_campaign", "parse_corpus_case", "evaluate_trace", "shrink_scenario"] {
            run_target(target, &data);
        }
    }
}
