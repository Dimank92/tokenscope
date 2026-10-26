# Threat model

TokenScope is a *testing tool*; this document covers (a) the class of bugs it targets, (b) how its
own results can be wrong, and (c) the security posture of the tool.

## Target bug class

Integrations that assume Token-2022 behaves like SPL Token. Implemented and demonstrated:

* **Nominal-vs-received accounting** under `TransferFeeConfig` (INV-ACCT-001, INV-SOLV-001,
  INV-CONS-001).

* **Frozen source accounts** (freeze/thaw by the freeze authority): deposits must fail atomically.
* **Unauthorized actors**: a withdrawal signed by a non-depositor must be rejected (INV-AUTH-001);
  only the depositor-signature path is modeled.

Listed in the project brief but **not yet implemented**: transfer-hook account requirements,
`DefaultAccountState`, pausable behavior, CPI Guard restrictions, delegate and permanent-delegate
assumptions, extension incompatibilities. Failure atomicity is checked (INV-ATOM-001) but holds
structurally on the current target.

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
| Missed bugs | seeded generation is not coverage-guided; absence of findings is not a security guarantee |

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
* The demo escrow is **intentionally vulnerable** in one flavor and must never be deployed.
* Generated `*-keypair.json` files from `cargo build-sbf` are build artifacts and are never committed
  (the build script copies only the `.so`).
