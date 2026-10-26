# Demo: vulnerable vs. corrected escrow

All output below was produced by the commands shown on this repository's code (debug build).
Campaign and scenario IDs are content hashes; they stay stable until the scenarios or targets change.

## 1. Build and check

```sh
./scripts/build-programs.sh           # builds target/sbf-programs/demo_escrow_{vulnerable,corrected}.so from source
cargo build -p tokenscope-cli && export PATH="$PWD/target/debug:$PATH"
tokenscope doctor                     # ends with "doctor: environment OK"
```

`doctor` runs a real 1,000-unit `TransferChecked` on a 200 bp fee mint and checks the decoded
balances: "delivered 980 with 20 withheld".

## 2. The bug

```sh
tokenscope run --target demo-escrow-vulnerable --count 0 --out demo-out   # exit code 1
```

```text
executed 14 | pass 4 | execution failures 2 | invariant violations 8 | setup errors 0 | findings 16
  pass                   control-no-fee
  invariant-violation    fee-2pct-deposit
  invariant-violation    fee-2pct-deposit-withdraw
  ...
  pass                   zero-amount-deposit                (zero fee, nothing to lose)
  execution-failure      failure-insufficient-funds         (rejected instruction, not a finding)
  execution-failure      frozen-source-deposit-rejected     (real Token-2022 rejects a frozen source)
  invariant-violation    freeze-thaw-then-deposit           (bug exposed again after the thaw)
  pass                   unauthorized-withdraw-rejected     (the rejected attack is the desired behavior)
```

The `fee-2pct-deposit` finding (evidence is decoded from real account data):

```text
fd_eb4facd442285199 INV-ACCT-001 [Medium] target=demo-escrow-vulnerable scenario=sc_3573a0f4384e7945 seed=0
  expected: delta(escrow_recorded) == delta(vault_amount) (Deposit credit equals amount received)
  observed: step 0: deposit of 1000 credited 1000 but the vault received 980 (withheld fee delta 20)
  evidence:
    escrow_recorded 0 -> 1000
    vault_amount 0 -> 980
    depositor_balance 10000 -> 9000
    vault_withheld 0 -> 20
```

`fee-2pct-deposit-withdraw` additionally shows the consequence: withdrawing the credited 1,000
from a vault holding 980 fails (INV-SOLV-001), i.e. funds are locked.

## 3. The fix

```sh
tokenscope run --target demo-escrow-corrected --count 0 --out demo-out    # exit code 0
```

```text
executed 14 | pass 12 | execution failures 2 | invariant violations 0 | setup errors 0 | findings 0
```

The same scenarios pass. The two execution failures are the intentional insufficient-funds and
frozen-source cases.
The corrected escrow's withdrawal is itself a fee-bearing transfer: 980 leaves the vault and 960
arrives (asserted in `crates/tokenscope-adapter/tests/escrow.rs`).

## 4. Replay and minimize

Artifacts are written to `demo-out/<campaign-id>/{campaign.json,report.json,report.txt,findings/,scenarios/}`.

```sh
tokenscope replay demo-out/<campaign-id>/scenarios/<scenario-id>.json        # exit 1 on the vulnerable target
tokenscope minimize demo-out/<campaign-id>/findings/<finding-id>.json --out min.json
```

For the `two-deposits-accumulate` finding:

```text
minimized finding fd_39a80af5323c8225 (INV-ACCT-001)
  original: 2 action(s)  ->  minimized: 1 action(s)
  executions: 20 (6 accepted)
  ... one deposit of 1 unit, mint with a 1 bp fee and maximum fee 1, initial balance 1, decimals 0
```

One unit at 1 bp rounds the fee **up** to 1, so nothing arrives, yet the vulnerable escrow still
credits 1. Every accepted reduction was validated by re-executing on the real SVM.

## 5. Regression corpus

`fixtures/corpus/` pins this minimized scenario on both targets:

```sh
tokenscope corpus list
tokenscope corpus replay --all      # exit 0; exit 4 if any case no longer matches its expectation
```

The test `crates/tokenscope-core/tests/regression_corpus.rs` additionally checks that the committed
scenario is exactly what minimization derives from the original noisy scenario.

## 6. TUI

```sh
tokenscope tui                          # interactive; press ? for help, t toggles vulnerable/corrected
tokenscope tui --snapshot findings      # headless: prints one screen as text
```

## What this does not show

It does not demonstrate exploitation. The findings establish that the escrow's recorded balance
diverges from its real token balance under a documented fee mechanism.
