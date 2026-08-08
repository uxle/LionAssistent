//! `metrics` -- an in-process metrics registry plus a performance
//! monitor built on top of it (Development_plan Phase 15 "Metrics" /
//! "Performance Monitor" modules).
//!
//! [`MetricsRegistry`] holds counters, gauges, and timers, all
//! thread-safe, none of it ever leaving the process on its own --
//! sending anything anywhere is `telemetry`'s job, deliberately kept
//! separate and opt-in. [`PerformanceMonitor`] samples real system state
//! (via Phase 8's `linux_services::system`) into a registry and keeps a
//! bounded rolling history.

pub mod monitor;
pub mod registry;

pub use monitor::{run_periodic_sampling, PerformanceMonitor, SystemSample};
pub use registry::{MetricsRegistry, MetricsSnapshot, TimerSummary};
