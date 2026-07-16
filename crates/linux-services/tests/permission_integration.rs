//! Integration test: `linux-services` tools running through
//! `tool-framework`'s router, gated by `permission-manager`'s real
//! `PermissionManager` -- not the router's simple `max_auto_level`
//! fallback. This is the first time all three Phase 7/8/9 crates have
//! actually been exercised together, proving the full pipeline TCLIS SS3
//! describes: lookup -> validate -> **real** permission check (policy,
//! confirmation, audit) -> execute -> structured result.

use linux_services::tools;
use permission_manager::{
    AlwaysApproveForTestingOnly, AlwaysDeny, PermissionManager, SecurityRules,
};
use std::sync::Arc;
use tool_framework::{ToolArgs, ToolRegistry, ToolRouter};

fn router_with_real_permission_manager(pm: PermissionManager) -> ToolRouter {
    let mut registry = ToolRegistry::new();
    tools::register_all(&mut registry).unwrap();
    // max_auto_level here is irrelevant -- a real checker is configured,
    // so it takes over entirely (tool-framework's own test already
    // confirms a configured checker overrides this fallback).
    ToolRouter::new(registry, common::PermissionLevel::ReadOnly)
        .with_permission_checker(Arc::new(pm))
}

#[tokio::test]
async fn read_only_tools_run_without_any_confirmation() {
    // AlwaysDeny would refuse if actually asked -- system_info being
    // ReadOnly means it never should be.
    let pm = PermissionManager::in_memory(SecurityRules::default(), Box::new(AlwaysDeny)).unwrap();
    let router = router_with_real_permission_manager(pm);

    let result = router.execute("system_info", ToolArgs::new()).await;
    assert!(result.success, "{}", result.message);
}

#[tokio::test]
async fn destructive_tool_is_denied_when_the_confirmation_handler_says_no() {
    let dir = std::env::temp_dir().join("linux-services-integration-deny");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("should-survive.txt");
    std::fs::write(&path, "still here").unwrap();

    let pm = PermissionManager::in_memory(SecurityRules::default(), Box::new(AlwaysDeny)).unwrap();
    let router = router_with_real_permission_manager(pm);

    let mut args = ToolArgs::new();
    args.insert("path".into(), serde_json::json!(path.to_str().unwrap()));
    let result = router.execute("delete_file", args).await;

    assert!(!result.success);
    assert!(path.exists(), "file must survive a denied confirmation");

    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn destructive_tool_actually_runs_when_the_confirmation_handler_says_yes() {
    let dir = std::env::temp_dir().join("linux-services-integration-approve");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("goodbye.txt");
    std::fs::write(&path, "delete me for real").unwrap();

    let pm = PermissionManager::in_memory(
        SecurityRules::default(),
        Box::new(AlwaysApproveForTestingOnly),
    )
    .unwrap();
    let router = router_with_real_permission_manager(pm);

    let mut args = ToolArgs::new();
    args.insert("path".into(), serde_json::json!(path.to_str().unwrap()));
    let result = router.execute("delete_file", args).await;

    assert!(result.success, "{}", result.message);
    assert!(
        !path.exists(),
        "an approved confirmation should have actually deleted the file"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn administrator_level_tools_are_denied_by_policy_before_confirmation_is_ever_asked() {
    // There's no registered Administrator-level tool in linux-services'
    // tools module (power actions aren't wrapped -- see docs/ROADMAP.md),
    // so this exercises the policy layer directly through the same
    // PermissionManager the router uses, confirming the two stay
    // consistent with each other.
    let pm = PermissionManager::in_memory(
        SecurityRules::default(),
        Box::new(AlwaysApproveForTestingOnly),
    )
    .unwrap();
    use tool_framework::PermissionChecker;
    assert!(!pm.authorize(
        "shutdown",
        common::PermissionLevel::Administrator,
        "shut the machine down"
    ));
}

#[tokio::test]
async fn a_remembered_approval_persists_across_repeated_calls() {
    let dir = std::env::temp_dir().join("linux-services-integration-remember");
    std::fs::create_dir_all(&dir).unwrap();

    let pm = PermissionManager::in_memory(SecurityRules::default(), Box::new(AlwaysDeny)).unwrap();
    pm.remember_decision("delete_file", true, None).unwrap();
    let router = router_with_real_permission_manager(pm);

    // Two separate files, two separate calls -- both should succeed
    // without ever consulting AlwaysDeny, since the decision is remembered.
    for name in ["first.txt", "second.txt"] {
        let path = dir.join(name);
        std::fs::write(&path, "temp").unwrap();
        let mut args = ToolArgs::new();
        args.insert("path".into(), serde_json::json!(path.to_str().unwrap()));
        let result = router.execute("delete_file", args).await;
        assert!(result.success, "{}", result.message);
        assert!(!path.exists());
    }

    std::fs::remove_dir_all(&dir).ok();
}
