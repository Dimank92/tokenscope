# TokenScope

TokenScope is a Rust framework for **adversarial testing of Solana Token-2022 integrations**. It
executes real programs in a real Solana Virtual Machine (the Mollusk SVM with the real Token-2022
program), checks explicit invariants against *decoded on-chain account state*, minimizes failures,
and writes reproducible findings.

```
Target adapter -> Scenarios -> Real SVM execution -> Invariant checking -> Shrinking -> Findings
```

> **Status:** early (version 0.1.0). One target (a demo escrow), Token-2022 `TransferFeeConfig`
> plus freeze/thaw of token accounts, five invariants. See [Limitations](#limitations) and
> [docs/supported-extensions.md](docs/supported-extensions.md) before relying on any result.
> TokenScope does not detect vulnerabilities in general, and an invariant violation is **not**
> proof of exploitability. It is also not the first tool in this space; see
> [docs/research.md](docs/research.md) for an evidence-based comparison with Trident and Crucible.

## The demonstration

Token-2022 transfer fees mean the amount a program *sends* is not the amount that *arrives*. The
demo escrow ships in two flavors built from one source (`programs/demo-escrow`):

* **vulnerable** credits the nominal deposit amount;
* **corrected** (`--features corrected`) credits the vault's real balance change.

With a 2% fee, depositing 1,000 credits 1,000 on the vulnerable escrow while the vault holds 980
(20 withheld by Token-2022). The same scenario passes on the corrected escrow.

```text
$ tokenscope run --target demo-escrow-vulnerable --count 0     # exit code 1
executed 14 | pass 4 | execution failures 2 | invariant violations 8 | setup errors 0 | findings 16
...
fd_eb4facd442285199 INV-ACCT-001 [Medium] target=demo-escrow-vulnerable scenario=sc_3573a0f4384e7945 seed=0
  observed: step 0: deposit of 1000 credited 1000 but the vault received 980 (withheld fee delta 20)

$ tokenscope run --target demo-escrow-corrected --count 0      # exit code 0
executed 14 | pass 12 | execution failures 2 | invariant violations 0 | setup errors 0 | findings 0
```

Full walkthrough with replay and minimization: [docs/demo.md](docs/demo.md).

## Quick start

Requirements: Rust 1.99.0 (MSRV 1.97), and for building the on-chain demo programs
`cargo-build-sbf` 4.5.0 plus Solana platform-tools **v1.58**
(see [docs/toolchain.md](docs/toolchain.md) for exact, verified installation steps).

```sh
./scripts/build-programs.sh          # builds both escrow flavors from source into target/sbf-programs/
cargo build -p tokenscope-cli        # debug build (release builds have not been tested)
export PATH="$PWD/target/debug:$PATH"

tokenscope doctor                    # verifies programs, bundled Token-2022 and real execution
tokenscope inspect                   # targets, accounts, instructions, invariants
tokenscope run --target demo-escrow  # runs both flavors; exit 1 because the vulnerable one violates
tokenscope tui                       # interactive UI
cargo test --workspace               # all tests (needs the built programs)
```

## Commands

| Command | Purpose | Exit codes |
|---|---|---|
| `doctor` | verify the environment, incl. a real fee transfer | 0 ok, 3 problem |
| `inspect [--target T]` | show target configuration and invariants | 0 |
| `run --target T --seed N [--count N] [--no-suite] [--out DIR]` / `run --config FILE` | run a campaign, write artifacts | 0 no violations, 1 violations, 2 bad input, 3 setup error |
| `replay <scenario-file>` | re-run one scenario | 0 / 1 / 2 / 3 |
| `minimize <finding-file> [--out F] [--max-attempts N]` | shrink a finding's scenario | 0, 2, 4 not reproducible |
| `report <campaign-id> --format json\|text` | print a stored report | 0, 2 |
| `corpus list \| replay <id>\|--all \| add <scenario> --description ...` | regression corpus | 0, 2, 3, 4 mismatch |
| `tui [--snapshot SCREEN]` | interactive UI (or print one screen headlessly) | 0, 2, 3 |

Targets: `demo-escrow-vulnerable`, `demo-escrow-corrected`, or `demo-escrow` (both).

## Repository layout

`crates/tokenscope-{model,executor,adapter,invariants,scenarios,shrinker,core,report,cli,tui}`,
`programs/demo-escrow`, `fixtures/` (scenarios and the regression corpus), `scripts/`, `docs/`.
Architecture: [docs/architecture.md](docs/architecture.md).

## Limitations

* One target program (the demo escrow). Token-2022 coverage is `TransferFeeConfig` and
  freeze/thaw of token accounts. Transfer hooks, default account state, pausable, CPI Guard,
  delegates and permanent delegates are **not implemented**; no support is claimed.
* Authorization is modeled only as the depositor-signature check (`INV-AUTH-001`).
* Scenario generation is seeded pseudo-random, not coverage-guided.
* Token-2022 is the mainnet-snapshot ELF bundled by `mollusk-svm-programs-token-2022` 0.16.2; it
  may differ from other Token-2022 builds.
* Execution is instruction-level (no signature verification, fees or blockhash); see
  `docs/experiments/svm-framework.md`.
* An invariant violation shows a property failing on observed state, not that funds can be stolen.

## Reproducibility and patch delivery

The repository history is also delivered as an ordered `git format-patch` series outside the source
tree; see [CONTRIBUTING.md](CONTRIBUTING.md#patch-delivery) and `scripts/verify-reconstruction.sh`.

## License

Apache-2.0 (see `LICENSE`).
