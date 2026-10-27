//! Evidence-bearing results: outcome classes, state deltas and findings.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{ModelError, SCHEMA_VERSION, Scenario};

/// Classification of one scenario run against one target. These classes are deliberately
/// distinct: only [`OutcomeClass::InvariantViolation`] and above indicate a target problem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeClass {
    /// All applicable invariants held.
    Pass,
    /// The harness could not establish the preconditions (bad mint, missing program, ...).
    SetupError,
    /// The program or runtime rejected an instruction; no invariant was evaluated as violated.
    ExecutionFailure,
    /// An explicit invariant did not hold on observed state.
    InvariantViolation,
    /// A violation a human reviewer judged potentially exploitable (never assigned automatically).
    SuspectedVulnerability,
}

/// Severity rationale bucket. Severity is advisory and always accompanied by a rationale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Informational.
    Info,
    /// Low.
    Low,
    /// Medium.
    Medium,
    /// High.
    High,
}

/// Before/after observation of one account quantity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateDelta {
    /// Role label, e.g. `vault_token_amount`.
    pub label: String,
    /// Account address (base58) or a program-defined field name.
    pub account: String,
    /// Value before the observed step.
    pub before: u64,
    /// Value after the observed step.
    pub after: u64,
}

/// How to reproduce a finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reproduction {
    /// Exact CLI command.
    pub command: String,
    /// Scenario that triggers the finding.
    pub scenario: Scenario,
}

/// A recorded invariant violation with the evidence needed to judge it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    /// Persisted-format version.
    pub schema_version: u32,
    /// Content-derived identifier, see [`Finding::compute_id`].
    pub id: String,
    /// Target identifier.
    pub target: String,
    /// Identifier of the scenario that produced it.
    pub scenario_id: String,
    /// Scenario seed.
    pub seed: u64,
    /// Identifier of the violated invariant, e.g. `INV-ACCT-001`.
    pub invariant_id: String,
    /// Outcome class (never `Pass`).
    pub class: OutcomeClass,
    /// What the invariant required.
    pub expected: String,
    /// What was observed.
    pub observed: String,
    /// Relevant state deltas.
    pub deltas: Vec<StateDelta>,
    /// Program/runtime logs captured during execution.
    pub logs: Vec<String>,
    /// Advisory severity.
    pub severity: Severity,
    /// Why that severity was chosen.
    pub severity_rationale: String,
    /// Reproduction details.
    pub reproduction: Reproduction,
    /// Minimized scenario, when shrinking has run.
    pub minimized: Option<Scenario>,
}

impl Finding {
    /// Stable identifier: `fd_` + 16 hex chars of SHA-256 over `target|scenario_id|invariant_id`.
    pub fn compute_id(target: &str, scenario_id: &str, invariant_id: &str) -> String {
        let digest = Sha256::digest(format!("{target}|{scenario_id}|{invariant_id}"));
        let hex: String = digest.iter().take(8).map(|b| format!("{b:02x}")).collect();
        format!("fd_{hex}")
    }

    /// Check internal consistency.
    pub fn validate(&self) -> Result<(), ModelError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(ModelError::UnsupportedSchema {
                found: self.schema_version,
                supported: SCHEMA_VERSION,
            });
        }
        if self.class == OutcomeClass::Pass {
            return Err(ModelError::InvalidScenario(
                "a finding cannot have class pass".into(),
            ));
        }
        if self.id != Self::compute_id(&self.target, &self.scenario_id, &self.invariant_id) {
            return Err(ModelError::Malformed(
                "finding id does not match content".into(),
            ));
        }
        Ok(())
    }

    /// Parse and validate a finding from JSON.
    pub fn from_json(input: &str) -> Result<Self, ModelError> {
        let f: Finding =
            serde_json::from_str(input).map_err(|e| ModelError::Malformed(e.to_string()))?;
        f.validate()?;
        Ok(f)
    }

    /// Serialize to pretty JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("finding serialization is infallible")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Action, MintSpec};

    fn finding() -> Finding {
        let scenario = Scenario {
            schema_version: SCHEMA_VERSION,
            name: "n".into(),
            seed: 1,
            target: "t".into(),
            mint: MintSpec {
                decimals: 0,
                transfer_fee: None,
            },
            initial_balance: 5,
            extensions: vec![],
            actions: vec![Action::Deposit { amount: 5 }],
        };
        let sid = scenario.id();
        Finding {
            schema_version: SCHEMA_VERSION,
            id: Finding::compute_id("t", &sid, "INV-ACCT-001"),
            target: "t".into(),
            scenario_id: sid,
            seed: 1,
            invariant_id: "INV-ACCT-001".into(),
            class: OutcomeClass::InvariantViolation,
            expected: "e".into(),
            observed: "o".into(),
            deltas: vec![StateDelta {
                label: "vault".into(),
                account: "a".into(),
                before: 0,
                after: 4,
            }],
            logs: vec![],
            severity: Severity::Medium,
            severity_rationale: "r".into(),
            reproduction: Reproduction {
                command: "c".into(),
                scenario,
            },
            minimized: None,
        }
    }

    #[test]
    fn round_trip_and_validate() {
        let f = finding();
        assert_eq!(Finding::from_json(&f.to_json()).unwrap(), f);
    }

    #[test]
    fn pass_class_is_rejected() {
        let mut f = finding();
        f.class = OutcomeClass::Pass;
        assert!(f.validate().is_err());
    }

    #[test]
    fn tampered_id_is_rejected() {
        let mut f = finding();
        f.id = "fd_0000000000000000".into();
        assert!(matches!(f.validate(), Err(ModelError::Malformed(_))));
    }

    #[test]
    fn severity_orders() {
        assert!(Severity::High > Severity::Low && Severity::Info < Severity::Medium);
    }

    #[test]
    fn classes_serialize_snake_case() {
        let s = serde_json::to_string(&OutcomeClass::InvariantViolation).unwrap();
        assert_eq!(s, "\"invariant_violation\"");
    }
}
