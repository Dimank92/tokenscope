# Supported Token-2022 behavior

"Supported" means exercised by an executable test against the real Token-2022 program: the
mainnet-snapshot ELF bundled by `mollusk-svm-programs-token-2022` 0.16.2 (sha256 `b2a7ce1e...612da4`;
`tokenscope doctor` prints the full hash). Expected values in tests were **observed** on that
runtime, then compared with the Solana documentation; where they differ the runtime is asserted
and the difference is listed here. Error codes are Token-2022 `TokenError` values unless noted.

| Extension | Status | Where it is tested |
|---|---|---|
| TransferFeeConfig | supported (limited) | `executor/tests/transfer_fee.rs`, `adapter/tests/escrow.rs`, `core/tests/acceptance.rs` |
| Freeze / thaw of token accounts | supported (limited) | `executor/tests/transfer_fee.rs`, `adapter/tests/escrow.rs` |
| DefaultAccountState | supported (limited) | `executor/tests/ext_default_state.rs`, `adapter/tests/extensions.rs` |
| Pausable | supported (limited) | `executor/tests/ext_pausable.rs`, `adapter/tests/extensions.rs` |
| TransferHook | supported (limited, fixture hook) | `executor/tests/ext_transfer_hook.rs`, `adapter/tests/extensions.rs` |
| CPI Guard | supported (limited) | `executor/tests/ext_cpi_guard.rs`, `adapter/tests/extensions.rs` |
| Delegates (approve / revoke) | supported | `executor/tests/ext_delegates.rs`, `adapter/tests/extensions.rs` |
| PermanentDelegate | supported | `executor/tests/ext_delegates.rs`, `adapter/tests/extensions.rs`, `core/tests/extension_suite.rs` |
| Extension compatibility / init order | supported (limited) | `executor/tests/ext_compat.rs` |

Everything else is **not supported**: no claim is made (see the end).

## TransferFeeConfig

Fee formula (from `spl-token-2022-interface` 3.1.2 source): `min(ceil(amount * bps / 10_000), maximum_fee)`,
used only as an independent cross-check in tests; invariants use decoded account state.

* 2% fee: a 1,000 transfer credits 980 and withholds 20 on the destination.
* Rounds up: 1 unit at 1 bp delivers 0 and withholds 1. `maximum_fee` caps the fee. A zero amount has
  a zero fee. 100% fee (10,000 bp) consumes the transfer. A mint without the extension delivers
  everything. u64-boundary amounts do not overflow (`u64::MAX` at 1%).
* Insufficient funds is an instruction failure, not a setup error.
* Combines with Pausable (a paused transfer leaves withheld fees untouched), TransferHook (the hook
  sees the *nominal* amount) and PermanentDelegate (its transfers still pay the fee).
* Not covered: fee changes (`SetTransferFee` takes effect two epochs later; epochs are not advanced),
  `TransferCheckedWithFee`, `WithdrawWithheldTokens*`, `HarvestWithheldTokensToMint`, fee-authority handling.

## Freeze and thaw

Every mint is created with the harness payer as **freeze authority**. `FreezeDepositor` /
`ThawDepositor` run real `FreezeAccount` / `ThawAccount`; the frozen flag is decoded from account data.
A transfer from or to a frozen account fails with `AccountFrozen` (17), logged as "frozen", and persists
nothing; it succeeds after the thaw.

## DefaultAccountState

* A mint initialized with `Frozen` creates frozen token accounts; `MintTo` into one fails (17) and
  changes neither balance nor supply.
* Only the freeze authority can thaw; the account owner gets `OwnerMismatch` (4).
* A transfer needs **both** accounts thawed; the failed attempt leaves state untouched.
* `UpdateDefaultAccountState` changes the default for accounts created afterwards only.
* `Frozen` without a freeze authority fails `InitializeMint2` with `MintCannotFreeze` (16).
* In the escrow, the **vault** starts frozen (setup thaws every other account), so deposits fail (17)
  until `ThawVault`.
* Not covered: freezing the vault or destination by other means, freeze-authority changes.

## Pausable

* `Pause` blocks transfer, mint and burn with `MintPaused` (67) ("paused" log); no partial state change.
  `Resume` restores them. The flag is decoded from the mint.
* Only the pause authority may pause or resume: others get `OwnerMismatch` (4) and the flag is unchanged.
* Observed: pausing twice succeeds; `Approve` and `FreezeAccount` are **not** gated by the pause.
* Not covered: pause-authority changes.

## TransferHook

Tested with a fixture hook (`programs/demo-hook`) that implements the `spl-transfer-hook-interface`
`Execute` layout with a counter, a reject flag and a `transferring`-flag check. Its validation
account (`ExtraAccountMetaList`) is written in the documented TLV layout with one fixed extra account.
The docs state that a hook is not optional at the account level; observed on the runtime:

* A transfer carrying the hook program, the validation PDA and the extra account invokes the hook
  (counter and last amount updated); repeated transfers accumulate.
* A plain transfer without those accounts fails (`NotEnoughAccountKeys`); nothing moves.
* Missing counter, or a same-shaped decoy counter, fails with the account-resolution error `0xa261c2c0`
  (the runtime logs "Error: Unknown"); a missing validation account or hook program fails with
  `NotEnoughAccountKeys`. All leave balances and the counter unchanged.
* Hook rejection (`Custom(100)`) fails the transfer atomically; it works again once the hook accepts.
* A direct `Execute` call outside a transfer is rejected by the hook (`Custom(101)`) because the token
  program sets `TransferHookAccount.transferring` only during its own CPI.
* A hook mint whose hook program is not loaded fails the transfer.
* Pause is enforced **before** the hook runs; minting does not invoke the hook.
* The demo escrow does not forward extra accounts, so deposits into a hook mint fail
  (`NotEnoughAccountKeys`, hook never ran): an integration incompatibility, shown as an execution failure.
* Not covered: PDA-derived or other-program extra accounts, `UpdateExtraAccountMetaList`, changing the
  hook program or authority. The fixture's `SetReject` instruction is unauthenticated: test-only.

## CPI Guard

* Enabling needs the `CpiGuard` account extension (the harness reallocates); only the owner can
  enable or disable (others get `OwnerMismatch`). The flag is decoded from the account.
* Direct (top-level) operations are unaffected: an owner-signed transfer and an approve still work.
* **CPI path** (through the escrow, which CPIs `TransferChecked` with the owner as authority): with the
  guard on, the deposit fails with `CpiGuardTransferBlocked` (42), logged "CPI Guard is enabled, and a
  program attempted to transfer user funds without using a delegate", and changes nothing; it works after the
  guard is disabled (a top-level instruction).
* Not covered: the other CPI restrictions (burn, approve, close, set-authority, delegate-authorized transfers).

## Delegates and the permanent delegate

* Approve sets delegate and allowance; a delegate spends it down (`delegated_amount` and balance both
  fall); exceeding it fails (`InsufficientFunds`, 1); strangers and delegates cannot approve or spend
  (`OwnerMismatch`); revoke clears both. Owner transfers do not consume the allowance.
* **PermanentDelegate:** the delegate can transfer and burn from *any* account of the mint with no approval, and
  nothing is visible on the account (`delegate` stays empty). Without the extension the same actor fails
  (`OwnerMismatch`). Its transfers pay transfer fees.
* Implication for escrows: a permanent delegate can drain the vault without the escrow's PDA. The
  `permanent-delegate-drains-vault` scenario flags this (`INV-CUST-001`, `INV-SOLV-001`) on **both**
  escrow flavors, because neither inspects the mint's extensions. It is an integration-assumption
  failure caused by the mint's configuration, not necessarily a program bug.

## Extension compatibility and configuration failures

* Valid combinations create and report exactly their extensions, for example fee+pausable,
  fee+default-state, hook+permanent-delegate, pausable+close-authority+interest-bearing,
  scaled-UI+fee, and a five-extension mint.
* `ScaledUiAmount` + `InterestBearingConfig` is rejected by the runtime (`InvalidExtensionCombination`, 51);
  the failed `InitializeMint2` leaves the mint uninitialized. Each alone is accepted.
* **Docs discrepancy:** the Solana docs use `NonTransferable` + `TransferFeeConfig` as an example of
  incompatible extensions. The interface crate's check (`extension/mod.rs`) only rejects confidential-transfer
  combinations and ScaledUiAmount+InterestBearing, and the runtime **accepts** this pair at creation;
  transfers are still blocked (`NonTransferable`, 37). Tests assert the runtime behavior.
* Initialization order: initializing the mint before an extension whose space was reserved fails
  (`InvalidAccountData`); the correct order succeeds. Re-running an extension initializer before
  `InitializeMint` is accepted (observed). An undersized mint account cannot hold an extension
  (`InvalidAccountData`).
* Required account extensions: a token account without the space the mint requires fails
  `InitializeAccount3` (`InvalidAccountData`); the harness derives the needed extensions from the mint
  (fee, hook, pausable, non-transferable). **Observed:** `NonTransferable` mints also force `ImmutableOwner`.
* `MintCloseAuthority`: an empty mint closes; a mint with supply does not.

## Escrow correctness and authorization (adapter tests, both flavors)

Substituting the mint (7), vault (4), token program (6) or vault authority (4), re-initializing the
escrow (1) and withdrawing as a non-depositor (5) are rejected with `EscrowError` codes and no state
change; repeated withdrawals are zero-amount no-ops; deposits and withdrawals at `u64::MAX` do not overflow.

## Not supported (no claim is made)

Confidential transfers and their fees, memo-required transfers, interest accrual and scaled-UI-amount
behavior beyond mint creation, metadata / group / pointer extensions, `TransferCheckedWithFee`, fee changes
across epochs, withheld-fee harvesting and withdrawal, multisig authorities, token-account close semantics
beyond what is listed, and any extension interaction not named above.

## Version caveat

Behavior is that of the bundled ELF. Mainnet or other builds may differ; re-run the suites on any
other build before drawing conclusions.
