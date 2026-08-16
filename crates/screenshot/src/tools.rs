//! Wires screen capture up as a real [`tool_framework::Tool`], following
//! `linux-services::tools`' exact shape (Phase 8 established the pattern;
//! this crate is the first *new* crate since to add its own tools rather
//! than extending that one).

use crate::capture_full_screen;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tool_framework::{
    InputSchema, ParamKind, ParamSchema, PermissionLevel, Tool, ToolArgs, ToolCategory, ToolOutcome,
};

fn err_outcome(e: impl std::fmt::Display) -> ToolOutcome {
    ToolOutcome {
        success: false,
        message: e.to_string(),
        data: None,
    }
}

fn default_output_dir() -> PathBuf {
    // DBDMS SS2's storage structure doesn't list a screenshots directory
    // -- `exports/` is specifically for user-triggered chat/memory/
    // settings/workflow exports (DBDMS SS12), not captured images, so
    // reusing it would misrepresent what's in there. This is a Phase 23
    // addition to the storage layout, the same way Phase 16 added
    // encryption/secret-storage tables that SS2's original tree didn't
    // itemize either -- see docs/ROADMAP.md.
    PathBuf::from("data/screenshots")
}

pub struct CaptureScreenshotTool;

impl Tool for CaptureScreenshotTool {
    fn name(&self) -> &str {
        "capture_screenshot"
    }
    fn description(&self) -> &str {
        "Captures the full screen to a PNG file and reports its path and dimensions."
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Vision
    }
    /// `Moderate`, not `ReadOnly` like `read_file` -- deliberately
    /// inconsistent-looking with that precedent, so the reasoning is
    /// worth spelling out. `read_file` requires the caller to already
    /// know and name a specific path; whatever's sensitive in it is
    /// something the caller pointed at on purpose. A screenshot exposes
    /// *whatever is currently on screen* -- across every window, from
    /// whichever app the user happened to have open -- without the
    /// caller naming anything at all. That's a meaningfully different
    /// risk shape, closer to "the tool decides what data to expose" than
    /// "the user named a specific known file", and TCLIS SS6's own
    /// levels don't have a category that fits either precedent
    /// perfectly. `Moderate` (confirmation required, per
    /// `permission-manager`'s default `auto_approve_at_or_below: Safe`)
    /// is this project's judgment call, not a spec-mandated value --
    /// worth revisiting if it turns out to be too strict or too loose in
    /// practice.
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Moderate
    }
    fn timeout(&self) -> Duration {
        Duration::from_secs(15)
    }
    fn input_schema(&self) -> InputSchema {
        InputSchema::new(vec![ParamSchema::optional("output_dir", ParamKind::String)])
    }
    fn execute(&self, args: &ToolArgs) -> ToolOutcome {
        let output_dir = args
            .get("output_dir")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
            .unwrap_or_else(default_output_dir);

        match capture_full_screen(Path::new(&output_dir)) {
            Ok(info) => ToolOutcome {
                success: true,
                message: format!(
                    "captured {}x{} screenshot to {}",
                    info.width,
                    info.height,
                    info.path.display()
                ),
                data: Some(serde_json::json!({
                    "path": info.path.to_string_lossy(),
                    "width": info.width,
                    "height": info.height,
                    "captured_at_unix_ms": info.captured_at_unix_ms as u64,
                })),
            },
            Err(e) => err_outcome(e),
        }
    }
}

pub fn register_all(registry: &mut tool_framework::ToolRegistry) -> common::AppResult<()> {
    use std::sync::Arc;
    registry.register(Arc::new(CaptureScreenshotTool))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tool_framework::{ToolRegistry, ToolRouter};

    fn router() -> ToolRouter {
        let mut registry = ToolRegistry::new();
        register_all(&mut registry).unwrap();
        ToolRouter::new(registry, PermissionLevel::Administrator)
    }

    #[test]
    fn all_tools_are_registered() {
        let mut registry = ToolRegistry::new();
        register_all(&mut registry).unwrap();
        assert_eq!(registry.list(), vec!["capture_screenshot"]);
    }

    #[tokio::test]
    async fn missing_display_fails_through_the_full_pipeline_cleanly() {
        // Deliberately does not touch DISPLAY/WAYLAND_DISPLAY itself
        // (mutating process env vars in a test that runs alongside
        // others invites exactly the kind of race `detect_session_from`
        // was split out to avoid -- see lib.rs). If this happens to run
        // where a display *is* configured, it genuinely captures instead,
        // which is also a legitimate, successful outcome for this tool.
        let router = router();
        let mut args = ToolArgs::new();
        args.insert(
            "output_dir".into(),
            serde_json::json!(std::env::temp_dir()
                .join("screenshot-tool-test")
                .to_string_lossy()),
        );
        let result = router.execute("capture_screenshot", args).await;
        // Either genuinely succeeds (a display is reachable) or fails
        // with a clean, structured message (it isn't) -- what matters is
        // there's no panic and the router's pipeline completed.
        assert!(result.status != tool_framework::ResultStatus::ToolUnavailable);
        assert!(result.status != tool_framework::ResultStatus::ValidationFailed);
    }
}
