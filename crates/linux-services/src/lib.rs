//! `linux-services` -- safe wrappers around Linux system capabilities
//! (Development_plan Phase 8; Archd Layer 5 "Linux Services"; TCLIS SS4
//! tool categories).
//!
//! TCLIS SS1 is explicit: the assistant never executes unrestricted shell
//! commands. Every function here does one specific, narrow thing -- list
//! a directory, read `/proc/meminfo`, run a command from a fixed
//! allowlist with no shell interpretation -- rather than accepting and
//! interpreting an arbitrary command string.
//!
//! ## What actually works in this sandbox vs. a real desktop
//!
//! This crate was built and tested in a headless container: no X11/
//! Wayland, no PulseAudio/PipeWire, no D-Bus session bus, no backlight
//! device, no battery. That's a real Linux environment, just not a
//! *desktop* one, and it splits this phase's modules into two honest
//! groups:
//!
//! - **Fully verified for real:** `files`, `system`, `processes`,
//!   `terminal`, `packages`, `network` -- these use the filesystem,
//!   `/proc`, and safe subprocess calls, all of which are genuinely
//!   present in any Linux environment including this one. Tests exercise
//!   real success paths.
//! - **Verified for graceful failure, not full success:** `clipboard`,
//!   `notifications`, `audio`, `display`, `applications` -- these need a
//!   desktop session this sandbox doesn't have. Tests confirm they
//!   return a clear, structured error rather than panicking or hanging;
//!   on a real desktop with the right services running, the same code
//!   paths succeed instead.
//! - **Never executed, even in tests:** `power` (lock/suspend/restart/
//!   shutdown). Implemented for real, but automatically running a
//!   shutdown command as part of a test suite is simply not a defensible
//!   thing to do. Tests cover command *construction* only.

pub mod applications;
pub mod audio;
pub mod clipboard;
pub mod display;
pub mod files;
pub mod network;
pub mod notifications;
pub mod packages;
pub mod power;
pub mod processes;
pub mod system;
pub mod terminal;
pub mod tools;

use common::AppError;

/// Wraps a `std::io::Error` with which module and operation it came from
/// (Apis SS16 Error API: every error should carry `module`/`message`).
pub(crate) fn io_err(
    module: &'static str,
    context: impl std::fmt::Display,
    e: std::io::Error,
) -> AppError {
    AppError::Module {
        module,
        message: format!("{context}: {e}"),
    }
}
