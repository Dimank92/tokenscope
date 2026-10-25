//! Executor errors. `Setup` errors mean the harness could not establish preconditions and must
//! never be reported as target vulnerabilities.

use thiserror::Error;

/// Errors from the execution harness.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ExecError {
    /// A harness setup step failed (mint creation, funding, account creation, ...).
    #[error("setup step `{step}` failed: {detail}")]
    Setup {
        /// Name of the failing step.
        step: String,
        /// Failure detail.
        detail: String,
    },
    /// An account expected to exist was not found in the store.
    #[error("account {0} not found")]
    MissingAccount(String),
    /// Account data could not be decoded as the expected Token-2022 state.
    #[error("cannot decode account {account}: {detail}")]
    Decode {
        /// Account address.
        account: String,
        /// Decode failure detail.
        detail: String,
    },
    /// The instruction was rejected by the program or runtime.
    #[error("instruction failed: {0}")]
    Instruction(String),
}
