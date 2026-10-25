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

## Patch delivery

The history is delivered as an ordered `git format-patch` series **outside** the source tree
(default `../tokenscope-patches`), with the canonical `PATCH_MANIFEST.md` and `PATCHES_README.md`
generated there:

```sh
./scripts/export-patches.sh ../tokenscope-patches     # regenerate after ANY commit or history change
./scripts/verify-reconstruction.sh ../tokenscope-patches --run-tests
```

`verify-reconstruction.sh` creates a fresh repository, applies every patch listed in `series` with
`git am`, then checks commit count, per-commit **tree hash, author, author date and subject**, and
the final tracked files against the source repository; with `--run-tests` it also builds the
programs and runs fmt, clippy and the tests inside the reconstructed repository. Commit hashes
necessarily differ after `git am`, so they are not compared. Patches must never depend on
uncommitted files.

## Development aids

RTK (https://github.com/rtk-ai/rtk) may be used to compress verbose command output
(`rtk cargo test`, `rtk git status`). It is not a dependency. Use raw output whenever you need
complete diagnostics.
