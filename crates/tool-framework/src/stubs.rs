//! Stub tools proving the pipeline actually works -- registration,
//! validation, permission checks, and genuine timeout enforcement --
//! without touching the real filesystem or OS. Not real Linux tools;
//! those are Phase 8's job (TCLIS SS4 categories). The same pattern as
//! `EchoResponseGenerator` (Phase 3) and `KeywordClassifier` (Phase 6):
//! honest stand-ins that prove the plumbing, not fake capability.

use crate::{
    InputSchema, ParamKind, ParamSchema, PermissionLevel, Tool, ToolArgs, ToolCategory, ToolOutcome,
};
use std::time::Duration;

/// Repeats back the given text. The happy-path proof that registration,
/// validation, and execution all work correctly together.
pub struct EchoTool;

impl Tool for EchoTool {
    fn name(&self) -> &str {
        "echo"
    }

    fn description(&self) -> &str {
        "Repeats back the given text. A stub proving the tool pipeline works; replaced by real Linux tools in Phase 8."
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::System
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(5)
    }

    fn input_schema(&self) -> InputSchema {
        InputSchema::new(vec![ParamSchema::required("text", ParamKind::String)])
    }

    fn execute(&self, args: &ToolArgs) -> ToolOutcome {
        let text = args
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        ToolOutcome {
            success: true,
            message: format!("echo: {text}"),
            data: Some(serde_json::json!({ "text": text })),
        }
    }
}

/// Deliberately takes far longer than its own declared timeout, to prove
/// `ToolRouter::execute` actually enforces timeouts rather than trusting
/// a tool to police itself.
pub struct SlowTool;

impl Tool for SlowTool {
    fn name(&self) -> &str {
        "slow-stub"
    }

    fn description(&self) -> &str {
        "Sleeps far longer than its declared timeout, to test that timeouts are actually enforced."
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::System
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    fn timeout(&self) -> Duration {
        Duration::from_millis(50)
    }

    fn input_schema(&self) -> InputSchema {
        InputSchema::default()
    }

    fn execute(&self, _args: &ToolArgs) -> ToolOutcome {
        std::thread::sleep(Duration::from_millis(500));
        ToolOutcome {
            success: true,
            message: "finished -- if you see this in a test, the timeout did NOT fire".to_string(),
            data: None,
        }
    }
}

/// Requires Administrator-level permission, to test that the router
/// correctly denies tools above the configured maximum rather than
/// running everything registered.
pub struct DangerousStubTool;

impl Tool for DangerousStubTool {
    fn name(&self) -> &str {
        "dangerous-stub"
    }

    fn description(&self) -> &str {
        "Requires Administrator permission. A stub for testing permission denial, not a real action."
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::System
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Administrator
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(5)
    }

    fn input_schema(&self) -> InputSchema {
        InputSchema::default()
    }

    fn execute(&self, _args: &ToolArgs) -> ToolOutcome {
        ToolOutcome {
            success: true,
            message: "if you see this in a test, permission denial did NOT fire".to_string(),
            data: None,
        }
    }
}
