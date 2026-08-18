//! Integration test: `rag`'s tools running through `tool-framework`'s
//! router, gated by `permission-manager`'s real `PermissionManager` --
//! mirrors `linux-services/tests/permission_integration.rs`. Worth
//! having one of these specifically because this crate spans three
//! different permission levels (`ReadOnly` for search/list, `Moderate`
//! for indexing, `Destructive` for forgetting) -- `coding-agent`'s own
//! `permission_integration.rs` is the only other crate with that same
//! spread.

use embeddings::HashingEmbedder;
use permission_manager::{
    AlwaysApproveForTestingOnly, AlwaysDeny, PermissionManager, SecurityRules,
};
use rag::store::KnowledgeBase;
use rag::tools;
use std::sync::{Arc, Mutex};
use tool_framework::{ResultStatus, ToolArgs, ToolRegistry, ToolRouter};

fn router_with(pm: PermissionManager) -> ToolRouter {
    let kb = Arc::new(Mutex::new(
        KnowledgeBase::in_memory(Box::new(HashingEmbedder::new(64))).unwrap(),
    ));
    let mut registry = ToolRegistry::new();
    tools::register_all(&mut registry, kb).unwrap();
    ToolRouter::new(registry, common::PermissionLevel::ReadOnly)
        .with_permission_checker(Arc::new(pm))
}

fn real_text_file() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("rag-permission-integration");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("doc.txt");
    std::fs::write(&path, "a real fact about permission gating").unwrap();
    path
}

#[tokio::test]
async fn read_only_search_and_list_run_without_any_confirmation() {
    let pm = PermissionManager::in_memory(SecurityRules::default(), Box::new(AlwaysDeny)).unwrap();
    let router = router_with(pm);

    let mut search_args = ToolArgs::new();
    search_args.insert("query".into(), serde_json::json!("anything"));
    let search_result = router.execute("search_knowledge_base", search_args).await;
    assert_ne!(search_result.status, ResultStatus::PermissionDenied);
    assert_eq!(search_result.status, ResultStatus::Ok);

    let list_result = router
        .execute("list_indexed_documents", ToolArgs::new())
        .await;
    assert_ne!(list_result.status, ResultStatus::PermissionDenied);
    assert_eq!(list_result.status, ResultStatus::Ok);
}

#[tokio::test]
async fn moderate_level_indexing_is_denied_when_confirmation_says_no() {
    let pm = PermissionManager::in_memory(SecurityRules::default(), Box::new(AlwaysDeny)).unwrap();
    let router = router_with(pm);

    let path = real_text_file();
    let mut args = ToolArgs::new();
    args.insert("path".into(), serde_json::json!(path.to_str().unwrap()));
    let result = router.execute("index_document", args).await;
    assert_eq!(result.status, ResultStatus::PermissionDenied);

    std::fs::remove_dir_all(path.parent().unwrap()).ok();
}

#[tokio::test]
async fn moderate_level_indexing_actually_proceeds_when_confirmation_says_yes() {
    let pm = PermissionManager::in_memory(
        SecurityRules::default(),
        Box::new(AlwaysApproveForTestingOnly),
    )
    .unwrap();
    let router = router_with(pm);

    let path = real_text_file();
    let mut args = ToolArgs::new();
    args.insert("path".into(), serde_json::json!(path.to_str().unwrap()));
    let result = router.execute("index_document", args).await;
    assert_eq!(result.status, ResultStatus::Ok, "{}", result.message);

    std::fs::remove_dir_all(path.parent().unwrap()).ok();
}

#[tokio::test]
async fn destructive_level_forgetting_is_denied_when_confirmation_says_no() {
    let pm = PermissionManager::in_memory(SecurityRules::default(), Box::new(AlwaysDeny)).unwrap();
    let router = router_with(pm);

    let mut args = ToolArgs::new();
    args.insert(
        "id".into(),
        serde_json::json!(common::Id::new().to_string()),
    );
    let result = router.execute("forget_document", args).await;
    assert_eq!(result.status, ResultStatus::PermissionDenied);
}

#[tokio::test]
async fn indexing_then_forgetting_the_same_document_works_end_to_end_when_approved() {
    let pm = PermissionManager::in_memory(
        SecurityRules::default(),
        Box::new(AlwaysApproveForTestingOnly),
    )
    .unwrap();
    let router = router_with(pm);
    let path = real_text_file();

    let mut index_args = ToolArgs::new();
    index_args.insert("path".into(), serde_json::json!(path.to_str().unwrap()));
    let index_result = router.execute("index_document", index_args).await;
    assert_eq!(index_result.status, ResultStatus::Ok);
    let id = index_result.data.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let mut forget_args = ToolArgs::new();
    forget_args.insert("id".into(), serde_json::json!(id));
    let forget_result = router.execute("forget_document", forget_args).await;
    assert_eq!(forget_result.status, ResultStatus::Ok);
    assert_eq!(forget_result.data.unwrap()["forgotten"], true);

    std::fs::remove_dir_all(path.parent().unwrap()).ok();
}
