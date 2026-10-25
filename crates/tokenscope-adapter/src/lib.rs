//! Target adapters.
//!
//! A program ID alone does not define behavior: an adapter says how to deploy the program, set up
//! its accounts, build its instructions, and observe its state. The only adapter so far targets the
//! demo escrow in `programs/demo-escrow`.

mod escrow;

pub use escrow::{EscrowAdapter, EscrowFlavor, default_program_dir};
