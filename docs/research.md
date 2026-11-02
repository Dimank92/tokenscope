# TokenScope research log

Status legend: **Verified** = read from primary source or executed; **Unverified** = not confirmed;
**Decision** = engineering choice made on the evidence. Dates are 2026 unless stated.

## 1. Mandatory references

| Reference | Result |
|---|---|
| https://solana.com/developers | Read. Developer hub; points at Surfpool for local testing. Notes SIMD-0268 (max CPI depth 4 -> 8). |
| https://solana.com/skills | Read. Foundation skills vs. community skills (community not endorsed). |
| https://github.com/solana-foundation/solana-dev-skill | Cloned at `33e2663` (2026-10-16). Read `testing.md`, `security.md`, `compatibility-matrix.md`, `programs/pinocchio.md`. Secondary source: crate versions below were re-checked against crates.io. |
| https://www.solana.new/ | **Unverified** - site disallows automated access (robots). Not relied on. |
| https://github.com/solana-foundation/awesome-solana-ai | Read; discovery only (community list, not an endorsement). |

## 2. Token-2022 transfer-fee semantics (Verified)

* Fee = `ceil(amount * basis_points / 10_000)`, capped at `maximum_fee`
  (`spl-token-2022-interface` 3.1.2, `extension/transfer_fee/mod.rs`).
* `TransferChecked` withholds the fee automatically (docs: transfer-fees page);
  `TransferCheckedWithFee` additionally asserts the caller's expected fee.
* `SetTransferFee` takes effect two epochs later (docs).
* `InitializeTransferFeeConfig` must run before `InitializeMint`.
* The ground truth used by TokenScope is **always the executed account state**, not the formula; the
  formula is only a cross-check.

## 3. Toolchain and dependency compatibility

Checked via crates.io metadata and by executing the toolchain (Verified unless noted).

| Item | Version | Evidence |
|---|---|---|
| rustc / cargo / clippy / rustfmt | 1.99.0 | conda-forge `rust=1.99.0` in an isolated env (`~/rust-env`); `rustc --version` executed |
| `mollusk-svm` | 0.16.2 | Apache-2.0; depends on Agave 4.3-line crates |
| `mollusk-svm-programs-token-2022` | 0.16.2 | ships mainnet-dump `token_2022.so` (slot 347196212, sha256 `b2a7ce1e...612da4`); `extensions` feature builds `TransferFeeConfig` accounts |
| `litesvm` | 0.18.1 | Apache-2.0, MSRV 1.97.1; no bundled Token-2022 |
| `spl-token-2022-interface` | 3.1.2 | instruction builders / state / fee math |
| `cargo-build-sbf` | 4.5.0 | crates.io owner `anza-team`, repo `anza-xyz/cargo-build-sbf`; built with `--locked` |
| platform-tools | v1.58 | `platform-tools-linux-x86_64.tar.bz2` sha256 `7b82eb42...5636c37` equals the digest GitHub publishes for the asset; bundled rustc `1.95.0-dev` |
| `pinocchio` | 0.11.2 | program framework (see decisions) |

Environment notes: the sandbox has 1 CPU, 4 GB RAM and limited disk; `static.rust-lang.org` and
`release.anza.xyz` are blocked by the egress allowlist, which is why conda-forge and GitHub
releases were used. Background processes do not survive between tool calls, so long builds run in
foreground chunks.

## 4. RTK

RTK v1.0.1 installed with the project's official `install.sh` pinned via `RTK_VERSION=v1.0.1`; the
script verifies the archive SHA-256 against the release `checksums.txt` and rejects unsafe archive paths.
`rtk --version` and `rtk gain` run. No agent hook is available in this environment, so RTK is
invoked explicitly (`rtk cargo ...`, `rtk git ...`); raw output is used whenever full diagnostics matter.
RTK is a development tool only and is not a TokenScope dependency.

## 5. Ecosystem audit

Evidence gathered from vendor docs, changelogs, crates.io/docs.rs pages and guides (2026-10-24/25).
"Not verified" means the capability was not found in what was read, not that it is absent.

| Tool | What the evidence shows | Overlap and limits |
|---|---|---|
| **Trident** (Ackee) 0.12.0 | Manually guided fuzzer for **Anchor** programs; generates harnesses from the IDL; `#[init]/#[flow]/#[end]`; invariants as asserts; honggfuzz under the hood; crash-file replay via `fuzz run-debug`; metrics dashboard. Changelog 0.12.0 (2025-11-27): "add support for Token 2022 program". `trident-fuzz` feature `token` exposes `initialize_mint_2022` with extensions (TransferFeeConfig listed), account extensions (ImmutableOwner, MemoTransfer, CpiGuard), `get_mint`/`get_token_account`. A Quicknode/Ackee guide fuzzes a token escrow. | **Substantial overlap**: fuzzing + invariants + Token-2022 helpers. No crash *minimization* feature was found in the docs read (only seed/crash-file replay); not verified. Anchor-centric (IDL driven). |
| **Crucible** (Asymmetric Research), public beta, `crucible-fuzz-cli` 0.2.1-0.3.0-alpha.1 | Coverage-guided invariant fuzzer on **LiteSVM** with sBPF tracing; works on any compiled `.so`; `#[invariant_test]` after every action; `crucible tmin` minimizes a crash to the smallest action sequence; `cmin` minimizes the corpus; slated for `anchor fuzz` in Anchor 2.0 (anchor-lang.com docs list `anchor fuzz tmin`). | **Largest overlap** with TokenScope's pipeline *and* shrinking. Token-2022-specific scenario support not verified. Far more powerful general fuzzer (coverage guidance, multi-core). |
| svmScope 0.6.0 | Mainnet-transaction replay and mutation on LiteSVM, HTML report | Replay/forensics; no Token-2022 invariant engine seen. |
| Surfpool | Local validator with mainnet forking (per solana-dev-skill) | Integration environment, not an invariant tester. Docs not read directly. |
| Solana Upgrade Guard, EPIC, layoutd | Upgrade and account-layout safety | Different problem. |
| `cf-invariants-anchor` 0.1.1 | AI-assisted invariant suggestion emitting Crucible fixtures | Complementary (invariant authoring). |
| Ratchet, Termina | **Unverified** - could not be identified | Need links. |

### Differentiation (evidence-based, deliberately modest)

TokenScope is **not** the first Solana invariant fuzzer, not the first to have Token-2022 helpers
(Trident 0.12.0), and not the first with crash minimization (Crucible `tmin`). It is also not
coverage-guided. What it offers that the audit did not find elsewhere (absence of evidence only):

1. A **domain-specific scenario model** for Token-2022 integrations (fee parameters, rounding and
   cap boundaries, zero and 100% fees) with **executed-state ground truth**: expected values are
   decoded from real Token-2022 account data after running the real program, never from a formula.
2. An explicit **four-way outcome taxonomy** (setup error / execution failure / invariant violation
   / suspected vulnerability) enforced in code and tests, so harness problems cannot masquerade
   as findings.
3. **Adapter-defined semantics** (a program ID alone says nothing) plus **self-contained,
   content-addressed scenarios and findings** with deterministic replay, bounded shrinking against
   the same real-execution predicate, and a committed regression corpus.
4. A small, auditable Rust code base with a CLI/TUI that show the same campaign data.

Whether these are meaningful advantages over Trident or Crucible for real users is untested; no
benchmark against them has been run. Wording in the README must not claim uniqueness.

## 6. Decisions

1. Native (non-Anchor) escrow using `pinocchio`: smallest dependency surface, hand-checkable CPI.
2. Demo program is excluded from the host workspace and built with `cargo build-sbf --tools-version v1.58`.
3. SVM framework chosen by a time-boxed experiment in M2: **Mollusk** (see `docs/experiments/svm-framework.md`).
4. Findings and scenarios are versioned JSON (`FORMAT.md`).
5. Patches live outside the repo; canonical manifest and README live in the delivery directory.
6. `ratatui` 0.30.2 re-exports `crossterm` (`ratatui::crossterm`), so the TUI uses that re-export and
   the crossterm version cannot drift from the one ratatui was built against (this resolves the
   earlier open question).
7. Edition 2024 / `unsafe_code = forbid` for host crates; the SBF program uses `deny` because the
   `entrypoint!` macro expands to unsafe code.
8. Extension behavior is **observed on the runtime first**, then compared with the docs. Differences found:
   the docs' `NonTransferable` + `TransferFeeConfig` incompatibility example is not enforced by the bundled
   program (accepted at creation); NonTransferable mints force `ImmutableOwner` on token accounts;
   duplicate extension initializers before `InitializeMint` are accepted. See `docs/supported-extensions.md`.
9. Fuzzing uses libFuzzer on the **stable** compiler with the SanitizerCoverage flags `cargo fuzz` applies,
   because nightly was not installable in the sandbox (official hosts blocked); no AddressSanitizer.
10. A vulnerability advisory (RUSTSEC-2026-0387) is reachable only via Agave's secp256k1 syscall path and is
    recorded as an accepted, justified exception (`docs/dependency-review.md`).
