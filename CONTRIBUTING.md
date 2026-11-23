# Contributing

## Setup

See [docs/toolchain.md](docs/toolchain.md). In short: Rust 1.99.0, `cargo-build-sbf` 4.5.0 and
platform-tools v1.58. Then:

```sh
./scripts/build-programs.sh    # required before running the tests that execute the escrow
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

CI (`.github/workflows/ci.yml`: lint, test with release/TUI/fuzz-smoke checks, audit) mirrors these gates on GitHub Actions. Other checks: `scripts/release-check.sh`,
`scripts/check-tui-terminal.py`, `scripts/fuzz.sh`, and `cargo audit` (exceptions live in `.cargo/audit.toml` with their
justification in `docs/dependency-review.md`).

Never silence a warning or weaken a test to get a green build. If a test expectation is wrong,
fix the expectation and say why in the commit message.

## Principles

* Execute real programs; never hand-edit balances or simulate fees. Ground truth is decoded
  account state.
* Keep the four outcome classes distinct (setup error / execution failure / invariant violation /
  suspected vulnerability). Setup problems must never become findings.
* Every invariant documents preconditions, expected behavior and legitimate exceptions
  (`docs/invariants.md`) and has held / violated / not-applicable tests.
* Only claim support for behavior verified by an executable test (`docs/supported-extensions.md`).
* Keep dependencies minimal and justified; prefer safe Rust and structured errors (`thiserror`).
* No fabricated metrics, results or security claims in code, docs or commit messages.

## Commits

One coherent, independently understandable change per commit, with a descriptive message
(conventional prefixes: `feat`, `fix`, `test`, `docs`, `build`, `chore`). Run the gates above before
committing. Do not commit secrets, keypairs, caches or build artifacts (`target/`).

## Pull Requests

Contributions follow standard GitHub fork-and-pull-request workflows:

1. Create a descriptive feature branch from `main`.
2. Ensure `./scripts/build-programs.sh`, `cargo test --workspace`, and `cargo clippy --workspace --all-targets -- -D warnings` pass cleanly.
3. Open a Pull Request against `main`. All CI checks must pass before merging.

## Development aids

RTK (https://github.com/rtk-ai/rtk) may be used to compress verbose command output
(`rtk cargo test`, `rtk git status`). It is not a dependency. Use raw output whenever you need
complete diagnostics.
