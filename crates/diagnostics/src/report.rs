//! Ties Phase 15's three modules together into the one thing the phase
//! actually promises to deliver: "Debug tools" (Development_plan Phase
//! 15). A [`DiagnosticsReport`] is a point-in-time snapshot of
//! everything this workspace can currently say about its own health --
//! metrics, recent crashes, and whether telemetry is even turned on
//! (never its buffered *content*; see `telemetry`'s own privacy rules,
//! which this module has no reason to work around).

use crate::crash::list_crash_reports;
use common::{now, AppResult, Timestamp};
use metrics::{MetricsRegistry, MetricsSnapshot};
use serde::Serialize;
use std::path::Path;
use telemetry::TelemetryClient;

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticsReport {
    pub generated_at: Timestamp,
    pub metrics: MetricsSnapshot,
    pub crash_report_count: usize,
    pub telemetry_enabled: bool,
    pub telemetry_buffered_events: usize,
}

/// Builds a report from whatever this process currently has: `registry`
/// for the metrics snapshot, `telemetry` for its enabled-status and how
/// much (never *what*) is buffered, and `crash_dir` for how many crash
/// reports exist on disk. Any of these being freshly-constructed/empty
/// is a normal, valid report -- a brand-new session with a clean crash
/// directory isn't an error condition.
pub fn build_report(
    registry: &MetricsRegistry,
    telemetry: &TelemetryClient,
    crash_dir: &Path,
) -> AppResult<DiagnosticsReport> {
    Ok(DiagnosticsReport {
        generated_at: now(),
        metrics: registry.snapshot(),
        crash_report_count: list_crash_reports(crash_dir)?.len(),
        telemetry_enabled: telemetry.is_enabled(),
        telemetry_buffered_events: telemetry.buffered_count(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crash::write_crash_report;
    use telemetry::{TelemetryEvent, TelemetryEventKind};

    #[test]
    fn report_reflects_metrics_crashes_and_telemetry_together() {
        let dir = std::env::temp_dir().join("diagnostics-report-test-full");
        std::fs::remove_dir_all(&dir).ok();

        let registry = MetricsRegistry::new();
        registry.increment("sessions_started");

        let crash = crate::crash::CrashReport {
            id: common::Id::new(),
            timestamp: now(),
            message: "test crash".to_string(),
            location: None,
            backtrace: None,
        };
        write_crash_report(&dir, &crash).unwrap();

        let tele = TelemetryClient::new(true, None);
        tele.record(TelemetryEvent::new(TelemetryEventKind::SessionStarted));

        let report = build_report(&registry, &tele, &dir).unwrap();

        assert_eq!(report.metrics.counters["sessions_started"], 1);
        assert_eq!(report.crash_report_count, 1);
        assert!(report.telemetry_enabled);
        assert_eq!(report.telemetry_buffered_events, 1);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn report_on_a_fresh_process_is_valid_not_an_error() {
        let dir = std::env::temp_dir().join("diagnostics-report-test-fresh-xyz-never-created");
        std::fs::remove_dir_all(&dir).ok();

        let registry = MetricsRegistry::new();
        let tele = TelemetryClient::new(false, None);
        let report = build_report(&registry, &tele, &dir).unwrap();

        assert_eq!(report.crash_report_count, 0);
        assert!(!report.telemetry_enabled);
    }
}
