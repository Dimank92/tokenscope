//! Interactive terminal UI over real engine state.
//!
//! Structure: [`app`] is pure state and key handling, [`runner`] executes campaigns on a worker
//! thread, [`ui`] renders, and [`driver`] connects them to a terminal. The CLI never depends on
//! this crate for its own commands except `tokenscope tui`.

pub mod app;
pub mod driver;
pub mod runner;
pub mod ui;

pub use app::{App, Command, Options, Screen};
pub use driver::{render_to_string, run_interactive, snapshot};
