//! `diagnostics` -- crash reports plus an aggregated report over
//! `metrics` and `telemetry` (Development_plan Phase 15's "Crash
//! Reports" module and overall "Debug tools" deliverable).
//!
//! [`crash::install_panic_hook`] captures every panic, on every thread,
//! as a small local JSON file -- see that module's doc comment for why
//! it's deliberately simple (no database, minimal state in the hook
//! closure) and how it was tested without needing to mutate the
//! process's global panic hook from within most tests.
//!
//! [`report::build_report`] ties this crate's crash history together
//! with `metrics`' current snapshot and `telemetry`'s enabled-status
//! (never its buffered content) into one [`report::DiagnosticsReport`],
//! exposed as a real, `ReadOnly` `get_diagnostics_report` tool via
//! [`tools`].

pub mod crash;
pub mod report;
pub mod tools;

pub use crash::{install_panic_hook, list_crash_reports, write_crash_report, CrashReport};
pub use report::{build_report, DiagnosticsReport};
