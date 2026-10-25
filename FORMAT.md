# Persisted formats

All TokenScope documents are UTF-8 JSON with a top-level `schema_version` (currently **1**).
Readers must reject unknown versions with a structured error (`ModelError::UnsupportedSchema`).

## Scenario (`schema_version` 1)

| Field | Type | Notes |
|---|---|---|
| `schema_version` | u32 | must be 1 |
| `name` | string | 1..=128 bytes |
| `seed` | u64 | seed that generated it; 0 if hand-written |
| `target` | string | adapter id, e.g. `demo-escrow-vulnerable` |
| `mint.decimals` | u8 | |
| `mint.transfer_fee` | null or `{basis_points: u16 (<=10000), maximum_fee: u64}` | |
| `initial_balance` | u64 | minted to the depositor before the first action |
| `actions` | array (1..=64) | `{"kind":"deposit","amount":u64}` or `{"kind":"withdraw"}` |

Scenario ID = `sc_` + first 8 bytes (hex) of SHA-256 over the compact JSON serialization.

## Finding (`schema_version` 1)

Fields: `id`, `target`, `scenario_id`, `seed`, `invariant_id`, `class`, `expected`, `observed`,
`deltas[]` (`label`, `account`, `before`, `after`), `logs[]`, `severity`, `severity_rationale`,
`reproduction` (`command`, `scenario`), `minimized` (scenario or null).

* `class`: `setup_error | execution_failure | invariant_violation | suspected_vulnerability`
  (`pass` is never persisted as a finding).
* `severity`: `info | low | medium | high`.
* Finding ID = `fd_` + first 8 bytes (hex) of SHA-256 over `target|scenario_id|invariant_id`.
  Loaders recompute and reject mismatches.

Later milestones extend this file with campaign, report and corpus formats.
