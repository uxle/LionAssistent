//! Integration test: `search`'s tools running through `tool-framework`'s
//! router, gated by `permission-manager`'s real `PermissionManager` --
//! not the router's simple `max_auto_level` fallback. Mirrors
//! `linux-services/tests/permission_integration.rs`'s pattern for the
//! same reason: proving the crate providing the concrete `Tool` impls
//! genuinely works end to end with Phase 9's real permission pipeline,
//! not just against a bare `ToolRouter` in this crate's own unit tests.

use embeddings::HashingEmbedder;
use memory::{MemoryCategory, MemoryStore};
use permission_manager::{AlwaysDeny, PermissionManager, SecurityRules};
use search::{engine::SearchEngine, tools};
use search::{CommandSearchProvider, FileSearchProvider, MemorySearchProvider};
use std::sync::{Arc, Mutex};
use tool_framework::{ToolArgs, ToolRegistry, ToolRouter};

fn router_with_real_permission_manager(
    dir: &std::path::Path,
    pm: Arc<PermissionManager>,
) -> ToolRouter {
    std::fs::write(dir.join("findable.txt"), "").unwrap();

    let memory_store = MemoryStore::in_memory(Box::<HashingEmbedder>::default()).unwrap();
    memory_store
        .remember(MemoryCategory::Knowledge, "findable", "a findable fact", 5)
        .unwrap();

    let command_registry = ToolRegistry::new();
    let engine = Arc::new(SearchEngine::new(
        FileSearchProvider::new(vec![dir.to_path_buf()]),
        MemorySearchProvider::new(Arc::new(Mutex::new(memory_store))),
        CommandSearchProvider::new(&command_registry, None).unwrap(),
    ));

    let mut registry = ToolRegistry::new();
    tools::register_all(&mut registry, engine).unwrap();

    // max_auto_level is irrelevant here -- a real checker is configured,
    // so it takes over entirely (tool-framework's own test already
    // confirms a configured checker overrides this fallback).
    ToolRouter::new(registry, common::PermissionLevel::ReadOnly).with_permission_checker(pm)
}

fn query_args(query: &str) -> ToolArgs {
    let mut args = ToolArgs::new();
    args.insert("query".into(), serde_json::json!(query));
    args
}

#[tokio::test]
async fn read_only_search_tools_run_without_any_confirmation() {
    // AlwaysDeny would refuse if actually asked -- every search tool
    // being ReadOnly means it never should be.
    let dir = std::env::temp_dir().join("search-integration-readonly");
    std::fs::create_dir_all(&dir).unwrap();

    let pm = Arc::new(
        PermissionManager::in_memory(SecurityRules::default(), Box::new(AlwaysDeny)).unwrap(),
    );
    let router = router_with_real_permission_manager(&dir, pm);

    let result = router
        .execute("global_search", query_args("findable"))
        .await;
    assert!(result.success, "{}", result.message);

    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn every_search_tool_is_readonly_and_gets_through_a_deny_everything_policy() {
    // A stricter proof than the single global_search check above: run
    // every registered search tool through a checker that would refuse
    // anything above ReadOnly, confirming none of them were accidentally
    // registered at a higher permission level.
    let dir = std::env::temp_dir().join("search-integration-all-tools");
    std::fs::create_dir_all(&dir).unwrap();

    let pm = Arc::new(
        PermissionManager::in_memory(SecurityRules::default(), Box::new(AlwaysDeny)).unwrap(),
    );
    let router = router_with_real_permission_manager(&dir, pm);

    for tool_name in [
        "search_files",
        "search_memory",
        "search_commands",
        "global_search",
    ] {
        let result = router.execute(tool_name, query_args("findable")).await;
        assert!(result.success, "{tool_name}: {}", result.message);
    }

    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn every_search_tool_call_is_recorded_in_the_audit_log() {
    let dir = std::env::temp_dir().join("search-integration-audit");
    std::fs::create_dir_all(&dir).unwrap();

    let pm = Arc::new(
        PermissionManager::in_memory(SecurityRules::default(), Box::new(AlwaysDeny)).unwrap(),
    );
    // Keep our own handle on the same manager the router uses (`Arc`
    // clone, not a second manager) so the audit log we inspect below is
    // actually the one the router's checks were logged to.
    let router = router_with_real_permission_manager(&dir, pm.clone());

    let result = router.execute("search_files", query_args("findable")).await;
    assert!(result.success, "{}", result.message);

    let entries = pm.audit_log(10).unwrap();
    assert!(
        entries.iter().any(|e| e.message.contains("search_files")),
        "{entries:?}"
    );

    std::fs::remove_dir_all(&dir).ok();
}
