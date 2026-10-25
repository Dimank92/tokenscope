//! Typed data model shared by every TokenScope component.
//!
//! Nothing in this crate executes programs or touches the network; it only
//! defines serializable, validated data.

mod campaign;
mod error;
mod finding;
mod scenario;
mod trace;

pub use campaign::{Campaign, RunRecord, Totals};
pub use error::ModelError;
pub use finding::{Finding, OutcomeClass, Reproduction, Severity, StateDelta};
pub use scenario::{
    Action, MAX_ACTIONS, MAX_BASIS_POINTS, MAX_NAME_LEN, MintSpec, Scenario, TransferFeeParams,
};
pub use trace::{AccountRef, ExecutionTrace, Snapshot, StepRecord, StepStatus};

/// Version of the persisted JSON formats produced by this crate (see `FORMAT.md`).
pub const SCHEMA_VERSION: u32 = 1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_version_is_one() {
        assert_eq!(SCHEMA_VERSION, 1);
    }
}
