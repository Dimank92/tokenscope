# Benchmarks

All numbers below are copied verbatim from the output of the benchmark program; nothing is
estimated. They come from one shared 1-vCPU VM, so treat them as order-of-magnitude, and no
comparison with other tools has been made.

## How to reproduce

```sh
./scripts/build-programs.sh                                         # once
cargo run --release -p tokenscope-core --example bench              # optimized
cargo run           -p tokenscope-core --example bench              # dev profile (unoptimized)
```

`crates/tokenscope-core/examples/bench.rs` is dependency-free. Each benchmark does one untimed
warm-up call and then `n` timed iterations on a single thread (wall clock); it prints
min / median / mean / p95 / max and items per second at the median. The program also prints the
hardware, OS, compiler and profile it ran under (included in each section below).

## Startup, compilation and steady state are separate

* **Compilation** is not in any timing. Measured on this VM: the optimized build of the CLI and
  the bench (`cargo build --release -p tokenscope-core --example bench -p tokenscope-cli`, all
  dependencies included, cold) took about **350 s wall time** over two tool invocations
  (including a short idle gap); a cold dev build of the dependency tree took about 3 minutes
  earlier in this project.
* **SVM startup** (`Harness::new()`) is timed on its own and is paid once per scenario.
* **Steady state** is a single instruction on an already-built harness
  (`TransferChecked with fee`).
* Every `run_scenario`/campaign number includes the full per-scenario startup and setup.

## Results: optimized (release profile, debug assertions off)

## Environment

- date (UTC): 2026-10-30T10:02:34Z
- cpu: Intel(R) Xeon(R) Processor @ 2.10GHz x 1 logical cpu(s), avx2 (as reported by /proc/cpuinfo)
- memory: 3995 MB
- os: Linux 6.18.44-fc-v185
- rustc: rustc 1.99.0 (b940084d7 2026-09-28)
- profile: release (optimized, debug assertions off)
- method: 1 untimed warm-up call, then n timed iterations; single thread; wall-clock

### SVM startup and steady-state execution (executor level)

| benchmark                                      |    n |     min us |  median us |    mean us |     p95 us |     max us | items/s @med |
|------------------------------------------------|------|------------|------------|------------|------------|------------|--------------|
| Harness::new() (Mollusk + system + Token-2022) |   30 |   3006.898 |   3671.850 |   3702.219 |   4307.646 |   4535.235 |        272.3 |
| load_program (escrow ELF, ~14 KB)              |   30 |     61.561 |     66.626 |     79.263 |    126.398 |    205.855 |      15009.2 |
| fresh harness + fee mint + 2 token accounts + mint_to |   30 |   2969.126 |   3225.493 |   3986.487 |   5207.439 |   7093.353 |        310.0 |
| TransferChecked with fee (steady state, 1 ix)  | 2000 |     43.416 |     57.813 |     70.463 |    162.564 |    344.564 |      17297.1 |

### Scenario generation (pure CPU, no SVM)

| benchmark                                      |    n |     min us |  median us |    mean us |     p95 us |     max us | items/s @med |
|------------------------------------------------|------|------------|------------|------------|------------|------------|--------------|
| generate(1000 random fee scenarios)            |   50 |    238.307 |    272.562 |    274.694 |    320.281 |    338.731 |    3668890.0 |
| generate_extended(1000 extension scenarios)    |   50 |    360.186 |    408.042 |    410.357 |    453.890 |    485.906 |    2450728.1 |
| generate_guided(50 of ~300 candidates)         |   20 |  33744.895 |  34781.378 |  34657.602 |  35334.938 |  35389.696 |       1437.6 |

### Invariant evaluation (pure CPU)

| benchmark                                      |    n |     min us |  median us |    mean us |     p95 us |     max us | items/s @med |
|------------------------------------------------|------|------------|------------|------------|------------|------------|--------------|
| evaluate(trace of 5 steps) x 6 invariants      | 5000 |      0.861 |      1.259 |      1.325 |      1.370 |     77.346 |     794281.2 |
| findings(scenario, trace, evaluations)         | 5000 |      6.295 |      8.071 |      8.924 |     12.909 |    126.977 |     123900.4 |

### Scenario execution, replay, shrinking and campaigns (real SVM)

| benchmark                                      |    n |     min ms |  median ms |    mean ms |     p95 ms |     max ms | items/s @med |
|------------------------------------------------|------|------------|------------|------------|------------|------------|--------------|
| run_scenario control-no-fee                    |   30 |      6.965 |      7.343 |      7.323 |      7.911 |      7.930 |        136.2 |
| run_scenario fee-2pct-deposit                  |   30 |      6.856 |      7.572 |      7.671 |      8.377 |      8.435 |        132.1 |
| run_scenario fee-2pct-deposit-withdraw         |   30 |      7.125 |      7.468 |      7.424 |      7.879 |      8.331 |        133.9 |
| run_scenario noisy (5 actions, fee)            |   30 |      5.666 |      5.959 |      6.411 |      8.137 |      8.446 |        167.8 |
| replay_finding (re-execute + compare)          |   30 |      5.573 |      5.790 |      5.952 |      6.627 |      8.549 |        172.7 |
| minimize noisy finding (21 executions)         |    5 |    109.788 |    112.944 |    113.471 |    118.228 |    118.228 |        185.9 |
| run_campaign core suite (14 scenarios)         |    5 |     53.470 |     54.224 |     54.038 |     54.565 |     54.565 |        258.2 |
| run_campaign extension suite (10 scenarios)    |    5 |     40.041 |     40.786 |     40.651 |     40.971 |     40.971 |        245.2 |
| run_campaign 50 generated scenarios            |    3 |    191.903 |    194.146 |    194.634 |    197.852 |    197.852 |        257.5 |
| render_json(campaign report)                   |  200 |      0.050 |      0.051 |      0.056 |      0.077 |      0.128 |      19497.4 |
| end to end: generate 20 + run + render JSON    |    3 |     80.255 |     82.342 |     81.746 |     82.641 |     82.641 |        242.9 |

### Reading the optimized results

* A complete scenario costs about 6-8 ms, of which about 3.7 ms is SVM startup
  (`Harness::new()`); a steady-state `TransferChecked` is about 58 us. Startup dominates, so
  per-scenario cost is nearly independent of the number of steps.
* Pure-CPU components are cheap: evaluating all six invariants over a 5-step trace takes about
  1.3 us, building findings about 8 us, generating 1,000 scenarios about 0.27-0.41 ms.
  `generate_guided` is the heavy generator (about 35 ms for 50 scenarios) because it scores
  candidates greedily.
* Replay (`replay_finding`) costs one scenario run (about 5.8 ms). Minimizing the noisy finding
  took 21 real executions in about 113 ms.
* Differences of a millisecond between rows with the same work (for example the 5-action `noisy`
  scenario running faster than `control-no-fee`) are noise or warm-up effects, not meaningful.

## Results: dev profile (unoptimized, debug assertions on) - baseline verification

Last round reported about 64 ms per scenario for this profile (medians 63.6 / 66.4 / 69.8 ms for
the three single-scenario runs). Re-measured below: medians of about 59-64 ms, i.e. the reported
baseline holds within the noise of a shared VM. The optimized build is roughly 8-10x faster.

## Environment

- date (UTC): 2026-10-30T09:55:36Z
- cpu: Intel(R) Xeon(R) Processor @ 2.10GHz x 1 logical cpu(s), avx2 (as reported by /proc/cpuinfo)
- memory: 3995 MB
- os: Linux 6.18.44-fc-v185
- rustc: rustc 1.99.0 (b940084d7 2026-09-28)
- profile: dev (unoptimized, debug assertions on)
- method: 1 untimed warm-up call, then n timed iterations; single thread; wall-clock

### SVM startup and steady-state execution (executor level)

| benchmark                                      |    n |     min us |  median us |    mean us |     p95 us |     max us | items/s @med |
|------------------------------------------------|------|------------|------------|------------|------------|------------|--------------|
| Harness::new() (Mollusk + system + Token-2022) |   30 |  32442.827 |  34146.645 |  36184.787 |  46777.771 |  54535.640 |         29.3 |
| load_program (escrow ELF, ~14 KB)              |   30 |    752.763 |    809.271 |    832.396 |    910.697 |   1063.070 |       1235.7 |
| fresh harness + fee mint + 2 token accounts + mint_to |   30 |  42142.982 |  44357.550 |  46950.663 |  65281.739 |  80416.288 |         22.5 |
| TransferChecked with fee (steady state, 1 ix)  | 2000 |   1192.989 |   1343.169 |   1430.461 |   1992.998 |   7526.057 |        744.5 |

### Scenario generation (pure CPU, no SVM)

| benchmark                                      |    n |     min us |  median us |    mean us |     p95 us |     max us | items/s @med |
|------------------------------------------------|------|------------|------------|------------|------------|------------|--------------|
| generate(1000 random fee scenarios)            |   50 |    380.003 |    402.898 |    403.318 |    445.953 |    467.228 |    2482017.8 |
| generate_extended(1000 extension scenarios)    |   50 |    512.510 |    554.240 |    573.639 |    704.520 |    918.811 |    1804272.5 |
| generate_guided(50 of ~300 candidates)         |   20 |  87517.327 |  97129.661 | 113945.716 | 155128.066 | 169416.433 |        514.8 |

### Invariant evaluation (pure CPU)

| benchmark                                      |    n |     min us |  median us |    mean us |     p95 us |     max us | items/s @med |
|------------------------------------------------|------|------------|------------|------------|------------|------------|--------------|
| evaluate(trace of 5 steps) x 6 invariants      | 5000 |      1.805 |      1.884 |      2.482 |      2.853 |    952.771 |     530785.6 |
| findings(scenario, trace, evaluations)         | 5000 |     53.885 |     55.354 |     63.221 |     89.956 |    833.007 |      18065.5 |

### Scenario execution, replay, shrinking and campaigns (real SVM)

| benchmark                                      |    n |     min ms |  median ms |    mean ms |     p95 ms |     max ms | items/s @med |
|------------------------------------------------|------|------------|------------|------------|------------|------------|--------------|
| run_scenario control-no-fee                    |   30 |     53.232 |     59.398 |     61.992 |     85.988 |     86.378 |         16.8 |
| run_scenario fee-2pct-deposit                  |   30 |     53.015 |     62.187 |     67.046 |     87.747 |     88.728 |         16.1 |
| run_scenario fee-2pct-deposit-withdraw         |   30 |     56.296 |     61.497 |     63.970 |     84.650 |     86.050 |         16.3 |
| run_scenario noisy (5 actions, fee)            |   30 |     58.876 |     63.756 |     67.024 |     90.092 |     90.527 |         15.7 |
| replay_finding (re-execute + compare)          |   30 |     57.756 |     64.761 |     66.298 |     82.720 |     87.126 |         15.4 |
| minimize noisy finding (21 executions)         |    5 |   1285.508 |   1444.077 |   1410.276 |   1471.103 |   1471.103 |         14.5 |
| run_campaign core suite (14 scenarios)         |    5 |    724.482 |    797.253 |    822.523 |    921.900 |    921.900 |         17.6 |
| run_campaign extension suite (10 scenarios)    |    5 |    650.785 |    732.673 |    715.542 |    756.883 |    756.883 |         13.6 |
| run_campaign 50 generated scenarios            |    3 |   3406.340 |   3797.491 |   3738.806 |   4012.588 |   4012.588 |         13.2 |
| render_json(campaign report)                   |  200 |      1.028 |      1.234 |      1.367 |      2.039 |      3.531 |        810.6 |
| end to end: generate 20 + run + render JSON    |    3 |   1217.842 |   1295.437 |   1302.352 |   1393.778 |   1393.778 |         15.4 |

## Limitations

* One VM, one CPU, a handful of iterations for the slower rows (n is shown); no confidence
  intervals are computed.
* Campaigns are single-threaded. Scenario runs are independent, so parallelism is possible
  future work; it has not been built or measured.
* Link-time optimization and `codegen-units=1` were not tried; the default release profile was used.
