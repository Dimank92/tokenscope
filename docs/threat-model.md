# Threat model

TokenScope is a *testing tool*; this document covers (a) the class of bugs it targets, (b) how its
own results can be wrong, and (c) the security posture of the tool.

## Target bug class

Integrations that assume Token-2022 behaves like SPL Token. Implemented, demonstrated and tested:

* **Nominal-vs-received accounting** under `TransferFeeConfig` (INV-ACCT-001, INV-SOLV-001, INV-CONS-001),
  including epoch-advancement fee changes, `TransferCheckedWithFee`, harvest and withdrawal operations.
* **Custody assumptions** broken by mint extensions: a `PermanentDelegate` can drain the vault (INV-CUST-001).
* **Execution-time rejections** an integration must survive atomically: frozen accounts and
  `DefaultAccountState`, `Pausable`, **CPI Guard** (tested via real CPI forwarder blocking owner-signed
  CPI transfers, burns, approves, close-account, and set-authority while allowing delegate CPI transfers),
  and **transfer hooks** the integration does not support (extra accounts are mandatory).
* **Authorization and identity**: non-depositor withdrawals, substituted mint/vault/token program/authority,
  re-initialization (INV-AUTH-001), and multisig authority threshold verification (including runtime acceptance of $m > n$).
* **Transaction atomicity and memo requirements**: required transfer memos with empty memo validation,
  strict instruction ordering, and complete atomic rollback on transaction failure.
* **Pointer extensions**: `MetadataPointer`, `GroupPointer`, `GroupMemberPointer` lifecycle, authorized
  updates, and immutable pointer enforcement (`NoAuthorityExists`).
* **Extension configuration**: valid and invalid combinations, initialization order, required account
  extensions (observed against the runtime, including one place it differs from the docs).

Failure atomicity is checked (INV-ATOM-001) but holds structurally on the current target. Not tested:
confidential transfers and their fees, full TokenMetadata/TokenGroup account structures (pointers are tested),
and interest accrual / scaled UI amounts beyond creation.

## Assumptions

* The bundled Token-2022 ELF is representative of the behavior under test.
* The adapter correctly encodes the target's instructions and decodes its state. A wrong adapter
  produces wrong findings; the four-way outcome taxonomy limits, but does not eliminate, this.
* Instruction-level execution is sufficient: no signature verification, transaction fees,
  blockhash or compute-budget limits beyond what Mollusk enforces per instruction.

## Where results can be wrong

| Risk | Mitigation |
|---|---|
| Harness failure reported as a finding (false positive) | setup problems are `SetupError` and produce no findings; tested |
| Invariant too strict for a legitimate behavior | each invariant documents preconditions and exceptions (`docs/invariants.md`); verdicts can be *not applicable* |
| Shrinker alters the failure | every candidate is re-executed against the same predicate; the result is re-checked; tested with real execution |
| Non-reproducible findings | scenarios are self-contained and content-addressed; runs are deterministic (tested byte-for-byte) |
| Over-claiming | findings are `invariant_violation`; severity is advisory with rationale; `suspected_vulnerability` is never assigned automatically |
| Missed bugs | generation is seeded pseudo-random or dimension-guided (input space), never program-coverage-guided; absence of findings is not a security guarantee |
| Harness or parser panics on hostile files | document-size and nesting limits, property tests, libFuzzer targets (one real bug found and fixed); see `docs/dependency-review.md` |
| A guided or mutated scenario reaches states the harness mishandles | 240 extension-aware generated runs on both flavors produced no setup errors; every generated scenario is validated |

## Exclusions

TokenScope does not test: arbitrary third-party programs without an adapter, network/RPC behavior,
validator or runtime bugs, economic attacks, access-control logic beyond the demo's depositor
check, or upgradeability.

## Tool security posture

* Inputs (scenario/finding/config/corpus JSON) are untrusted: size-bounded (`MAX_ACTIONS`, name
  length, generated count limit), schema-versioned, validated, and rejected with structured errors;
  parsers are property-tested against arbitrary text.
* Campaign IDs are validated (`cp_` + 16 hex) before being used as path components.
* Host crates forbid `unsafe`. The on-chain demo uses `deny` (the pinocchio entrypoint macro expands
  to unsafe code).
* The CLI makes no network requests. Program ELFs are read from a local directory.
* The TUI restores the terminal on quit, error, panic, SIGTERM and SIGHUP (verified on a pty);
  SIGKILL cannot be handled.
* Test-only code paths are feature-gated: `fault-injection` (TUI) is absent from default builds, and the
  fixture hook's unauthenticated `SetReject` instruction exists only in `programs/demo-hook`.
* `cargo audit` finds one advisory reachable only through Agave's secp256k1 syscall path; accepted
  with justification in `docs/dependency-review.md`.
* The demo escrow is **intentionally vulnerable** in one flavor and must never be deployed.
* Generated `*-keypair.json` files from `cargo build-sbf` are build artifacts and are never committed
  (the build script copies only the `.so`).
