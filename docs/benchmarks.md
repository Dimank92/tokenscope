# Benchmarks

Run: `cargo run -p tokenscope-core --example bench` (after `scripts/build-programs.sh`).
This is a small dependency-free harness (`crates/tokenscope-core/examples/bench.rs`) rather than
criterion, to avoid a heavy dependency and a second full build of the Agave crate tree; it reports
wall-clock min/median/mean over a fixed number of iterations after one warm-up run.

## Measured results

Environment: 1 vCPU, ~4 GB RAM sandbox VM, Linux x86_64, rustc 1.99.0, **unoptimized dev-profile
build with debug assertions** (the default for `cargo run`), single thread. Each measured item
includes full setup: a fresh Mollusk instance, Token-2022 and escrow program load, mint and account
creation through real instructions, then the scenario steps and invariant evaluation.

| Benchmark | iterations | min | median | mean |
|---|---|---|---|---|
| `run_scenario control-no-fee` | 30 | 61.8 ms | 63.6 ms | 64.8 ms |
| `run_scenario fee-2pct-deposit` | 30 | 63.1 ms | 66.4 ms | 68.3 ms |
| `run_scenario fee-2pct-deposit-withdraw` | 30 | 65.1 ms | 69.8 ms | 71.5 ms |
| `run_campaign` built-in suite (14 scenarios) | 5 | 907.6 ms | 939.9 ms | 966.0 ms |
| `run_campaign` 50 generated scenarios | 3 | 3396 ms | 3677 ms | 3684 ms |
| `minimize` noisy finding (21 executions) | 5 | 1483 ms | 1515 ms | 1539 ms |

Throughput is roughly 14-16 scenarios per second per thread in this configuration.

## How to read this

* Cost is dominated by per-scenario setup, not by the number of steps (adding a withdraw step
  costs about 6 ms).
* Optimized builds were **not** measured; they would be faster, but no number is claimed.
* Numbers come from a shared, noisy VM and a handful of iterations; treat them as order-of-magnitude.
* No comparison with other tools (Trident, Crucible) has been made, so nothing here supports a
  claim of being faster or slower than them.
* Campaigns are single-threaded; the scenario runs are independent, so parallelism is possible
  future work but has not been built or measured.
