//! The tool execution pipeline (TCLIS SS3): lookup, validate, permission
//! check, execute (with a real, enforced timeout), structured result.
//!
//! `execute` never returns a Rust-level error for a tool-side problem --
//! every outcome (unavailable, invalid, denied, timed out, failed, or
//! genuinely successful) is a [`ToolResult`], because TCLIS SS10 treats
//! all of these as ordinary, structured, user-facing outcomes rather
//! than exceptional control flow.

use crate::{PermissionLevel, ToolArgs, ToolRegistry};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Something that can authorize a tool call for real (Phase 9's
/// Permission Manager implements this). `&self` and a plain `bool`
/// return rather than `async`/`Result`: a real UI confirmation dialog
/// isn't wired up yet (Phase 2's UI has its own toolchain constraint --
/// see docs/ROADMAP.md), so there's nothing to genuinely `.await` here
/// yet, and any internal failure (e.g. a database error) should fail
/// closed (deny) rather than propagate as a separate error path the
/// router would need to handle differently from an ordinary denial.
pub trait PermissionChecker: Send + Sync {
    fn authorize(&self, tool_name: &str, level: PermissionLevel, description: &str) -> bool;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultStatus {
    Ok,
    ValidationFailed,
    PermissionDenied,
    ToolUnavailable,
    TimedOut,
    ExecutionFailed,
}

/// TCLIS SS8 "Standard Result Format".
#[derive(Debug, Clone)]
pub struct ToolResult {
    pub success: bool,
    pub status: ResultStatus,
    pub message: String,
    pub data: Option<serde_json::Value>,
    pub execution_time: Duration,
    pub warnings: Vec<String>,
}

impl std::fmt::Display for ToolResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{:?}] {} ({:.1}ms)",
            self.status,
            self.message,
            self.execution_time.as_secs_f64() * 1000.0
        )
    }
}

fn failure(status: ResultStatus, message: String, started: Instant) -> ToolResult {
    ToolResult {
        success: false,
        status,
        message,
        data: None,
        execution_time: started.elapsed(),
        warnings: vec![],
    }
}

pub struct ToolRouter {
    registry: ToolRegistry,
    /// Used only when no `checker` is configured: tools at or below this
    /// level run without confirmation, above it they're denied outright.
    /// A simple built-in fallback, not a substitute for a real
    /// `PermissionChecker`.
    max_auto_level: PermissionLevel,
    checker: Option<Arc<dyn PermissionChecker>>,
}

impl ToolRouter {
    pub fn new(registry: ToolRegistry, max_auto_level: PermissionLevel) -> Self {
        Self {
            registry,
            max_auto_level,
            checker: None,
        }
    }

    /// Replaces the simple `max_auto_level` threshold with a real
    /// `PermissionChecker` (Phase 9's `PermissionManager`, most likely).
    pub fn with_permission_checker(mut self, checker: Arc<dyn PermissionChecker>) -> Self {
        self.checker = Some(checker);
        self
    }

    pub fn registry(&self) -> &ToolRegistry {
        &self.registry
    }

    pub fn registry_mut(&mut self) -> &mut ToolRegistry {
        &mut self.registry
    }

    /// Runs the full pipeline (TCLIS SS3) for one tool call.
    pub async fn execute(&self, name: &str, args: ToolArgs) -> ToolResult {
        let started = Instant::now();

        let Some(tool) = self.registry.get(name) else {
            return failure(
                ResultStatus::ToolUnavailable,
                format!("no tool registered named {name:?}"),
                started,
            );
        };

        if let Err(errors) = tool.input_schema().validate(&args) {
            let details = errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ");
            return failure(
                ResultStatus::ValidationFailed,
                format!("{} validation error(s): {details}", errors.len()),
                started,
            );
        }

        let authorized = match &self.checker {
            Some(checker) => checker.authorize(name, tool.permission_level(), tool.description()),
            None => tool.permission_level() <= self.max_auto_level,
        };
        if !authorized {
            return failure(
                ResultStatus::PermissionDenied,
                format!(
                    "{name} (permission level {:?}) was not authorized to run",
                    tool.permission_level()
                ),
                started,
            );
        }

        let timeout_duration = tool.timeout();
        let execution = {
            let tool = tool.clone();
            let args = args.clone();
            tokio::task::spawn_blocking(move || tool.execute(&args))
        };

        match tokio::time::timeout(timeout_duration, execution).await {
            Ok(Ok(outcome)) => ToolResult {
                success: outcome.success,
                status: if outcome.success {
                    ResultStatus::Ok
                } else {
                    ResultStatus::ExecutionFailed
                },
                message: outcome.message,
                data: outcome.data,
                execution_time: started.elapsed(),
                warnings: Vec::new(),
            },
            Ok(Err(join_error)) => failure(
                ResultStatus::ExecutionFailed,
                format!("{name} panicked during execution: {join_error}"),
                started,
            ),
            Err(_elapsed) => failure(
                ResultStatus::TimedOut,
                format!("{name} did not complete within {timeout_duration:?}"),
                started,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stubs::{DangerousStubTool, EchoTool, SlowTool};
    use std::sync::Arc;

    fn router_with(
        tools: Vec<Arc<dyn crate::Tool>>,
        max_auto_level: PermissionLevel,
    ) -> ToolRouter {
        let mut registry = ToolRegistry::new();
        for tool in tools {
            registry.register(tool).unwrap();
        }
        ToolRouter::new(registry, max_auto_level)
    }

    #[tokio::test]
    async fn unregistered_tool_is_unavailable() {
        let router = router_with(vec![], PermissionLevel::Safe);
        let result = router.execute("nonexistent", ToolArgs::new()).await;
        assert_eq!(result.status, ResultStatus::ToolUnavailable);
        assert!(!result.success);
    }

    #[tokio::test]
    async fn valid_call_succeeds() {
        let router = router_with(vec![Arc::new(EchoTool)], PermissionLevel::Safe);
        let mut args = ToolArgs::new();
        args.insert("text".into(), serde_json::json!("hi"));

        let result = router.execute("echo", args).await;
        assert_eq!(result.status, ResultStatus::Ok);
        assert!(result.success);
        assert_eq!(result.message, "echo: hi");
    }

    #[tokio::test]
    async fn missing_required_arg_fails_validation() {
        let router = router_with(vec![Arc::new(EchoTool)], PermissionLevel::Safe);
        let result = router.execute("echo", ToolArgs::new()).await;
        assert_eq!(result.status, ResultStatus::ValidationFailed);
    }

    #[tokio::test]
    async fn slow_tool_is_genuinely_timed_out() {
        // SlowTool sleeps 500ms with a 50ms declared timeout. If this
        // ever comes back Ok instead of TimedOut, timeout enforcement is
        // broken, not just slow.
        let router = router_with(vec![Arc::new(SlowTool)], PermissionLevel::Safe);
        let result = router.execute("slow-stub", ToolArgs::new()).await;
        assert_eq!(result.status, ResultStatus::TimedOut);
        assert!(result.execution_time < Duration::from_millis(500));
    }

    #[tokio::test]
    async fn above_max_permission_level_is_denied() {
        let router = router_with(vec![Arc::new(DangerousStubTool)], PermissionLevel::Moderate);
        let result = router.execute("dangerous-stub", ToolArgs::new()).await;
        assert_eq!(result.status, ResultStatus::PermissionDenied);
    }

    #[tokio::test]
    async fn at_max_permission_level_is_allowed() {
        let router = router_with(
            vec![Arc::new(DangerousStubTool)],
            PermissionLevel::Administrator,
        );
        let result = router.execute("dangerous-stub", ToolArgs::new()).await;
        assert_eq!(result.status, ResultStatus::Ok);
    }

    #[tokio::test]
    async fn a_configured_checker_overrides_the_max_auto_level_fallback() {
        struct AlwaysDeny;
        impl PermissionChecker for AlwaysDeny {
            fn authorize(
                &self,
                _tool_name: &str,
                _level: PermissionLevel,
                _description: &str,
            ) -> bool {
                false
            }
        }

        // EchoTool is ReadOnly, well within Administrator's max_auto_level
        // -- but a configured checker takes priority over that fallback
        // entirely, denying anyway.
        let router = router_with(vec![Arc::new(EchoTool)], PermissionLevel::Administrator)
            .with_permission_checker(Arc::new(AlwaysDeny));

        let mut args = ToolArgs::new();
        args.insert("text".into(), serde_json::json!("hi"));
        let result = router.execute("echo", args).await;
        assert_eq!(result.status, ResultStatus::PermissionDenied);
    }

    #[test]
    fn display_includes_status_and_message() {
        let result = ToolResult {
            success: true,
            status: ResultStatus::Ok,
            message: "did the thing".into(),
            data: None,
            execution_time: Duration::from_millis(12),
            warnings: vec![],
        };
        let rendered = result.to_string();
        assert!(rendered.contains("Ok"));
        assert!(rendered.contains("did the thing"));
    }
}
