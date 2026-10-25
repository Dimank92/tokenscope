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

## 5. Ecosystem audit (in progress)

| Tool | What the evidence shows | Overlap / limits |
|---|---|---|
| Trident (Ackee) | Rust fuzzer for Anchor programs on the Anza SVM; honggfuzz; invariant checks; regression runs | Substantial overlap on SVM fuzzing + invariants. Token-2022 and shrinking support **still being verified** (see section 6). |
| svmScope 0.6.0 | Mainnet-transaction replay and mutation on LiteSVM, HTML report | Replay/forensics tool; no Token-2022 invariant engine seen. |
| Surfpool | Local validator with mainnet forking (per solana-dev-skill) | Integration environment, not an invariant tester. Docs not read directly. |
| Solana Upgrade Guard, EPIC, layoutd | Upgrade and account-layout safety | Different problem. |
| Ratchet, Termina | **Unverified** - could not be identified | Need links. |

No uniqueness claim is made.

## 6. Decisions

1. Native (non-Anchor) escrow using `pinocchio`: smallest dependency surface, hand-checkable CPI.
2. Demo program is excluded from the host workspace and built with `cargo build-sbf --tools-version v1.58`.
3. SVM framework chosen by a time-boxed experiment in M2 (Mollusk default).
4. Findings and scenarios are versioned JSON (`FORMAT.md`).
5. Patches live outside the repo; canonical manifest and README live in the delivery directory.
