//! Orchestration of the TokenScope pipeline:
//! `scenario -> real SVM execution -> invariant checking -> findings`.

use std::path::PathBuf;

use tokenscope_adapter::{EscrowAdapter, EscrowFlavor};
use tokenscope_invariants::{classify, evaluate, findings};
use tokenscope_model::{Campaign, OutcomeClass, RunRecord, Scenario};

/// Target identifiers this build can run.
pub const TARGETS: [&str; 2] = ["demo-escrow-vulnerable", "demo-escrow-corrected"];

/// Runs scenarios and campaigns.
#[derive(Debug, Clone, Default)]
pub struct Engine {
    program_dir: Option<PathBuf>,
}

impl Engine {
    /// Engine that locates built programs automatically.
    pub fn new() -> Self {
        Self::default()
    }

    /// Engine that reads built programs from `dir`.
    pub fn with_program_dir(dir: impl Into<PathBuf>) -> Self {
        Self {
            program_dir: Some(dir.into()),
        }
    }

    fn adapter(&self, flavor: EscrowFlavor) -> EscrowAdapter {
        match &self.program_dir {
            Some(d) => EscrowAdapter::with_program_dir(flavor, d.clone()),
            None => EscrowAdapter::new(flavor),
        }
    }

    /// Run one scenario. Setup problems (unknown target, missing program, invalid scenario) are
    /// classified as [`OutcomeClass::SetupError`] and never produce findings.
    pub fn run_scenario(&self, scenario: &Scenario) -> RunRecord {
        let scenario_id = scenario.id();
        let setup_error = |detail: String| RunRecord {
            scenario: scenario.clone(),
            scenario_id: scenario_id.clone(),
            outcome: OutcomeClass::SetupError,
            setup_error: Some(detail),
            trace: None,
            findings: vec![],
        };
        let Some(flavor) = EscrowFlavor::from_target_id(&scenario.target) else {
            return setup_error(format!(
                "unknown target `{}` (known: {})",
                scenario.target,
                TARGETS.join(", ")
            ));
        };
        match self.adapter(flavor).run(scenario) {
            Err(e) => setup_error(e.to_string()),
            Ok(trace) => {
                let evaluations = evaluate(&trace);
                RunRecord {
                    scenario: scenario.clone(),
                    scenario_id,
                    outcome: classify(&trace, &evaluations),
                    setup_error: None,
                    findings: findings(scenario, &trace, &evaluations),
                    trace: Some(trace),
                }
            }
        }
    }

    /// Run every scenario in order and bundle the results.
    pub fn run_campaign(&self, target: &str, seed: u64, scenarios: &[Scenario]) -> Campaign {
        let runs = scenarios.iter().map(|s| self.run_scenario(s)).collect();
        Campaign::new(target.to_string(), seed, runs)
    }
}
