//! `telemetry` -- strictly opt-in, coarse-grained usage telemetry
//! (Development_plan Phase 15 "Telemetry (Optional)" module).
//!
//! Read [`event`]'s doc comment before using anything here: what a
//! [`TelemetryEvent`] is allowed to contain is deliberately restrictive
//! (a closed enum of event kinds, one short optional label), and
//! [`TelemetryClient`] is gated twice over -- `enabled` and `endpoint`
//! must both be explicitly set for a single byte to ever leave the
//! device. Nothing in this crate defaults to on.

pub mod client;
pub mod event;

pub use client::TelemetryClient;
pub use event::{TelemetryEvent, TelemetryEventKind};
