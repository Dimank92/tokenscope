//! Typed data model shared by every TokenScope component.
//!
//! Nothing in this crate executes programs or touches the network; it only
//! defines serializable, validated data.

mod campaign;
mod corpus;
mod error;
mod finding;
mod raw_state;
mod reference;
mod scenario;
mod trace;

pub use campaign::{Campaign, RunRecord, Totals};
pub use corpus::{CorpusCase, Expectation};
pub use error::ModelError;
pub use finding::{Finding, OutcomeClass, Reproduction, Severity, StateDelta};
pub use raw_state::{RawAccount, RawSnapshot};
pub use reference::{ReferenceError, TokenReferenceModel};
pub use scenario::{
    Action, MAX_ACTIONS, MAX_BASIS_POINTS, MAX_EXTENSIONS, MAX_NAME_LEN, MintExtension, MintSpec,
    Mismatch, Scenario, TransferFeeParams,
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
