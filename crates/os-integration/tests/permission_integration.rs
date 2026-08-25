//! Proves this crate's own tools go through the real
//! validate/permission-check/timeout pipeline, mirroring the exact
//! pattern `rag`'s, `mcp`'s, and `remote-control`'s own
//! `permission_integration.rs` files already established.
//!
//! Deliberately does not need a real X display or a real systemd:
//! permission denial happens in `ToolRouter::execute` *before* a
//! tool's own `execute()` is ever called, so proving `CloseWindowTool`
//! (`Moderate`) or `StartUserServiceTool` (`Moderate`) is correctly
//! blocked by a denying `PermissionManager` needs no real window
//! manager or session bus at all -- the underlying, real
//! `wmctrl`/`systemctl` command this workspace's sandbox can't fully
//! exercise (see `windows`'s and `systemd`'s own module docs) is never
//! reached in the denied case, which is exactly the property being
//! tested.

use os_integration::tools::{
    ActivateWindowTool, CloseWindowTool, ListWindowsTool, StartUserServiceTool,
};
use permission_manager::{
    AlwaysApproveForTestingOnly, AlwaysDeny, PermissionManager, SecurityRules,
};
use std::sync::Arc;
use tool_framework::{PermissionLevel, ToolArgs, ToolRegistry, ToolRouter};

fn router_with(
    rules: SecurityRules,
    handler: Box<dyn permission_manager::ConfirmationHandler>,
) -> ToolRouter {
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(ListWindowsTool)).unwrap();
    registry.register(Arc::new(ActivateWindowTool)).unwrap();
    registry.register(Arc::new(CloseWindowTool)).unwrap();
    registry.register(Arc::new(StartUserServiceTool)).unwrap();
    let permissions = PermissionManager::in_memory(rules, handler).unwrap();
    ToolRouter::new(registry, PermissionLevel::ReadOnly)
        .with_permission_checker(Arc::new(permissions))
}

#[tokio::test]
async fn read_only_list_windows_is_never_blocked_by_a_denying_manager() {
    let router = router_with(SecurityRules::default(), Box::new(AlwaysDeny));
    let result = router.execute("list_windows", ToolArgs::new()).await;
    // Permission-wise this must pass through (ReadOnly auto-approves);
    // whether the underlying wmctrl call itself succeeds in this
    // sandbox is a separate, environment-dependent question this
    // assertion doesn't depend on either way.
    assert_ne!(format!("{:?}", result.status), "PermissionDenied");
}

#[tokio::test]
async fn moderate_close_window_is_blocked_by_a_denying_manager_before_wmctrl_ever_runs() {
    let router = router_with(SecurityRules::default(), Box::new(AlwaysDeny));
    let mut args = ToolArgs::new();
    args.insert("id".into(), serde_json::json!("0x0"));
    let result = router.execute("close_window", args).await;
    assert!(!result.success);
    assert_eq!(format!("{:?}", result.status), "PermissionDenied");
}

#[tokio::test]
async fn moderate_close_window_is_allowed_through_by_an_approving_manager() {
    let router = router_with(
        SecurityRules::default(),
        Box::new(AlwaysApproveForTestingOnly),
    );
    let mut args = ToolArgs::new();
    args.insert("id".into(), serde_json::json!("0x0"));
    let result = router.execute("close_window", args).await;
    // Permission was granted; the *tool itself* may still fail (no
    // real X display in a plain `cargo test` process) -- that failure
    // must come back as ExecutionFailed or similar, never
    // PermissionDenied, proving the gate opened even though the room
    // behind it turned out to be empty.
    assert_ne!(format!("{:?}", result.status), "PermissionDenied");
}

#[tokio::test]
async fn moderate_start_user_service_is_blocked_by_a_denying_manager() {
    let router = router_with(SecurityRules::default(), Box::new(AlwaysDeny));
    let mut args = ToolArgs::new();
    args.insert("unit".into(), serde_json::json!("anything.service"));
    let result = router.execute("start_user_service", args).await;
    assert!(!result.success);
    assert_eq!(format!("{:?}", result.status), "PermissionDenied");
}

#[tokio::test]
async fn safe_activate_window_auto_approves_below_the_router_ceiling() {
    // Safe is below this router's max_auto_level (ReadOnly is the
    // floor passed to ToolRouter::new; Safe still needs a checker to
    // approve unless it's within the router's own auto-approve
    // threshold -- this proves it reaches the checker/is granted
    // rather than being silently rejected as unregistered or
    // malformed).
    let router = router_with(
        SecurityRules::default(),
        Box::new(AlwaysApproveForTestingOnly),
    );
    let mut args = ToolArgs::new();
    args.insert("id".into(), serde_json::json!("0x0"));
    let result = router.execute("activate_window", args).await;
    assert_ne!(format!("{:?}", result.status), "PermissionDenied");
}
