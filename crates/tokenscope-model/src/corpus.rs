//! Regression corpus cases: scenarios with the outcome they must keep producing.

use serde::{Deserialize, Serialize};

use crate::{ModelError, OutcomeClass, RunRecord, SCHEMA_VERSION, Scenario};

/// The observable result a corpus case pins down.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Expectation {
    /// Required run classification.
    pub outcome: OutcomeClass,
    /// Sorted identifiers of invariants that must be violated (empty for non-violations).
    pub violated: Vec<String>,
}

impl Expectation {
    /// Summarize an actual run.
    pub fn from_run(run: &RunRecord) -> Self {
        let mut violated: Vec<String> = run
            .findings
            .iter()
            .map(|f| f.invariant_id.clone())
            .collect();
        violated.sort();
        violated.dedup();
        Self {
            outcome: run.outcome,
            violated,
        }
    }
}

/// A persisted regression case.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorpusCase {
    /// Persisted-format version.
    pub schema_version: u32,
    /// Case identifier (`rc_` + the scenario's content hash).
    pub id: String,
    /// Why the case exists.
    pub description: String,
    /// The scenario to replay (it names its own target).
    pub scenario: Scenario,
    /// What replay must produce.
    pub expect: Expectation,
}

impl CorpusCase {
    /// Create a case; the ID is derived from the scenario.
    pub fn new(description: impl Into<String>, scenario: Scenario, expect: Expectation) -> Self {
        let id = Self::id_for(&scenario);
        Self {
            schema_version: SCHEMA_VERSION,
            id,
            description: description.into(),
            scenario,
            expect,
        }
    }

    /// Case ID for a scenario.
    pub fn id_for(scenario: &Scenario) -> String {
        format!("rc_{}", &scenario.id()[3..])
    }

    /// Validate version, scenario and ID consistency.
    pub fn validate(&self) -> Result<(), ModelError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(ModelError::UnsupportedSchema {
                found: self.schema_version,
                supported: SCHEMA_VERSION,
            });
        }
        self.scenario.validate()?;
        if self.id != Self::id_for(&self.scenario) {
            return Err(ModelError::Malformed(
                "corpus case id does not match its scenario".into(),
            ));
        }
        Ok(())
    }

    /// Parse and validate from JSON.
    pub fn from_json(input: &str) -> Result<Self, ModelError> {
        let c: CorpusCase =
            serde_json::from_str(input).map_err(|e| ModelError::Malformed(e.to_string()))?;
        c.validate()?;
        Ok(c)
    }

    /// Serialize to pretty JSON.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("corpus case serialization is infallible")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Action, MintSpec};

    fn case() -> CorpusCase {
        let scenario = Scenario {
            schema_version: SCHEMA_VERSION,
            name: "n".into(),
            seed: 0,
            target: "t".into(),
            mint: MintSpec {
                decimals: 0,
                transfer_fee: None,
            },
            initial_balance: 1,
            extensions: vec![],
            actions: vec![Action::Deposit { amount: 1 }],
        };
        CorpusCase::new(
            "d",
            scenario,
            Expectation {
                outcome: OutcomeClass::Pass,
                violated: vec![],
            },
        )
    }

    #[test]
    fn round_trip_and_id_check() {
        let c = case();
        assert_eq!(CorpusCase::from_json(&c.to_json()).unwrap(), c);
        assert!(c.id.starts_with("rc_"));
        let mut t = c.clone();
        t.id = "rc_0000000000000000".into();
        assert!(matches!(t.validate(), Err(ModelError::Malformed(_))));
        let mut v = c;
        v.schema_version = 7;
        assert!(matches!(
            v.validate(),
            Err(ModelError::UnsupportedSchema { found: 7, .. })
        ));
    }

    #[test]
    fn expectation_from_run_sorts_and_dedups() {
        let c = case();
        let run = RunRecord {
            scenario_id: c.scenario.id(),
            scenario: c.scenario,
            outcome: OutcomeClass::Pass,
            setup_error: None,
            trace: None,
            findings: vec![],
        };
        assert_eq!(
            Expectation::from_run(&run),
            Expectation {
                outcome: OutcomeClass::Pass,
                violated: vec![]
            }
        );
    }
}
