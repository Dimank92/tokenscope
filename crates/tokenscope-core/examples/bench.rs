//! Dependency-free benchmarks of the real pipeline.
//!
//! Run (needs `scripts/build-programs.sh` first):
//!
//! ```text
//! cargo run --release -p tokenscope-core --example bench            # optimized
//! cargo run           -p tokenscope-core --example bench            # unoptimized dev profile
//! ```
//!
//! Every benchmark does one untimed warm-up call, then `n` timed iterations, and prints
//! min / median / mean / p95 / max in milliseconds or microseconds. Compilation time is *not*
//! included; SVM startup is measured separately from steady-state execution. See
//! `docs/benchmarks.md` for how to read the output.

use std::{hint::black_box, process::Command, time::Instant};

use tokenscope_core::Engine;
use tokenscope_executor::{Harness, derive_address};
use tokenscope_invariants::{evaluate, findings};
use tokenscope_model::{Action, MintSpec, SCHEMA_VERSION, Scenario, TransferFeeParams};
use tokenscope_scenarios::{
    builtin_suite, extension_suite, generate, generate_extended, generate_guided,
};
use tokenscope_shrinker::ShrinkConfig;

const VULN: &str = "demo-escrow-vulnerable";

fn sh(cmd: &str, args: &[&str]) -> String {
    Command::new(cmd)
        .args(args)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".into())
}

fn cpuinfo(key: &str) -> String {
    std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|t| {
            t.lines()
                .find(|l| l.starts_with(key))
                .map(|l| l.split(':').nth(1).unwrap_or("").trim().to_string())
        })
        .unwrap_or_else(|| "unknown".into())
}

fn mem_total_mb() -> String {
    std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|t| {
            t.lines().find(|l| l.starts_with("MemTotal")).and_then(|l| {
                l.split_whitespace()
                    .nth(1)
                    .and_then(|k| k.parse::<u64>().ok())
            })
        })
        .map_or("unknown".into(), |kb| format!("{} MB", kb / 1024))
}

struct Row {
    name: String,
    unit: &'static str,
    items: usize,
    samples: Vec<f64>,
}

impl Row {
    fn print(&mut self) {
        self.samples
            .sort_by(|a, b| a.partial_cmp(b).expect("no NaN"));
        let n = self.samples.len();
        let at = |q: f64| self.samples[(((n - 1) as f64) * q).round() as usize];
        let mean = self.samples.iter().sum::<f64>() / n as f64;
        let per_sec = self.items as f64 / (at(0.5) / if self.unit == "ms" { 1e3 } else { 1e6 });
        println!(
            "| {:<46} | {:>4} | {:>10.3} | {:>10.3} | {:>10.3} | {:>10.3} | {:>10.3} | {:>12.1} |",
            self.name,
            n,
            at(0.0),
            at(0.5),
            mean,
            at(0.95),
            at(1.0),
            per_sec
        );
    }
}

/// Time `f` `iters` times (after one warm-up) and report in `unit`.
fn bench<T>(
    name: &str,
    unit: &'static str,
    items: usize,
    iters: usize,
    mut f: impl FnMut() -> T,
) -> Row {
    black_box(f());
    let scale = if unit == "ms" { 1e3 } else { 1e6 };
    let samples = (0..iters)
        .map(|_| {
            let t = Instant::now();
            black_box(f());
            t.elapsed().as_secs_f64() * scale
        })
        .collect();
    Row {
        name: name.into(),
        unit,
        items,
        samples,
    }
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
        extensions: vec![],
        actions: vec![
            Action::Withdraw,
            Action::Deposit { amount: 50_000 },
            Action::Deposit { amount: 3 },
            Action::Withdraw,
            Action::Deposit { amount: 4_000 },
        ],
    }
}

fn section(title: &str, unit: &str) {
    println!("\n### {title}\n");
    println!(
        "| {:<46} | {:>4} | {:>10} | {:>10} | {:>10} | {:>10} | {:>10} | {:>12} |",
        "benchmark",
        "n",
        format!("min {unit}"),
        format!("median {unit}"),
        format!("mean {unit}"),
        format!("p95 {unit}"),
        format!("max {unit}"),
        "items/s @med"
    );
    println!(
        "|{:-<48}|{:-<6}|{:-<12}|{:-<12}|{:-<12}|{:-<12}|{:-<12}|{:-<14}|",
        "", "", "", "", "", "", "", ""
    );
}

fn main() {
    println!("## Environment\n");
    println!(
        "- date (UTC): {}",
        sh("date", &["-u", "+%Y-%m-%dT%H:%M:%SZ"])
    );
    println!(
        "- cpu: {} x {} logical cpu(s), {} (as reported by /proc/cpuinfo)",
        cpuinfo("model name"),
        sh("nproc", &[]),
        if cpuinfo("flags").contains("avx2") {
            "avx2"
        } else {
            "no avx2"
        }
    );
    println!("- memory: {}", mem_total_mb());
    println!("- os: {}", sh("uname", &["-sr"]));
    println!("- rustc: {}", sh("rustc", &["--version"]));
    println!(
        "- profile: {}",
        if cfg!(debug_assertions) {
            "dev (unoptimized, debug assertions on)"
        } else {
            "release (optimized, debug assertions off)"
        }
    );
    println!(
        "- method: 1 untimed warm-up call, then n timed iterations; single thread; wall-clock"
    );

    let engine = Engine::new();
    let elf =
        std::fs::read(tokenscope_adapter::default_program_dir().join("demo_escrow_vulnerable.so"))
            .expect("run scripts/build-programs.sh first");

    // --- startup vs steady state, SVM level -------------------------------------------------
    section(
        "SVM startup and steady-state execution (executor level)",
        "us",
    );
    bench(
        "Harness::new() (Mollusk + system + Token-2022)",
        "us",
        1,
        30,
        Harness::new,
    )
    .print();
    let mut h = Harness::new();
    bench("load_program (escrow ELF, ~14 KB)", "us", 1, 30, || {
        h.load_program(&derive_address("p"), &elf)
    })
    .print();
    let spec = MintSpec {
        decimals: 6,
        transfer_fee: Some(TransferFeeParams {
            basis_points: 200,
            maximum_fee: u64::MAX,
        }),
    };
    bench(
        "fresh harness + fee mint + 2 token accounts + mint_to",
        "us",
        1,
        30,
        || {
            let h = Harness::new();
            let m = h.create_mint("m", &spec).unwrap();
            let a = h
                .create_token_account("a", &m, &derive_address("alice"))
                .unwrap();
            let _b = h
                .create_token_account("b", &m, &derive_address("bob"))
                .unwrap();
            h.mint_to(&m, &a, 1_000_000).unwrap();
        },
    )
    .print();
    let h = Harness::new();
    let (alice, bob) = (derive_address("alice"), derive_address("bob"));
    let m = h.create_mint("m", &spec).unwrap();
    let a = h.create_token_account("a", &m, &alice).unwrap();
    let b = h.create_token_account("b", &m, &bob).unwrap();
    h.mint_to(&m, &a, u64::MAX / 2).unwrap();
    let ix = h.transfer_checked_ix(&a, &m, &b, &alice, 1_000, 6).unwrap();
    bench(
        "TransferChecked with fee (steady state, 1 ix)",
        "us",
        1,
        2_000,
        || h.execute_logged(&ix),
    )
    .print();

    // --- generation --------------------------------------------------------------------------
    section("Scenario generation (pure CPU, no SVM)", "us");
    bench(
        "generate(1000 random fee scenarios)",
        "us",
        1000,
        50,
        || generate(1, 1000, VULN),
    )
    .print();
    bench(
        "generate_extended(1000 extension scenarios)",
        "us",
        1000,
        50,
        || generate_extended(1, 1000, VULN),
    )
    .print();
    let seeds: Vec<_> = builtin_suite(VULN)
        .into_iter()
        .chain(extension_suite(VULN))
        .collect();
    bench(
        "generate_guided(50 of ~300 candidates)",
        "us",
        50,
        20,
        || generate_guided(1, 50, VULN, &seeds),
    )
    .print();

    // --- invariants --------------------------------------------------------------------------
    let run = engine.run_scenario(&noisy());
    let trace = run.trace.clone().unwrap();
    section("Invariant evaluation (pure CPU)", "us");
    bench(
        "evaluate(trace of 5 steps) x 6 invariants",
        "us",
        1,
        5_000,
        || evaluate(&trace),
    )
    .print();
    let evals = evaluate(&trace);
    bench(
        "findings(scenario, trace, evaluations)",
        "us",
        1,
        5_000,
        || findings(&run.scenario, &trace, &evals),
    )
    .print();

    // --- end to end --------------------------------------------------------------------------
    section(
        "Scenario execution, replay, shrinking and campaigns (real SVM)",
        "ms",
    );
    let suite = builtin_suite(VULN);
    for name in [
        "control-no-fee",
        "fee-2pct-deposit",
        "fee-2pct-deposit-withdraw",
    ] {
        let s = suite.iter().find(|s| s.name == name).unwrap().clone();
        bench(&format!("run_scenario {name}"), "ms", 1, 30, || {
            engine.run_scenario(&s)
        })
        .print();
    }
    bench("run_scenario noisy (5 actions, fee)", "ms", 1, 30, || {
        engine.run_scenario(&noisy())
    })
    .print();
    let finding = run
        .findings
        .iter()
        .find(|f| f.invariant_id == "INV-ACCT-001")
        .unwrap()
        .clone();
    bench("replay_finding (re-execute + compare)", "ms", 1, 30, || {
        engine.replay_finding(&finding)
    })
    .print();
    let mut executions = 0;
    let mut row = bench("minimize noisy finding", "ms", 1, 5, || {
        let (_, r) = engine.minimize(&finding, ShrinkConfig::default()).unwrap();
        executions = r.attempts;
        r
    });
    row.name = format!("minimize noisy finding ({executions} executions)");
    row.items = executions;
    row.print();
    bench(
        &format!("run_campaign core suite ({} scenarios)", suite.len()),
        "ms",
        suite.len(),
        5,
        || engine.run_campaign(VULN, 0, &suite),
    )
    .print();
    let ext = extension_suite(VULN);
    bench(
        &format!("run_campaign extension suite ({} scenarios)", ext.len()),
        "ms",
        ext.len(),
        5,
        || engine.run_campaign(VULN, 0, &ext),
    )
    .print();
    let fifty = generate(1, 50, VULN);
    bench(
        "run_campaign 50 generated scenarios",
        "ms",
        fifty.len(),
        3,
        || engine.run_campaign(VULN, 1, &fifty),
    )
    .print();
    let campaign = engine.run_campaign(VULN, 0, &suite);
    bench("render_json(campaign report)", "ms", 1, 200, || {
        tokenscope_report::render_json(&campaign)
    })
    .print();
    bench(
        "end to end: generate 20 + run + render JSON",
        "ms",
        20,
        3,
        || {
            let sc = generate(2, 20, VULN);
            let c = engine.run_campaign(VULN, 2, &sc);
            tokenscope_report::render_json(&c)
        },
    )
    .print();
}
