//! Campaign-level results: many scenario runs against one target.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{ExecutionTrace, Finding, ModelError, OutcomeClass, SCHEMA_VERSION, Scenario};

/// Result of running one scenario.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunRecord {
    /// The scenario that was run.
    pub scenario: Scenario,
    /// Its identifier.
    pub scenario_id: String,
    /// Run classification.
    pub outcome: OutcomeClass,
    /// Setup failure detail (only for `setup_error`).
    pub setup_error: Option<String>,
    /// Observed execution (absent for setup errors).
    pub trace: Option<ExecutionTrace>,
    /// Findings produced by invariant violations.
    pub findings: Vec<Finding>,
}

/// Aggregate counts for a campaign.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Totals {
    /// Scenarios run.
    pub executed: usize,
    /// Runs where every invariant held and every step succeeded.
    pub passed: usize,
    /// Runs with rejected instructions but no violated invariant.
    pub execution_failures: usize,
    /// Runs with at least one violated invariant.
    pub invariant_violations: usize,
    /// Runs the harness could not set up.
    pub setup_errors: usize,
    /// Total findings.
    pub findings: usize,
}

/// A set of runs against one target. Contains no wall-clock data so that identical inputs
/// produce identical campaigns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Campaign {
    /// Persisted-format version.
    pub schema_version: u32,
    /// Content-derived identifier (`cp_` + 16 hex).
    pub id: String,
    /// Target identifier.
    pub target: String,
    /// Seed used to derive generated scenarios (0 for the built-in suite).
    pub seed: u64,
    /// Runs in execution order.
    pub runs: Vec<RunRecord>,
}

impl Campaign {
    /// Build a campaign from runs, deriving its ID from target, seed and scenario IDs.
    pub fn new(target: String, seed: u64, runs: Vec<RunRecord>) -> Self {
        let mut h = Sha256::new();
        h.update(format!("{target}|{seed}"));
        for r in &runs {
            h.update(format!("|{}", r.scenario_id));
        }
        let hex: String = h
            .finalize()
            .iter()
            .take(8)
            .map(|b| format!("{b:02x}"))
            .collect();
        Self {
            schema_version: SCHEMA_VERSION,
            id: format!("cp_{hex}"),
            target,
            seed,
            runs,
        }
    }

    /// Aggregate counts.
    pub fn totals(&self) -> Totals {
        let mut t = Totals {
            executed: self.runs.len(),
            ..Totals::default()
        };
        for r in &self.runs {
            match r.outcome {
                OutcomeClass::Pass => t.passed += 1,
                OutcomeClass::ExecutionFailure => t.execution_failures += 1,
                OutcomeClass::InvariantViolation | OutcomeClass::SuspectedVulnerability => {
                    t.invariant_violations += 1
                }
                OutcomeClass::SetupError => t.setup_errors += 1,
            }
            t.findings += r.findings.len();
        }
        t
    }

    /// All findings in run order.
    pub fn findings(&self) -> impl Iterator<Item = &Finding> {
        self.runs.iter().flat_map(|r| r.findings.iter())
    }

    /// Parse a campaign from JSON, rejecting unknown schema versions.
    pub fn from_json(input: &str) -> Result<Self, ModelError> {
        let c: Campaign =
            serde_json::from_str(input).map_err(|e| ModelError::Malformed(e.to_string()))?;
        if c.schema_version != SCHEMA_VERSION {
            return Err(ModelError::UnsupportedSchema {
                found: c.schema_version,
                supported: SCHEMA_VERSION,
            });
        }
        Ok(c)
    }

    /// Serialize to pretty JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("campaign serialization is infallible")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Action, MintSpec};

    fn run(outcome: OutcomeClass, n: u64) -> RunRecord {
        let scenario = Scenario {
            schema_version: SCHEMA_VERSION,
            name: "n".into(),
            seed: 0,
            target: "t".into(),
            mint: MintSpec {
                decimals: 0,
                transfer_fee: None,
            },
            initial_balance: n,
            extensions: vec![],
            actions: vec![Action::Withdraw],
        };
        RunRecord {
            scenario_id: scenario.id(),
            scenario,
            outcome,
            setup_error: None,
            trace: None,
            findings: vec![],
        }
    }

    #[test]
    fn totals_count_each_class() {
        let c = Campaign::new(
            "t".into(),
            0,
            vec![
                run(OutcomeClass::Pass, 1),
                run(OutcomeClass::ExecutionFailure, 2),
                run(OutcomeClass::InvariantViolation, 3),
                run(OutcomeClass::SetupError, 4),
            ],
        );
        let t = c.totals();
        assert_eq!(
            (
                t.executed,
                t.passed,
                t.execution_failures,
                t.invariant_violations,
                t.setup_errors
            ),
            (4, 1, 1, 1, 1)
        );
    }

    #[test]
    fn id_is_deterministic_and_depends_on_content() {
        let a = Campaign::new("t".into(), 1, vec![run(OutcomeClass::Pass, 1)]);
        assert_eq!(
            a.id,
            Campaign::new("t".into(), 1, vec![run(OutcomeClass::Pass, 1)]).id
        );
        assert_ne!(
            a.id,
            Campaign::new("t".into(), 2, vec![run(OutcomeClass::Pass, 1)]).id
        );
        assert_ne!(
            a.id,
            Campaign::new("t".into(), 1, vec![run(OutcomeClass::Pass, 9)]).id
        );
    }

    #[test]
    fn json_round_trip_and_version_check() {
        let c = Campaign::new("t".into(), 1, vec![run(OutcomeClass::Pass, 1)]);
        assert_eq!(Campaign::from_json(&c.to_json()).unwrap(), c);
        let bad = c
            .to_json()
            .replace("\"schema_version\": 1", "\"schema_version\": 9");
        assert!(matches!(
            Campaign::from_json(&bad),
            Err(ModelError::UnsupportedSchema { found: 9, .. })
        ));
        assert!(matches!(
            Campaign::from_json("nope"),
            Err(ModelError::Malformed(_))
        ));
    }
}
