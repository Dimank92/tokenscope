# Architecture

## Data flow

```
Scenario (JSON, validated)
   |  tokenscope-core::Engine::run_scenario
   v
Target adapter (tokenscope-adapter)  --builds-->  instructions + accounts
   |  uses tokenscope-executor::Harness (Mollusk SVM, real system + Token-2022 + target ELF)
   v
ExecutionTrace  (real before/after Snapshots, step statuses, program logs, compute units)
   |  tokenscope-invariants::{evaluate, classify, findings}
   v
RunRecord { outcome, findings }  ->  Campaign  ->  Report / TUI / corpus / shrinker
```

Setup problems (invalid scenario, unknown target, missing program ELF, harness failure) stop at the
adapter and become `OutcomeClass::SetupError`; they never reach the invariant engine and never
produce findings.

## Crates

| Crate | Responsibility | Depends on |
|---|---|---|
| `tokenscope-model` | Serializable, validated types: `Scenario`, `Action`, `ExecutionTrace`, `Finding`, `Campaign`, `CorpusCase`, `OutcomeClass`; content-derived IDs; schema versioning | serde, sha2, thiserror |
| `tokenscope-executor` | `Harness`: creates Token-2022 mints/accounts via real instructions, loads program ELFs, executes with log capture, decodes real account state | mollusk-svm, spl-token-2022-interface |
| `tokenscope-adapter` | `EscrowAdapter`: deploys the escrow, creates accounts, builds instructions, records snapshots | executor, model |
| `tokenscope-invariants` | Pure functions over traces; documented preconditions/exceptions; finding construction | model |
| `tokenscope-scenarios` | Built-in suite and seeded SplitMix64 generator | model |
| `tokenscope-shrinker` | Generic bounded deterministic minimizer driven by a failure predicate | model |
| `tokenscope-core` | `Engine` (run, replay, minimize, campaign) and the on-disk corpus | adapter, invariants, shrinker |
| `tokenscope-report` | Versioned JSON/text reports, artifact files | model |
| `tokenscope-cli` | `tokenscope` binary; exit codes; no dependency on the TUI for its own commands | all of the above, `tokenscope-tui` for `tui` |
| `tokenscope-tui` | Ratatui interface: pure `App` state, worker-thread `runner`, `ui` rendering, terminal `driver` | core, report, invariants |
| `programs/demo-escrow` | On-chain demo (pinocchio), separate lockfile, built by `scripts/build-programs.sh` | pinocchio 0.11.2 |

Design rules:

* Execution logic never lives in rendering code; the TUI's `App` is pure and tested without a terminal.
* Reports, CLI output and the TUI are views of the same `Campaign` value (no timestamps in persisted
  data), which is why they cannot disagree.
* Ground truth is executed account state. The fee formula appears only in tests, as an independent
  cross-check.

## Decisions (see `docs/research.md` for evidence)

* **Mollusk over LiteSVM** (experiment: `docs/experiments/svm-framework.md`).
* **Native escrow with pinocchio**, no Anchor: smallest dependency surface.
* **Program kept out of the host workspace**: different toolchain (platform-tools Rust), own lockfile.
* **Scenario/finding IDs are content hashes** so artifacts are self-verifying and dedupe naturally.
* **Shrinker is generic** over a predicate: it is unit/property-tested without the SVM and driven by
  real execution in `tokenscope-core`.
* **Single failure atomicity model**: Mollusk's context persists state only for successful
  instructions, matching Solana transaction semantics for the single-instruction steps used here.

## Extension points

A new target needs: an adapter (setup, instruction builders, snapshot decoding), scenario
templates, and invariants over its trace type. The current `ExecutionTrace`/`Snapshot` types are
escrow-shaped; generalizing them is future work and is deliberately not pre-built.
