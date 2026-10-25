# Experiment: Mollusk vs. LiteSVM for real Token-2022 execution

Time-boxed spike (one compile cycle each). Same scenario in both: create a Token-2022 mint with
`TransferFeeConfig` (200 bps, unbounded max fee, 6 decimals), two token accounts, `MintTo` 10,000,
`TransferChecked` 1,000. Token-2022 program: the mainnet-dump ELF shipped in
`mollusk-svm-programs-token-2022` 0.16.2 (loaded explicitly into LiteSVM).

| Criterion | Mollusk 0.16.2 | LiteSVM 0.18.1 |
|---|---|---|
| Result of the scenario | destination 980, withheld 20 (asserted in `tests/transfer_fee.rs`) | destination 980, withheld 20 (spike binary output) |
| Token-2022 program | bundled by `mollusk-svm-programs-token-2022` (`extensions` feature) | must be supplied by the caller |
| Execution model | instruction-level; no signatures, blockhash or tx fees | full transactions: signatures, blockhash, tx fees |
| State persistence | `MolluskContext`: state persists only when an instruction succeeds | `LiteSVM` store, transaction atomicity |
| Determinism hooks | no randomness in setup; addresses derived from labels | keypairs must be seeded explicitly (`Keypair::new_from_array`) |
| Clean debug build (1 CPU) | 3 min 07 s, `target` 657 MB | 3 min 21 s, `target` 642 MB |
| MSRV | not stated on crates.io | 1.97.1 |

## Decision

Use **Mollusk** for P0/P1. It needs less ceremony (no signing or blockhash plumbing), ships the
Token-2022 ELF with a feature-gated builder crate, reports `ProgramResult` explicitly, and its
context only persists successful instructions, which matches the failure-atomicity invariant we
want to test. LiteSVM remains a viable alternative for later work that needs full transaction
semantics (fees, signature checks); the executor API is kept small so a second backend could be added.

## Caveats

* The bundled Token-2022 ELF is a mainnet snapshot, not necessarily equal to the
  `spl-token-2022-interface` 3.1.2 source. TokenScope treats executed account state as ground
  truth and uses the interface crate only for instruction/state encoding.
* Mollusk 0.16.2 enables `solana_logger` by default, so test logs are noisy; use `rtk cargo test`
  or `RUST_LOG=off`.
* The LiteSVM spike code was not kept in the repository.
