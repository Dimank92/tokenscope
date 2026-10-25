//! Structured errors for untrusted or malformed input.

use thiserror::Error;

/// Errors produced while validating or decoding model data.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ModelError {
    /// A scenario failed validation.
    #[error("invalid scenario: {0}")]
    InvalidScenario(String),
    /// A persisted document used a schema version this build does not support.
    #[error("unsupported schema version {found}; this build supports {supported}")]
    UnsupportedSchema {
        /// Version found in the document.
        found: u32,
        /// Version supported by this build.
        supported: u32,
    },
    /// JSON could not be parsed into the expected type.
    #[error("malformed document: {0}")]
    Malformed(String),
}
