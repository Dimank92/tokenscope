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

## Execution trace (`schema_version` 1, embedded in campaigns)

`scenario_id`, `target`, `initial` and each step's `before`/`after` are *snapshots*:
`{vault_amount, vault_withheld (u64|null), escrow_recorded, depositor_balance, destination_balance}`.
Steps: `{index, action, before, after, status, logs[], compute_units}` where `status` is
`{"status":"ok"}` or `{"status":"failed","error":string}`. `accounts[]` is `{label, address}`.

## Campaign (`campaign.json`, `schema_version` 1)

`{schema_version, id, target, seed, runs[]}`; each run is
`{scenario, scenario_id, outcome, setup_error, trace, findings[]}`. `outcome` is one of
`pass | setup_error | execution_failure | invariant_violation | suspected_vulnerability`.
Campaign ID = `cp_` + first 8 bytes (hex) of SHA-256 over `target|seed` followed by `|<scenario_id>`
for each run. Campaigns contain no timestamps, so identical inputs give identical files.

## Report (`report.json`, `schema_version` 1)

`{schema_version, tokenscope_version, campaign_id, target, seed, totals, runs[], findings[]}`;
`totals` = `{executed, passed, execution_failures, invariant_violations, setup_errors, findings}`;
`runs[]` = `{scenario_id, name, outcome, setup_error, finding_ids[]}`. Findings are the full
`Finding` documents above. `report.txt` is the human-readable rendering of the same data.

## Corpus case (`fixtures/corpus/<id>.json`, `schema_version` 1)

`{schema_version, id, description, scenario, expect}` with `expect = {outcome, violated[]}`
(sorted invariant IDs). ID = `rc_` + the scenario's content hash. Loaders recompute and reject
mismatches. `tokenscope corpus replay` compares the actual run to `expect`.

## Run configuration (`tokenscope run --config`)

```json
{"schema_version": 1, "target": "demo-escrow", "seed": 7, "generated": 10, "builtin_suite": true}
```

`seed` (default 0), `generated` (default 0, max 10,000) and `builtin_suite` (default true) are
optional; unknown keys are rejected.

## Artifact directory layout

`<out>/<campaign-id>/campaign.json`, `report.json`, `report.txt`, `findings/<finding-id>.json`
(input for `minimize`), `scenarios/<scenario-id>.json` (input for `replay`).

## Compatibility policy

Any change to a persisted format that older readers cannot parse bumps `schema_version`; readers
reject versions they do not know.
