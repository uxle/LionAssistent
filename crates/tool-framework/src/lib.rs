//! `tool-framework` -- tool registry, router, validation, and timeout
//! enforcement (Development_plan Phase 7; TCLIS SS3 "Tool Execution
//! Pipeline").
//!
//! No real Linux tools live here -- that's Phase 8's job, and TCLIS SS1
//! is explicit that the LLM never performs OS actions directly, only
//! ever through this kind of validated, audited pipeline. [`stubs`]
//! provides tools that prove the pipeline actually works (registration,
//! validation, permission checks, genuine timeout enforcement) without
//! touching the real filesystem or OS, the same way `EchoResponseGenerator`
//! (Phase 3) and `KeywordClassifier` (Phase 6) proved their pipelines.
//!
//! This crate deliberately isn't wired into `assistant-core`'s message
//! flow yet. Doing that well needs either a real way to extract concrete
//! tool calls (arguments and all) from a user request -- which needs a
//! working `llm-runtime` (Phase 4) -- or real tools worth calling in the
//! first place (Phase 8). Wiring it up now against stub tools would mean
//! guessing at a design better made once there's something real on the
//! other end.

pub mod registry;
pub mod router;
pub mod schema;
pub mod stubs;

pub use common::PermissionLevel;
pub use registry::ToolRegistry;
pub use router::{PermissionChecker, ResultStatus, ToolResult, ToolRouter};
pub use schema::{InputSchema, ParamKind, ParamSchema, ToolArgs, ValidationError};

use std::time::Duration;

/// TCLIS SS4 tool categories.
///
/// `Vision` and `Development` aren't in the original TCLIS SS4 list --
/// both are Development_plan additions (Phase 23 and Phase 24
/// respectively), the same way `Search` and `Automation` were already
/// extensions beyond the SS4 base set rather than being redundant with
/// it. TCLIS SS15 "Future Expansion" names "OCR"/"local vision models"
/// (`Vision`); Ai_reasoning SS15 names a future "Coding Agent"
/// explicitly (`Development`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCategory {
    System,
    FileManagement,
    ApplicationManagement,
    ProcessManagement,
    Terminal,
    PackageManagement,
    Network,
    Display,
    Audio,
    Notification,
    Clipboard,
    Browser,
    Power,
    Search,
    Automation,
    Vision,
    Development,
}

/// What a tool itself reports after running (TCLIS SS8, minus the
/// execution-timing/status fields the router adds on top -- see
/// [`ToolResult`]).
#[derive(Debug, Clone)]
pub struct ToolOutcome {
    pub success: bool,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

/// TCLIS SS5 "Standard Tool Definition". `execute` is synchronous: no
/// real Linux tool exists yet to need true async I/O, and a sync trait
/// avoids either an extra dependency (`async-trait`) or the
/// dyn-compatibility complications of native async fn in traits for a
/// capability nothing needs yet. `ToolRouter` still enforces real,
/// genuine timeouts on synchronous tools by running them via
/// `spawn_blocking` and racing that against `tokio::time::timeout`.
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn category(&self) -> ToolCategory;
    fn permission_level(&self) -> PermissionLevel;
    fn timeout(&self) -> Duration;
    fn input_schema(&self) -> InputSchema;
    fn execute(&self, args: &ToolArgs) -> ToolOutcome;
}
