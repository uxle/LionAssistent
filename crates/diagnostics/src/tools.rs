//! Exposes [`crate::build_report`] as a real, `ReadOnly`
//! `ToolCategory::System` tool -- the closest existing category (TCLIS
//! SS4's "System" covers CPU/memory/uptime-style self-status; a
//! diagnostics report is that same kind of question one level up).
//! Read-only by construction: nothing this tool touches can be written
//! to through it.

use crate::report::build_report;
use metrics::MetricsRegistry;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use telemetry::TelemetryClient;
use tool_framework::{InputSchema, PermissionLevel, Tool, ToolArgs, ToolCategory, ToolOutcome};

pub struct GetDiagnosticsReportTool {
    pub registry: Arc<MetricsRegistry>,
    pub telemetry: Arc<TelemetryClient>,
    pub crash_dir: PathBuf,
}

impl Tool for GetDiagnosticsReportTool {
    fn name(&self) -> &str {
        "get_diagnostics_report"
    }
    fn description(&self) -> &str {
        "Reports current performance metrics, recent crash count, and telemetry status."
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::System
    }
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
    fn timeout(&self) -> Duration {
        Duration::from_secs(10)
    }
    fn input_schema(&self) -> InputSchema {
        InputSchema::new(vec![])
    }
    fn execute(&self, _args: &ToolArgs) -> ToolOutcome {
        match build_report(&self.registry, &self.telemetry, &self.crash_dir) {
            Ok(report) => ToolOutcome {
                success: true,
                message: format!(
                    "{} crash report(s) on file; telemetry {}",
                    report.crash_report_count,
                    if report.telemetry_enabled {
                        "enabled"
                    } else {
                        "disabled"
                    }
                ),
                data: serde_json::to_value(&report).ok(),
            },
            Err(e) => ToolOutcome {
                success: false,
                message: e.to_string(),
                data: None,
            },
        }
    }
}

/// Registers the tool, for `app` to call once at startup alongside every
/// other `register_all` -- not done in this phase, same reason as
/// always (docs/ROADMAP.md).
pub fn register_all(
    registry: &mut tool_framework::ToolRegistry,
    metrics_registry: Arc<MetricsRegistry>,
    telemetry: Arc<TelemetryClient>,
    crash_dir: PathBuf,
) -> common::AppResult<()> {
    registry.register(Arc::new(GetDiagnosticsReportTool {
        registry: metrics_registry,
        telemetry,
        crash_dir,
    }))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tool_framework::{ToolRegistry, ToolRouter};

    #[tokio::test]
    async fn the_tool_runs_and_reports_a_valid_snapshot() {
        let dir = std::env::temp_dir().join("diagnostics-tools-test-basic");
        std::fs::remove_dir_all(&dir).ok();

        let metrics_registry = Arc::new(MetricsRegistry::new());
        metrics_registry.increment("test_counter");
        let tele = Arc::new(TelemetryClient::new(false, None));

        let mut registry = ToolRegistry::new();
        register_all(&mut registry, metrics_registry, tele, dir.clone()).unwrap();
        let router = ToolRouter::new(registry, PermissionLevel::ReadOnly);

        let result = router
            .execute("get_diagnostics_report", ToolArgs::new())
            .await;
        assert!(result.success, "{}", result.message);
        assert_eq!(
            result.data.unwrap()["metrics"]["counters"]["test_counter"],
            1
        );
    }

    #[tokio::test]
    async fn the_tool_is_registered_under_its_expected_name() {
        let dir = std::env::temp_dir().join("diagnostics-tools-test-registered");
        let metrics_registry = Arc::new(MetricsRegistry::new());
        let tele = Arc::new(TelemetryClient::new(false, None));

        let mut registry = ToolRegistry::new();
        register_all(&mut registry, metrics_registry, tele, dir).unwrap();

        assert_eq!(registry.list(), vec!["get_diagnostics_report"]);
    }
}
