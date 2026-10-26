//! Dependency-free benchmarks of the real pipeline.
//!
//! Run: `cargo run -p tokenscope-core --example bench` (needs `scripts/build-programs.sh` first).
//! Builds use whatever profile cargo selects (the default dev profile unless `--release` is given);
//! the report prints wall-clock statistics only. See `docs/benchmarks.md` for how to read them.

use std::{hint::black_box, time::Instant};

use tokenscope_core::Engine;
use tokenscope_model::{Action, MintSpec, SCHEMA_VERSION, Scenario, TransferFeeParams};
use tokenscope_scenarios::{builtin_suite, generate};
use tokenscope_shrinker::ShrinkConfig;

const VULN: &str = "demo-escrow-vulnerable";

fn stats(name: &str, items_per_iter: usize, samples: &mut [f64]) {
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let min = samples[0];
    let median = samples[samples.len() / 2];
    let mean = samples.iter().sum::<f64>() / samples.len() as f64;
    println!(
        "{name:<44} n={:<3} min {min:>9.2} ms  median {median:>9.2} ms  mean {mean:>9.2} ms  ({:.1} items/s at median)",
        samples.len(),
        items_per_iter as f64 / (median / 1000.0)
    );
}

fn time<T>(iters: usize, mut f: impl FnMut() -> T) -> Vec<f64> {
    black_box(f()); // warm-up (loads the ELFs, fills caches)
    (0..iters)
        .map(|_| {
            let t = Instant::now();
            black_box(f());
            t.elapsed().as_secs_f64() * 1000.0
        })
        .collect()
}

fn noisy() -> Scenario {
    Scenario {
        schema_version: SCHEMA_VERSION,
        name: "noisy".into(),
        seed: 12,
        target: VULN.into(),
        mint: MintSpec {
            decimals: 6,
            transfer_fee: Some(TransferFeeParams {
                basis_points: 777,
                maximum_fee: 123_456,
            }),
        },
        initial_balance: 9_999_999,
        actions: vec![
            Action::Withdraw,
            Action::Deposit { amount: 50_000 },
            Action::Deposit { amount: 3 },
            Action::Withdraw,
            Action::Deposit { amount: 4_000 },
        ],
    }
}

fn main() {
    let engine = Engine::new();
    println!(
        "profile: {}  (wall-clock, single thread; includes full per-scenario setup: mint, accounts, program load)",
        if cfg!(debug_assertions) {
            "unoptimized (debug assertions on)"
        } else {
            "optimized"
        }
    );

    let suite = builtin_suite(VULN);
    for name in [
        "control-no-fee",
        "fee-2pct-deposit",
        "fee-2pct-deposit-withdraw",
    ] {
        let s = suite.iter().find(|s| s.name == name).unwrap().clone();
        stats(
            &format!("run_scenario {name}"),
            1,
            &mut time(30, || engine.run_scenario(&s)),
        );
    }
    stats(
        &format!("run_campaign built-in suite ({} scenarios)", suite.len()),
        suite.len(),
        &mut time(5, || engine.run_campaign(VULN, 0, &suite)),
    );
    let generated = generate(1, 50, VULN);
    stats(
        "run_campaign 50 generated scenarios",
        generated.len(),
        &mut time(3, || engine.run_campaign(VULN, 1, &generated)),
    );

    let finding = engine
        .run_scenario(&noisy())
        .findings
        .into_iter()
        .find(|f| f.invariant_id == "INV-ACCT-001")
        .unwrap();
    let mut executions = 0;
    let mut samples = time(5, || {
        let (_, report) = engine.minimize(&finding, ShrinkConfig::default()).unwrap();
        executions = report.attempts;
        report
    });
    stats(
        &format!("minimize noisy finding ({executions} executions)"),
        executions,
        &mut samples,
    );
}
