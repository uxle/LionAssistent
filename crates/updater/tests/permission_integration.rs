//! Integration test: `updater`'s tools running through `tool-framework`'s
//! router, gated by `permission-manager`'s real `PermissionManager`.
//! Mirrors `linux-services`' and `search`'s own `permission_integration.rs`
//! files -- proving `check_for_updates` (ReadOnly) needs no confirmation
//! and `download_update` (Moderate) genuinely does, against a real
//! confirmation handler, not just the router's simple threshold
//! fallback the crate's own unit tests use.

use network::testing::MockServer;
use network::HttpClient;
use permission_manager::{
    AlwaysApproveForTestingOnly, AlwaysDeny, PermissionManager, SecurityRules,
};
use std::sync::Arc;
use tool_framework::{ToolArgs, ToolRegistry, ToolRouter};
use updater::tools;

fn manifest_json(version: &str, download_url: &str) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "version": version,
        "download_url": download_url,
        "notes": "release notes",
    }))
    .unwrap()
}

fn router(
    manifest_url: String,
    dest_dir: std::path::PathBuf,
    pm: Arc<PermissionManager>,
) -> ToolRouter {
    let mut registry = ToolRegistry::new();
    tools::register_all(
        &mut registry,
        Arc::new(HttpClient::new().unwrap()),
        manifest_url,
        "1.0.0",
        dest_dir,
    )
    .unwrap();
    ToolRouter::new(registry, common::PermissionLevel::ReadOnly).with_permission_checker(pm)
}

#[tokio::test]
async fn check_for_updates_runs_without_confirmation_even_under_a_deny_everything_policy() {
    let server = MockServer::respond_once(
        200,
        "application/json",
        manifest_json("2.0.0", "http://x/y.tar.gz"),
    );
    let dir = std::env::temp_dir().join("updater-integration-readonly");

    let pm = Arc::new(
        PermissionManager::in_memory(SecurityRules::default(), Box::new(AlwaysDeny)).unwrap(),
    );
    let result = router(server.base_url.clone(), dir, pm)
        .execute("check_for_updates", ToolArgs::new())
        .await;
    server.join();

    assert!(result.success, "{}", result.message);
}

#[tokio::test]
async fn download_update_is_refused_when_confirmation_is_denied() {
    let dir = std::env::temp_dir().join("updater-integration-denied");
    std::fs::remove_dir_all(&dir).ok();

    // No artifact server at all: confirmation should be denied before
    // `download_update` ever reaches the manifest's download_url, so
    // nothing should try to connect anywhere past the manifest fetch.
    let manifest_server = MockServer::respond_once(
        200,
        "application/json",
        manifest_json("2.0.0", "http://127.0.0.1:1/unreachable.tar.gz"),
    );

    let pm = Arc::new(
        PermissionManager::in_memory(SecurityRules::default(), Box::new(AlwaysDeny)).unwrap(),
    );
    let result = router(manifest_server.base_url.clone(), dir.clone(), pm)
        .execute("download_update", ToolArgs::new())
        .await;
    // Deliberately not `manifest_server.join()`-ed: `ToolRouter::execute`
    // checks permission *before* ever calling `Tool::execute`
    // (tool-framework's router, "authorized" check precedes running the
    // tool), so a denied call never reaches `check_for_update` at all --
    // the mock server's one expected connection genuinely never arrives.
    // Joining here would deadlock the test waiting on a request that,
    // if this test is actually passing, can never come.

    assert!(!result.success);
    assert!(!dir.join("unreachable.tar.gz").exists());
}

#[tokio::test]
async fn download_update_proceeds_once_confirmation_is_approved() {
    let dir = std::env::temp_dir().join("updater-integration-approved");
    std::fs::remove_dir_all(&dir).ok();

    let artifact_server = MockServer::respond_once(200, "application/gzip", b"bytes".to_vec());
    let download_url = format!("{}/update.tar.gz", artifact_server.base_url);
    let manifest_server = MockServer::respond_once(
        200,
        "application/json",
        manifest_json("2.0.0", &download_url),
    );

    let pm = Arc::new(
        PermissionManager::in_memory(
            SecurityRules::default(),
            Box::new(AlwaysApproveForTestingOnly),
        )
        .unwrap(),
    );
    let result = router(manifest_server.base_url.clone(), dir.clone(), pm)
        .execute("download_update", ToolArgs::new())
        .await;
    manifest_server.join();
    artifact_server.join();

    assert!(result.success, "{}", result.message);
    assert!(dir.join("update.tar.gz").exists());

    std::fs::remove_dir_all(&dir).ok();
}
