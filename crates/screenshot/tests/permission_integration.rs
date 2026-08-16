//! Integration test: `screenshot`'s tool running through `tool-framework`'s
//! router, gated by `permission-manager`'s real `PermissionManager` --
//! mirrors `linux-services/tests/permission_integration.rs` exactly, the
//! established pattern for any crate whose tools touch something the
//! permission/security model cares about (DEVELOPER_GUIDE.md). This one
//! is worth having specifically because `capture_screenshot` is
//! `Moderate`, not `ReadOnly` -- confirming it actually *requires*
//! confirmation, rather than just declaring the level and hoping.
//!
//! The "approved" test needs a real display server (see screenshot's
//! crate-level doc comment) to reach a genuine success outcome; without
//! one it still exercises the real confirmation-granted code path, just
//! ending in `capture_full_screen`'s own honest capture failure rather
//! than the router denying it -- which is itself the thing this test is
//! checking (that approval was consulted and said yes) and is asserted
//! for accordingly.

use permission_manager::{
    AlwaysApproveForTestingOnly, AlwaysDeny, PermissionManager, SecurityRules,
};
use screenshot::tools;
use std::sync::Arc;
use tool_framework::{ResultStatus, ToolArgs, ToolRegistry, ToolRouter};

fn router_with_real_permission_manager(pm: PermissionManager) -> ToolRouter {
    let mut registry = ToolRegistry::new();
    tools::register_all(&mut registry).unwrap();
    ToolRouter::new(registry, common::PermissionLevel::ReadOnly)
        .with_permission_checker(Arc::new(pm))
}

#[tokio::test]
async fn moderate_level_capture_is_denied_when_confirmation_says_no() {
    let pm = PermissionManager::in_memory(SecurityRules::default(), Box::new(AlwaysDeny)).unwrap();
    let router = router_with_real_permission_manager(pm);

    let result = router.execute("capture_screenshot", ToolArgs::new()).await;
    assert_eq!(result.status, ResultStatus::PermissionDenied);
}

#[tokio::test]
async fn moderate_level_capture_actually_asks_and_proceeds_when_confirmation_says_yes() {
    let pm = PermissionManager::in_memory(
        SecurityRules::default(),
        Box::new(AlwaysApproveForTestingOnly),
    )
    .unwrap();
    let router = router_with_real_permission_manager(pm);

    let mut args = ToolArgs::new();
    args.insert(
        "output_dir".into(),
        serde_json::json!(std::env::temp_dir()
            .join("screenshot-permission-integration-test")
            .to_string_lossy()),
    );
    let result = router.execute("capture_screenshot", args).await;

    // Never PermissionDenied: approval was granted, so whatever happens
    // next is `capture_full_screen`'s own outcome (real success against
    // Xvfb, or a clean, honest "no display" failure without one) -- not
    // the router refusing to even try.
    assert_ne!(result.status, ResultStatus::PermissionDenied);
}

#[tokio::test]
async fn a_remembered_approval_persists_across_repeated_capture_calls() {
    let pm = PermissionManager::in_memory(SecurityRules::default(), Box::new(AlwaysDeny)).unwrap();
    pm.remember_decision("capture_screenshot", true, None)
        .unwrap();
    let router = router_with_real_permission_manager(pm);

    for _ in 0..2 {
        let mut args = ToolArgs::new();
        args.insert(
            "output_dir".into(),
            serde_json::json!(std::env::temp_dir()
                .join("screenshot-permission-integration-remember")
                .to_string_lossy()),
        );
        let result = router.execute("capture_screenshot", args).await;
        assert_ne!(
            result.status,
            ResultStatus::PermissionDenied,
            "a remembered approval should never fall back to asking AlwaysDeny"
        );
    }
}
