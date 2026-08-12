//! Integration tests spanning combinations of crates that no existing
//! test -- crate-local or otherwise -- already exercises together. See
//! `src/lib.rs` for why this crate exists and what it deliberately does
//! *not* try to prove (the LLM-driven tool-call extraction that still
//! needs a working `llm-runtime`/Candle).

use common::PermissionLevel;
use permission_manager::{AlwaysApproveForTestingOnly, PermissionManager, SecurityRules};
use plugin_manager::{
    Plugin, PluginManager, PluginManifest, PluginMetadata, PluginPermissionPolicy,
};
use std::sync::Arc;
use tool_framework::{
    ResultStatus, Tool, ToolArgs, ToolCategory, ToolOutcome, ToolRegistry, ToolRouter,
};

fn minimal_manifest(id: &str) -> PluginManifest {
    PluginManifest::from_toml(&format!(
        "id = {id:?}\nname = \"Test\"\nversion = \"1.0.0\"\nauthor = \"test\"\ndescription = \"test\"\n\
         license = \"MIT\"\nminimum_sdk_version = \"0.1.0\"\nsupported_platforms = [\"linux\"]\npermissions = []\n"
    ))
    .unwrap()
}

fn activated_plugin_manager(id: &str, instance: Box<dyn Plugin>) -> PluginManager {
    let mut manager = PluginManager::new();
    manager.register(minimal_manifest(id), instance).unwrap();
    manager
        .validate(id, &PluginPermissionPolicy::new())
        .unwrap();
    manager.activate(id).unwrap();
    manager
}

struct EchoPlugin;
impl Plugin for EchoPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            id: "test.echo-plugin".into(),
            name: "Echo Plugin".into(),
            version: "1.0.0".into(),
        }
    }
    fn initialize(&mut self) -> common::AppResult<()> {
        Ok(())
    }
    fn shutdown(&mut self) -> common::AppResult<()> {
        Ok(())
    }
    fn register_tools(&self) -> Vec<Arc<dyn Tool>> {
        vec![Arc::new(tool_framework::stubs::EchoTool)]
    }
}

/// A tool whose `execute()` panics -- standing in for a genuinely broken
/// or malicious plugin tool, the same role `SlowTool` (tool-framework's
/// own tests) plays for timeouts.
struct PanickingTool;
impl Tool for PanickingTool {
    fn name(&self) -> &str {
        "panicking-plugin-tool"
    }
    fn description(&self) -> &str {
        "Always panics. Proves the router isolates a plugin tool's panic the same way it would a built-in tool's."
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::System
    }
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
    fn timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(5)
    }
    fn input_schema(&self) -> tool_framework::InputSchema {
        tool_framework::InputSchema::default()
    }
    fn execute(&self, _args: &ToolArgs) -> ToolOutcome {
        panic!("deliberate panic from a plugin-contributed tool");
    }
}

struct PanickingPlugin;
impl Plugin for PanickingPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata {
            id: "test.panicking-plugin".into(),
            name: "Panicking Plugin".into(),
            version: "1.0.0".into(),
        }
    }
    fn initialize(&mut self) -> common::AppResult<()> {
        Ok(())
    }
    fn shutdown(&mut self) -> common::AppResult<()> {
        Ok(())
    }
    fn register_tools(&self) -> Vec<Arc<dyn Tool>> {
        vec![Arc::new(PanickingTool)]
    }
}

fn router_from(tools: Vec<Arc<dyn Tool>>) -> ToolRouter {
    let mut registry = ToolRegistry::new();
    for tool in tools {
        registry.register(tool).unwrap();
    }
    ToolRouter::new(registry, PermissionLevel::Safe)
}

/// `plugin_manager::PluginManager::collect_tools` already proves it can
/// gather a `Vec<Arc<dyn Tool>>` from active plugins (that crate's own
/// tests). What's new here: pushing that output the rest of the way,
/// into a real `ToolRegistry` and executing it through a real
/// `ToolRouter`, exactly the path the `Plugin` trait's own doc comment
/// promises ("identical validation, permission checks, and timeout
/// enforcement ... No separate, weaker path") -- checked, not just
/// asserted in a comment.
#[tokio::test]
async fn a_plugin_contributed_tool_runs_through_the_real_router() {
    let manager = activated_plugin_manager("test.echo-plugin", Box::new(EchoPlugin));
    let router = router_from(manager.collect_tools());

    let mut args = ToolArgs::new();
    args.insert("text".into(), serde_json::json!("hello from a plugin"));
    let result = router.execute("echo", args).await;

    assert_eq!(result.status, ResultStatus::Ok);
    assert_eq!(result.message, "echo: hello from a plugin");
}

/// `plugin_manager::PluginManager::activate`'s own tests already prove a
/// panic during a plugin's `initialize()`/`shutdown()` is caught
/// (`catch_unwind`, marks the plugin `Failed`). That's a *different*
/// panic path from a plugin's tool panicking during `execute()`, which
/// runs through `tool_framework::ToolRouter::execute`'s `spawn_blocking`
/// instead -- untested, for a plugin-sourced tool specifically, until
/// now.
#[tokio::test]
async fn a_plugin_tools_panic_during_execution_is_isolated_not_a_crash() {
    let manager = activated_plugin_manager("test.panicking-plugin", Box::new(PanickingPlugin));
    let router = router_from(manager.collect_tools());

    let result = router
        .execute("panicking-plugin-tool", ToolArgs::new())
        .await;

    assert_eq!(result.status, ResultStatus::ExecutionFailed);
    assert!(!result.success);
    // The test process is still running to make this assertion at all --
    // that's the actual proof. The message content is a bonus check.
    assert!(result.message.contains("panicked"));
}

/// `assistant-core` (conversation), `tool-framework` + `permission-manager`
/// (a real, gated tool call), and `memory` (persistence) have never been
/// exercised in the same scenario before -- every phase since 7 has
/// explicitly noted that wiring assistant-core's live message flow to
/// real tool calls needs a working `llm-runtime` (Phase 4, blocked on
/// Candle) to extract concrete tool calls from a model's output. This
/// test does **not** change that -- it does not parse a tool call out of
/// `assistant-core`'s reply. What it does show: the four pieces that
/// *would* need to cooperate once that extraction exists already do,
/// today, at the data level -- a conversation session, a real permission
/// decision with its own audit trail, and a tool execution result can
/// all be produced side by side and persisted/inspected together without
/// any interface friction between the crates.
#[tokio::test]
async fn assistant_core_tool_execution_and_memory_cooperate_in_one_scenario() {
    // 1. A real conversation, independent of any tool call.
    let mut assistant =
        assistant_core::AssistantCore::new(Box::new(assistant_core::EchoResponseGenerator));
    let session_id = assistant.create_session("gemma-3-1b");
    let reply = assistant
        .process_message(session_id, "what's my disk usage?")
        .unwrap();
    assert!(reply.contains("pipeline OK"));

    // 2. A real tool call, gated by a real PermissionManager -- entirely
    // independent of assistant-core, the way it will keep being until
    // Phase 4 unblocks.
    let pm = PermissionManager::in_memory(
        SecurityRules::default(),
        Box::new(AlwaysApproveForTestingOnly),
    )
    .unwrap();
    let mut registry = ToolRegistry::new();
    linux_services::tools::register_all(&mut registry).unwrap();
    let router =
        ToolRouter::new(registry, PermissionLevel::ReadOnly).with_permission_checker(Arc::new(pm));
    let tool_result = router.execute("system_info", ToolArgs::new()).await;
    assert!(tool_result.success, "{}", tool_result.message);

    // 3. Persistence: the conversation session round-trips through a
    // real MemoryStore, the same one a live app would use.
    let memory_store =
        memory::MemoryStore::in_memory(Box::<embeddings::HashingEmbedder>::default()).unwrap();
    let session = assistant.session(session_id).unwrap();
    memory_store.save_session(session).unwrap();

    let sessions = memory_store.list_sessions().unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, session_id);
}
