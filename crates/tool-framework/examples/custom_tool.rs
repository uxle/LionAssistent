//! Defining and running a custom `Tool` -- the core extensibility
//! pattern this whole project is built around (TCLIS SS3: every Linux
//! action goes through this same pipeline, never called directly). Run
//! with:
//!
//!     cargo run -p tool-framework --example custom_tool

use common::PermissionLevel;
use std::sync::Arc;
use std::time::Duration;
use tool_framework::{
    InputSchema, ParamKind, ParamSchema, ResultStatus, Tool, ToolArgs, ToolCategory, ToolOutcome,
    ToolRegistry, ToolRouter,
};

/// A minimal real tool: counts words in a string. Every `Tool` needs
/// these six methods -- `execute` is the only one that does real work,
/// the rest describe the tool to the router (what it needs, what it's
/// allowed to do, how long it's allowed to take).
struct WordCountTool;

impl Tool for WordCountTool {
    fn name(&self) -> &str {
        "word_count"
    }

    fn description(&self) -> &str {
        "Counts the words in a string."
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::System
    }

    fn permission_level(&self) -> PermissionLevel {
        // Read-only, no side effects -- the lowest risk level (Archd
        // SS7 Level 0), so it never needs confirmation.
        PermissionLevel::ReadOnly
    }

    fn timeout(&self) -> Duration {
        Duration::from_secs(2)
    }

    fn input_schema(&self) -> InputSchema {
        InputSchema::new(vec![ParamSchema::required("text", ParamKind::String)])
    }

    fn execute(&self, args: &ToolArgs) -> ToolOutcome {
        let text = args
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let count = text.split_whitespace().count();
        ToolOutcome {
            success: true,
            message: format!("{count} words"),
            data: Some(serde_json::json!({ "count": count })),
        }
    }
}

#[tokio::main]
async fn main() {
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(WordCountTool)).unwrap();

    // A router configured to allow up to Safe-level tools -- ReadOnly
    // tools like this one always pass.
    let router = ToolRouter::new(registry, PermissionLevel::Safe);

    let mut args = ToolArgs::new();
    args.insert(
        "text".to_string(),
        serde_json::json!("the quick brown fox jumps over the lazy dog"),
    );
    let result = router.execute("word_count", args).await;
    println!("status:  {:?}", result.status);
    println!("message: {}", result.message);
    println!("data:    {:?}", result.data);
    assert_eq!(result.status, ResultStatus::Ok);

    // Calling a tool that was never registered fails the same
    // structured way a real invalid request would -- no panics, no
    // special-casing needed by the caller.
    let missing = router.execute("does_not_exist", ToolArgs::new()).await;
    println!("\nunregistered tool status: {:?}", missing.status);
    assert_eq!(missing.status, ResultStatus::ToolUnavailable);

    // Missing a required argument is caught by validation before
    // execute() ever runs.
    let invalid = router.execute("word_count", ToolArgs::new()).await;
    println!("missing-argument status:  {:?}", invalid.status);
    assert_eq!(invalid.status, ResultStatus::ValidationFailed);
}
