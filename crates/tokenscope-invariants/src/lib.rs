//! Invariant engine.
//!
//! Each invariant has an identifier, preconditions, an expected behavior, and documented
//! legitimate exceptions (see `docs/invariants.md`). Invariants are pure functions over an
//! [`ExecutionTrace`]; they never execute programs and never edit state.
//!
//! A verdict of [`Verdict::NotApplicable`] means the preconditions were not met, which is never
//! evidence of a bug. Setup problems never reach this crate: the adapter reports them separately.

use tokenscope_model::{
    Action, ExecutionTrace, Finding, OutcomeClass, Reproduction, SCHEMA_VERSION, Scenario,
    Severity, Snapshot, StateDelta, StepRecord, StepStatus,
};

/// Documentation of one invariant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvariantDef {
    /// Stable identifier, e.g. `INV-ACCT-001`.
    pub id: &'static str,
    /// Short name.
    pub name: &'static str,
    /// What the property states.
    pub description: &'static str,
    /// Conditions under which the invariant is evaluated.
    pub preconditions: &'static [&'static str],
    /// Required behavior.
    pub expected: &'static str,
    /// Cases deliberately not treated as violations.
    pub exceptions: &'static [&'static str],
}

/// Transfer accounting: credit equals actual receipt.
pub const ACCT_001: InvariantDef = InvariantDef {
    id: "INV-ACCT-001",
    name: "Deposit credit equals amount received",
    description: "After a successful deposit, the escrow's recorded balance must increase by exactly the amount the vault actually received.",
    preconditions: &[
        "the step is a deposit",
        "the step succeeded",
        "the vault is a Token-2022 account observed from real account data",
    ],
    expected: "delta(escrow_recorded) == delta(vault_amount)",
    exceptions: &[
        "none in P0: the demo escrow has no fee of its own and no other path credits the vault",
    ],
};

/// Solvency: liabilities never exceed assets.
pub const SOLV_001: InvariantDef = InvariantDef {
    id: "INV-SOLV-001",
    name: "Recorded liability does not exceed vault assets",
    description: "After every successful step, the amount the escrow owes the depositor must not exceed the vault's token balance.",
    preconditions: &["the step succeeded"],
    expected: "escrow_recorded <= vault_amount",
    exceptions: &["transient states inside a single instruction are not observed"],
};

/// Failure atomicity.
pub const ATOM_001: InvariantDef = InvariantDef {
    id: "INV-ATOM-001",
    name: "Failed instructions leave no partial state",
    description: "If an instruction fails, every observed quantity must equal its value before the instruction.",
    preconditions: &["the step failed"],
    expected: "after == before for all observed quantities",
    exceptions: &["lamport fees and compute use are not observed"],
};

/// Conservation under documented transfer-fee semantics.
pub const CONS_001: InvariantDef = InvariantDef {
    id: "INV-CONS-001",
    name: "Deposit conserves tokens under transfer-fee semantics",
    description: "On a successful deposit of `amount`, the depositor is debited exactly `amount`, and the vault's balance gain plus its withheld-fee gain equals `amount`.",
    preconditions: &["the step is a deposit", "the step succeeded"],
    expected: "depositor_debit == amount && delta(vault_amount) + delta(vault_withheld) == amount",
    exceptions: &["a missing transfer-fee extension counts as withheld = 0"],
};

/// All invariants implemented by this crate.
pub fn catalog() -> [InvariantDef; 4] {
    [ACCT_001, SOLV_001, ATOM_001, CONS_001]
}

/// Result of evaluating one invariant on one step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// The invariant held.
    Held,
    /// The invariant did not hold.
    Violated {
        /// Observed behavior description.
        observed: String,
    },
    /// Preconditions were not met; not evidence either way.
    NotApplicable,
}

/// One invariant evaluated on one step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evaluation {
    /// Invariant identifier.
    pub invariant_id: &'static str,
    /// Step index.
    pub step_index: usize,
    /// Outcome.
    pub verdict: Verdict,
}

fn d(after: u64, before: u64) -> i128 {
    i128::from(after) - i128::from(before)
}

fn acct_001(step: &StepRecord) -> Verdict {
    let (Action::Deposit { amount }, StepStatus::Ok) = (step.action, &step.status) else {
        return Verdict::NotApplicable;
    };
    let credit = d(step.after.escrow_recorded, step.before.escrow_recorded);
    let received = d(step.after.vault_amount, step.before.vault_amount);
    if credit == received {
        Verdict::Held
    } else {
        Verdict::Violated {
            observed: format!(
                "deposit of {amount} credited {credit} but the vault received {received} (withheld fee delta {})",
                d(
                    step.after.vault_withheld.unwrap_or(0),
                    step.before.vault_withheld.unwrap_or(0)
                )
            ),
        }
    }
}

fn solv_001(step: &StepRecord) -> Verdict {
    if step.status != StepStatus::Ok {
        return Verdict::NotApplicable;
    }
    let s = &step.after;
    if s.escrow_recorded <= s.vault_amount {
        Verdict::Held
    } else {
        Verdict::Violated {
            observed: format!(
                "escrow records {} owed but the vault holds {}: shortfall of {}",
                s.escrow_recorded,
                s.vault_amount,
                s.escrow_recorded - s.vault_amount
            ),
        }
    }
}

fn atom_001(step: &StepRecord) -> Verdict {
    if !matches!(step.status, StepStatus::Failed { .. }) {
        return Verdict::NotApplicable;
    }
    if step.before == step.after {
        Verdict::Held
    } else {
        Verdict::Violated {
            observed: format!(
                "failed step changed state: before={:?} after={:?}",
                step.before, step.after
            ),
        }
    }
}

fn cons_001(step: &StepRecord) -> Verdict {
    let (Action::Deposit { amount }, StepStatus::Ok) = (step.action, &step.status) else {
        return Verdict::NotApplicable;
    };
    let debit = d(step.before.depositor_balance, step.after.depositor_balance);
    let gain = d(step.after.vault_amount, step.before.vault_amount);
    let withheld = d(
        step.after.vault_withheld.unwrap_or(0),
        step.before.vault_withheld.unwrap_or(0),
    );
    if debit == i128::from(amount) && gain + withheld == i128::from(amount) {
        Verdict::Held
    } else {
        Verdict::Violated {
            observed: format!(
                "deposit of {amount}: depositor debited {debit}, vault gained {gain}, withheld gained {withheld}"
            ),
        }
    }
}

type Check = (&'static str, fn(&StepRecord) -> Verdict);

/// Evaluate every invariant on every step of `trace`, in step then catalog order.
pub fn evaluate(trace: &ExecutionTrace) -> Vec<Evaluation> {
    let checks: [Check; 4] = [
        (ACCT_001.id, acct_001),
        (SOLV_001.id, solv_001),
        (ATOM_001.id, atom_001),
        (CONS_001.id, cons_001),
    ];
    let mut out = Vec::new();
    for step in &trace.steps {
        for (id, check) in checks {
            out.push(Evaluation {
                invariant_id: id,
                step_index: step.index,
                verdict: check(step),
            });
        }
    }
    out
}

/// Run-level classification from evaluations and step statuses. `SetupError` is decided earlier
/// (the adapter returns an error) and never produced here.
pub fn classify(trace: &ExecutionTrace, evaluations: &[Evaluation]) -> OutcomeClass {
    if evaluations
        .iter()
        .any(|e| matches!(e.verdict, Verdict::Violated { .. }))
    {
        OutcomeClass::InvariantViolation
    } else if trace
        .steps
        .iter()
        .any(|s| matches!(s.status, StepStatus::Failed { .. }))
    {
        OutcomeClass::ExecutionFailure
    } else {
        OutcomeClass::Pass
    }
}

fn delta(label: &str, account: &str, before: u64, after: u64) -> StateDelta {
    StateDelta {
        label: label.into(),
        account: account.into(),
        before,
        after,
    }
}

fn evidence(trace: &ExecutionTrace, before: &Snapshot, after: &Snapshot) -> Vec<StateDelta> {
    let acct = |l: &str| trace.address_of(l).unwrap_or("unknown").to_string();
    let mut v = vec![
        delta(
            "escrow_recorded",
            &acct("escrow_state"),
            before.escrow_recorded,
            after.escrow_recorded,
        ),
        delta(
            "vault_amount",
            &acct("vault"),
            before.vault_amount,
            after.vault_amount,
        ),
        delta(
            "depositor_balance",
            &acct("depositor_source"),
            before.depositor_balance,
            after.depositor_balance,
        ),
    ];
    if before.vault_withheld.is_some() || after.vault_withheld.is_some() {
        v.push(delta(
            "vault_withheld",
            &acct("vault"),
            before.vault_withheld.unwrap_or(0),
            after.vault_withheld.unwrap_or(0),
        ));
    }
    v
}

fn severity_for(id: &str) -> (Severity, &'static str) {
    match id {
        "INV-SOLV-001" => (
            Severity::High,
            "Recorded liabilities exceed vault assets, so honoring the record can fail or drain other users' funds. Exploitability by an attacker is not established by this check.",
        ),
        "INV-ACCT-001" => (
            Severity::Medium,
            "The escrow over- or under-credits relative to tokens actually held. Impact depends on how credits are later redeemed; this check alone does not prove exploitability.",
        ),
        "INV-CONS-001" => (
            Severity::Medium,
            "Tokens are not conserved under the documented fee semantics, which suggests an accounting error in the observation or the program.",
        ),
        _ => (
            Severity::Low,
            "A failed instruction left partial state, which can be exploitable depending on the program; severity is advisory.",
        ),
    }
}

/// Build one [`Finding`] per violated invariant (using its first violating step) in `trace`.
/// Findings always have class [`OutcomeClass::InvariantViolation`]; promotion to a suspected
/// vulnerability is a human decision.
pub fn findings(
    scenario: &Scenario,
    trace: &ExecutionTrace,
    evaluations: &[Evaluation],
) -> Vec<Finding> {
    let mut seen = Vec::<&'static str>::new();
    let mut out = Vec::new();
    for e in evaluations {
        let Verdict::Violated { observed } = &e.verdict else {
            continue;
        };
        if seen.contains(&e.invariant_id) {
            continue;
        }
        seen.push(e.invariant_id);
        let step = &trace.steps[e.step_index];
        let def = catalog()
            .into_iter()
            .find(|d| d.id == e.invariant_id)
            .expect("known invariant");
        let (severity, rationale) = severity_for(e.invariant_id);
        let scenario_id = scenario.id();
        out.push(Finding {
            schema_version: SCHEMA_VERSION,
            id: Finding::compute_id(&scenario.target, &scenario_id, e.invariant_id),
            target: scenario.target.clone(),
            scenario_id: scenario_id.clone(),
            seed: scenario.seed,
            invariant_id: e.invariant_id.into(),
            class: OutcomeClass::InvariantViolation,
            expected: format!("{} ({})", def.expected, def.name),
            observed: format!("step {}: {observed}", e.step_index),
            deltas: evidence(trace, &step.before, &step.after),
            logs: step.logs.clone(),
            severity,
            severity_rationale: rationale.into(),
            reproduction: Reproduction {
                command: format!("tokenscope replay {scenario_id}.json"),
                scenario: scenario.clone(),
            },
            minimized: None,
        });
    }
    out
}

#[cfg(test)]
mod tests;
