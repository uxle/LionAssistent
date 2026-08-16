//! Integration test: `coding-agent`'s tools running through
//! `tool-framework`'s router, gated by `permission-manager`'s real
//! `PermissionManager` -- mirrors `linux-services/tests/
//! permission_integration.rs` and `screenshot/tests/
//! permission_integration.rs`'s exact pattern. This crate is worth
//! having one of these specifically because it has both a `ReadOnly`
//! tool and this workspace's first `Destructive`-level tool -- the
//! widest permission-level spread any single crate has introduced so
//! far.

use assistant_core::{EchoResponseGenerator, ResponseGenerator};
use coding_agent::tools;
use permission_manager::{
    AlwaysApproveForTestingOnly, AlwaysDeny, PermissionManager, SecurityRules,
};
use std::sync::{Arc, Mutex};
use tool_framework::{ResultStatus, ToolArgs, ToolRegistry, ToolRouter};

fn registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    tools::register_all(
        &mut registry,
        Arc::new(Mutex::new(
            Box::new(EchoResponseGenerator) as Box<dyn ResponseGenerator>
        )),
    )
    .unwrap();
    registry
}

fn real_rust_project_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("coding-agent-permission-integration-project");
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"permission-integration-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::write(dir.join("src/lib.rs"), "pub fn ok() -> i32 { 1 }\n").unwrap();
    dir
}

#[tokio::test]
async fn read_only_tools_run_without_any_confirmation() {
    let pm = PermissionManager::in_memory(SecurityRules::default(), Box::new(AlwaysDeny)).unwrap();
    let router = ToolRouter::new(registry(), common::PermissionLevel::ReadOnly)
        .with_permission_checker(Arc::new(pm));

    let mut analyze_args = ToolArgs::new();
    analyze_args.insert("path".into(), serde_json::json!(env!("CARGO_MANIFEST_DIR")));
    let analyze_result = router.execute("analyze_project", analyze_args).await;
    assert_ne!(analyze_result.status, ResultStatus::PermissionDenied);
    assert_eq!(
        analyze_result.status,
        ResultStatus::Ok,
        "{}",
        analyze_result.message
    );

    let mut generate_args = ToolArgs::new();
    generate_args.insert("kind".into(), serde_json::json!("function"));
    generate_args.insert("name".into(), serde_json::json!("f"));
    let generate_result = router.execute("generate_code", generate_args).await;
    assert_ne!(generate_result.status, ResultStatus::PermissionDenied);
    assert_eq!(
        generate_result.status,
        ResultStatus::Ok,
        "{}",
        generate_result.message
    );
}

#[tokio::test]
async fn destructive_level_diagnostics_is_denied_when_confirmation_says_no() {
    let pm = PermissionManager::in_memory(SecurityRules::default(), Box::new(AlwaysDeny)).unwrap();
    let router = ToolRouter::new(registry(), common::PermissionLevel::ReadOnly)
        .with_permission_checker(Arc::new(pm));

    let dir = real_rust_project_dir();
    let mut args = ToolArgs::new();
    args.insert("path".into(), serde_json::json!(dir.to_str().unwrap()));
    let result = router.execute("run_diagnostics", args).await;
    assert_eq!(result.status, ResultStatus::PermissionDenied);

    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn destructive_level_diagnostics_actually_asks_and_proceeds_when_confirmation_says_yes() {
    let pm = PermissionManager::in_memory(
        SecurityRules::default(),
        Box::new(AlwaysApproveForTestingOnly),
    )
    .unwrap();
    let router = ToolRouter::new(registry(), common::PermissionLevel::ReadOnly)
        .with_permission_checker(Arc::new(pm));

    let dir = real_rust_project_dir();
    let mut args = ToolArgs::new();
    args.insert("path".into(), serde_json::json!(dir.to_str().unwrap()));
    let result = router.execute("run_diagnostics", args).await;

    // Approval was granted, so this is a real cargo check run against a
    // real, clean fixture crate -- genuinely Ok, not just "not denied".
    assert_eq!(result.status, ResultStatus::Ok, "{}", result.message);

    std::fs::remove_dir_all(&dir).ok();
}
