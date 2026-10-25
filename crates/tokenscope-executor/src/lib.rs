//! Real SVM execution harness.
//!
//! Every state change in this crate is produced by executing real programs (the system program
//! and the Token-2022 program ELF bundled with `mollusk-svm-programs-token-2022`) inside the
//! Mollusk SVM. Balances are never edited by hand; observations decode actual account data.

mod error;
mod harness;

pub use error::ExecError;
pub use harness::{Harness, TokenAccountView, derive_address};
