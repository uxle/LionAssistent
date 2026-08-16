//! Integration test: `ocr`'s tool running through `tool-framework`'s
//! router, gated by `permission-manager`'s real `PermissionManager` --
//! mirrors `linux-services/tests/permission_integration.rs`.

use ocr::tools;
use permission_manager::{AlwaysDeny, PermissionManager, SecurityRules};
use std::sync::Arc;
use tool_framework::{ToolArgs, ToolRegistry, ToolRouter};

#[tokio::test]
async fn read_only_extraction_runs_without_any_confirmation() {
    // AlwaysDeny would refuse if actually asked -- ReadOnly means it
    // never should be, the same property linux-services' own
    // `read_only_tools_run_without_any_confirmation` proves for
    // `system_info`.
    let pm = PermissionManager::in_memory(SecurityRules::default(), Box::new(AlwaysDeny)).unwrap();
    let mut registry = ToolRegistry::new();
    tools::register_all(&mut registry).unwrap();
    let router = ToolRouter::new(registry, common::PermissionLevel::ReadOnly)
        .with_permission_checker(Arc::new(pm));

    let mut args = ToolArgs::new();
    args.insert(
        "path".into(),
        serde_json::json!("/nonexistent/permission-integration-check.png"),
    );
    let result = router.execute("extract_text_from_image", args).await;

    // Never denied by AlwaysDeny (ReadOnly bypasses confirmation
    // entirely); the file genuinely not existing is a separate,
    // expected `ExecutionFailed`, not a permission problem.
    assert_ne!(
        result.status,
        tool_framework::ResultStatus::PermissionDenied
    );
}
