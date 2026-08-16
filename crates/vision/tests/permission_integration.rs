//! Integration test: `vision`'s tool running through `tool-framework`'s
//! router, gated by `permission-manager`'s real `PermissionManager` --
//! mirrors `linux-services/tests/permission_integration.rs`.

use permission_manager::{AlwaysDeny, PermissionManager, SecurityRules};
use std::sync::Arc;
use tool_framework::{ToolArgs, ToolRegistry, ToolRouter};
use vision::tools;

#[tokio::test]
async fn read_only_description_runs_without_any_confirmation() {
    let pm = PermissionManager::in_memory(SecurityRules::default(), Box::new(AlwaysDeny)).unwrap();
    let mut registry = ToolRegistry::new();
    tools::register_all(&mut registry).unwrap();
    let router = ToolRouter::new(registry, common::PermissionLevel::ReadOnly)
        .with_permission_checker(Arc::new(pm));

    let dir = std::env::temp_dir().join("vision-permission-integration-test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("solid.png");
    let img = image::RgbImage::from_fn(2, 2, |_, _| image::Rgb([1, 2, 3]));
    image::DynamicImage::ImageRgb8(img).save(&path).unwrap();

    let mut args = ToolArgs::new();
    args.insert("path".into(), serde_json::json!(path.to_str().unwrap()));
    let result = router.execute("describe_image", args).await;

    assert_ne!(
        result.status,
        tool_framework::ResultStatus::PermissionDenied
    );
    assert_eq!(
        result.status,
        tool_framework::ResultStatus::Ok,
        "{}",
        result.message
    );

    std::fs::remove_dir_all(&dir).ok();
}
