# Invariants

Each invariant is a pure function over one step of an `ExecutionTrace`
(`crates/tokenscope-invariants`). A verdict is **held**, **violated** or **not applicable**
(preconditions unmet; never evidence of a bug). Setup errors never reach this stage.

Severity attached to findings is advisory and always carries a rationale; no finding is promoted
to `suspected_vulnerability` automatically.

## INV-ACCT-001: deposit credit equals amount received

* **Precondition:** the step is a deposit, it succeeded, and the vault is a Token-2022 account read
  from real account data.
* **Expected:** `delta(escrow_recorded) == delta(vault_amount)`.
* **Legitimate exceptions:** none in P0 (the demo escrow charges no fee of its own).
* **Detects:** crediting the nominal amount when a transfer fee reduces what arrives.
* **Does not prove:** how the over-credit can be exploited.

## INV-SOLV-001: recorded liability does not exceed vault assets

* **Precondition:** the step succeeded.
* **Expected:** `escrow_recorded <= vault_amount` after the step.
* **Exceptions:** transient states inside one instruction are not observed.
* **Observation:** on the vulnerable escrow a later withdrawal of the over-credited balance fails
  (locked funds), shown by `fee-2pct-deposit-withdraw`.

## INV-ATOM-001: failed instructions leave no partial state

* **Precondition:** the step failed.
* **Expected:** every observed quantity equals its value before the step.
* **Exceptions:** lamport fees and compute use are not observed.
* **Note:** Mollusk does not persist failed instructions, so this holds structurally for the demo
  escrow; the invariant exists to catch regressions and future multi-instruction targets, and is
  unit-tested against synthetic partial-state traces.

## INV-CONS-001: deposit conserves tokens under transfer-fee semantics

* **Precondition:** the step is a deposit and succeeded.
* **Expected:** `depositor_debit == amount && delta(vault_amount) + delta(vault_withheld) == amount`.
* **Exceptions:** a missing transfer-fee extension counts as `withheld = 0`.
* **Expected result:** holds for *both* escrow flavors, because the fee is accounted for; it
  guards the harness's own observations.

## Not implemented

* **Authorization** (unauthorized actors cause no prohibited transition): the adapter has no
  attacker action yet.
* Invariants for transfer hooks, frozen accounts, pausable, CPI Guard, delegates.

## Adding an invariant

State id, preconditions, expected behavior and exceptions in `InvariantDef`; implement a
`fn(&StepRecord) -> Verdict`; add held/violated/not-applicable unit tests with synthetic traces;
add it to `catalog()` and `evaluate`; document it here.
