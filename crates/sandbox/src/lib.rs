//! `sandbox` -- process resource limits and a filesystem access sandbox
//! (Development_plan Phase 16 "Sandbox" module).
//!
//! [`ProcessLimits`]/[`apply_limits`] (rlimit-based) are empirically
//! confirmed to work in this sandbox's own build/test environment.
//! [`FilesystemSandbox`] (Landlock-based) is correct, real code that
//! compiled, ran, and reported honestly that it was **not** actually
//! enforced here -- read [`filesystem`]'s doc comment before relying on
//! it anywhere without checking [`EnforcementStatus`] yourself.

pub mod filesystem;
pub mod process_limits;

pub use filesystem::{EnforcementStatus, FilesystemSandbox};
pub use process_limits::{apply_limits, current_limits, ProcessLimits};
