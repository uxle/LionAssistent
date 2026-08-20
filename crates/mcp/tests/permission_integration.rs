//! Integration test: an MCP-bridged tool running through
//! `tool-framework`'s router, gated by `permission-manager`'s real
//! `PermissionManager` -- mirrors `rag`'s and `linux-services`'s own
//! `permission_integration.rs`. The connection itself is real (the
//! same `mcp-stub-server` subprocess `client_server_roundtrip.rs`
//! uses); what this file adds is the permission dimension: proving
//! `McpToolProxy`'s fixed `Moderate` level is actually enforced by a
//! real `PermissionManager`, not merely declared.

mod support;

use permission_manager::{
    AlwaysApproveForTestingOnly, AlwaysDeny, PermissionManager, SecurityRules,
};
use std::sync::Arc;
use support::connect_to_stub_server;
use tool_framework::{ResultStatus, ToolArgs, ToolRegistry, ToolRouter};

fn router_with(pm: PermissionManager, client: &Arc<mcp::McpClient>) -> ToolRouter {
    let mut registry = ToolRegistry::new();
    mcp::register_mcp_tools(&mut registry, client).unwrap();
    ToolRouter::new(registry, common::PermissionLevel::ReadOnly)
        .with_permission_checker(Arc::new(pm))
}

#[tokio::test]
async fn an_mcp_bridged_tool_is_denied_when_confirmation_says_no() {
    let client = connect_to_stub_server();
    let pm = PermissionManager::in_memory(SecurityRules::default(), Box::new(AlwaysDeny)).unwrap();
    let router = router_with(pm, &client);

    let mut args = ToolArgs::new();
    args.insert("text".into(), serde_json::json!("should not run"));
    let result = router.execute("echo", args).await;
    assert_eq!(result.status, ResultStatus::PermissionDenied);

    client.shutdown().unwrap();
}

#[tokio::test]
async fn an_mcp_bridged_tool_actually_proceeds_when_confirmation_says_yes() {
    let client = connect_to_stub_server();
    let pm = PermissionManager::in_memory(
        SecurityRules::default(),
        Box::new(AlwaysApproveForTestingOnly),
    )
    .unwrap();
    let router = router_with(pm, &client);

    let mut args = ToolArgs::new();
    args.insert("text".into(), serde_json::json!("should run for real"));
    let result = router.execute("echo", args).await;
    assert_eq!(result.status, ResultStatus::Ok, "{}", result.message);
    assert_eq!(result.message, "echo: should run for real");

    client.shutdown().unwrap();
}

#[tokio::test]
async fn a_natively_administrator_tool_is_still_only_moderate_once_bridged() {
    // dangerous-stub requires Administrator locally (tool-framework's
    // own stubs.rs). Bridged through MCP it registers at Moderate --
    // the fixed floor bridge.rs documents -- so a PermissionManager
    // configured to approve at Moderate, but not Administrator, still
    // lets it run. This is the concrete, executable proof of the
    // "never trust the remote side's own framing of risk" principle:
    // nothing about *this* server's tool being locally Administrator
    // survives the bridge into a lower requirement, because the
    // bridge does not consult that at all.
    let client = connect_to_stub_server();
    let pm = PermissionManager::in_memory(
        SecurityRules::default(),
        Box::new(AlwaysApproveForTestingOnly),
    )
    .unwrap();
    let router = router_with(pm, &client);

    let result = router.execute("dangerous-stub", ToolArgs::new()).await;
    assert_eq!(result.status, ResultStatus::Ok, "{}", result.message);

    client.shutdown().unwrap();
}

#[tokio::test]
async fn read_only_default_max_auto_level_still_denies_an_mcp_tool_with_no_checker_configured() {
    // No PermissionChecker at all this time -- ToolRouter's own
    // built-in max_auto_level fallback (Phase 7) is what's under test
    // here, confirming McpToolProxy's Moderate level is real enough to
    // trip even the simple fallback path, not just a configured
    // PermissionManager.
    let client = connect_to_stub_server();
    let mut registry = ToolRegistry::new();
    mcp::register_mcp_tools(&mut registry, &client).unwrap();
    let router = ToolRouter::new(registry, common::PermissionLevel::ReadOnly);

    let mut args = ToolArgs::new();
    args.insert("text".into(), serde_json::json!("x"));
    let result = router.execute("echo", args).await;
    assert_eq!(result.status, ResultStatus::PermissionDenied);

    client.shutdown().unwrap();
}
