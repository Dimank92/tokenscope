//! Target adapters.
//!
//! A program ID alone does not define behavior: an adapter says how to deploy the program, set up
//! its accounts, build its instructions, and observe its state. The only adapter so far targets the
//! demo escrow in `programs/demo-escrow`.

mod escrow;

pub use escrow::{EscrowAdapter, EscrowFlavor, default_program_dir};
use tokenscope_executor::ExecError;
use tokenscope_model::{ExecutionTrace, Scenario};

/// Common contract for program target adapters.
///
/// An adapter encapsulates program-specific deployment, account provisioning,
/// instruction encoding, and state observation over real Solana VM execution.
pub trait TargetAdapter {
    /// Target identifier matching [`Scenario::target`].
    fn target_id(&self) -> &str;

    /// Execute the scenario and produce an execution trace with before/after snapshots.
    fn run(&self, scenario: &Scenario) -> Result<ExecutionTrace, ExecError>;
}

impl TargetAdapter for EscrowAdapter {
    fn target_id(&self) -> &str {
        self.flavor().target_id()
    }

    fn run(&self, scenario: &Scenario) -> Result<ExecutionTrace, ExecError> {
        self.run(scenario)
    }
}
