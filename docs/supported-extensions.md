# Supported Token-2022 behavior

"Supported" means exercised by an executable test against the real Token-2022 program (the
mainnet-snapshot ELF bundled by `mollusk-svm-programs-token-2022` 0.16.2, sha256
`b2a7ce1e...612da4`; run `tokenscope doctor` for the full hash).

## TransferFeeConfig - supported (limited)

Verified by `crates/tokenscope-executor/tests/transfer_fee.rs` and the adapter/core tests:

| Behavior | Evidence |
|---|---|
| 2% fee: transfer of 1,000 credits 980 net and withholds 20 on the destination | decoded account state |
| Fee rounds up (`ceil`): 1 unit at 1 bp delivers 0, withholds 1 | test `fee_rounds_up_and_can_consume_entire_tiny_transfer` |
| `maximum_fee` caps the fee | test `maximum_fee_caps_the_fee` |
| Zero-amount transfer has zero fee | test `zero_amount_transfer_has_zero_fee` |
| 100% fee (10,000 bp) consumes the transfer | built-in scenario `fee-100pct-consumes-deposit` |
| Mint without the extension delivers the full amount | test `mint_without_fee_extension_delivers_full_amount` |
| Insufficient funds is an instruction failure, not a setup error | test `insufficient_funds_is_an_instruction_failure_not_a_setup_error` |

Fee formula (from `spl-token-2022-interface` 3.1.2 source): `min(ceil(amount * bps / 10_000), maximum_fee)`.
It is used only as an independent cross-check in tests; invariants use decoded state.

### Not covered for TransferFeeConfig

* Fee **changes** (`SetTransferFee` takes effect two epochs later per the docs) - epochs are not advanced.
* `TransferCheckedWithFee`, `WithdrawWithheldTokens*`, `HarvestWithheldTokensToMint`.
* Fee-config authority handling.

## Freeze / thaw of token accounts - supported (limited)

The harness creates every mint with the harness payer as **freeze authority**. The actions
`FreezeDepositor` / `ThawDepositor` run real `FreezeAccount` / `ThawAccount` instructions; the
frozen flag is decoded from account data. Verified by
`crates/tokenscope-executor/tests/transfer_fee.rs` (a transfer from a frozen account fails with the
Token-2022 "frozen" log and persists nothing; it succeeds after thaw) and
`crates/tokenscope-adapter/tests/escrow.rs` (escrow deposits fail atomically while the source is
frozen and work after the thaw). **Not covered:** the `DefaultAccountState` extension (accounts
frozen at creation), freezing the vault or destination, freeze-authority changes.

## Not supported (no claim made)

Transfer hooks, default account state, pausable, CPI Guard, permanent delegate,
delegates and approvals, interest-bearing, non-transferable, confidential transfers, metadata,
group/member pointers, mint close authority. Their behavior has not been tested here. Extension
incompatibility checks are likewise not implemented.

## Version caveat

Behavior is that of the bundled ELF. Mainnet or other builds may differ; re-run the suite against
any other build before drawing conclusions.
