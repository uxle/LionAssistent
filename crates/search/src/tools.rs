//! Wires [`SearchEngine`] up as real [`tool_framework::Tool`]
//! implementations -- finally giving `ToolCategory::Search` (declared in
//! Phase 7, unused until now) actual tools, following the exact pattern
//! `linux_services::tools` established for Phase 8's capabilities.
//!
//! All four are `PermissionLevel::ReadOnly`: nothing here modifies
//! files, memories, tools, or workflows, only reads them (TCLIS SS6
//! Level 0 examples -- "CPU, RAM, weather, system info" -- apply exactly
//! as well to "what files/memories/commands match this query").

use crate::engine::SearchEngine;
use crate::types::SearchHit;
use std::sync::Arc;
use std::time::Duration;
use tool_framework::{
    InputSchema, ParamKind, ParamSchema, PermissionLevel, Tool, ToolArgs, ToolCategory, ToolOutcome,
};

const DEFAULT_LIMIT: u64 = 10;

fn err_outcome(e: impl std::fmt::Display) -> ToolOutcome {
    ToolOutcome {
        success: false,
        message: e.to_string(),
        data: None,
    }
}

fn limit_from(args: &ToolArgs) -> usize {
    args.get("limit")
        .and_then(|v| v.as_u64())
        .unwrap_or(DEFAULT_LIMIT) as usize
}

fn require_query(args: &ToolArgs) -> Result<&str, ToolOutcome> {
    args.get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| err_outcome("missing required parameter \"query\""))
}

fn hits_to_outcome(label: &str, query: &str, hits: Vec<SearchHit>) -> ToolOutcome {
    let data = hits
        .iter()
        .map(|h| {
            serde_json::json!({
                "kind": h.kind.as_str(),
                "title": h.title,
                "snippet": h.snippet,
                "score": h.score,
                "source_id": h.source_id,
            })
        })
        .collect::<Vec<_>>();
    ToolOutcome {
        success: true,
        message: format!("{} result(s) for {query:?} ({label})", hits.len()),
        data: Some(serde_json::json!(data)),
    }
}

fn search_input_schema() -> InputSchema {
    InputSchema::new(vec![
        ParamSchema::required("query", ParamKind::String),
        ParamSchema::optional("limit", ParamKind::Integer),
    ])
}

pub struct SearchFilesTool(pub Arc<SearchEngine>);

impl Tool for SearchFilesTool {
    fn name(&self) -> &str {
        "search_files"
    }
    fn description(&self) -> &str {
        "Searches file names under the configured root directories."
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Search
    }
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
    fn timeout(&self) -> Duration {
        Duration::from_secs(10)
    }
    fn input_schema(&self) -> InputSchema {
        search_input_schema()
    }
    fn execute(&self, args: &ToolArgs) -> ToolOutcome {
        let query = match require_query(args) {
            Ok(q) => q,
            Err(outcome) => return outcome,
        };
        match self.0.search_files(query, limit_from(args)) {
            Ok(hits) => hits_to_outcome("files", query, hits),
            Err(e) => err_outcome(e),
        }
    }
}

pub struct SearchMemoryTool(pub Arc<SearchEngine>);

impl Tool for SearchMemoryTool {
    fn name(&self) -> &str {
        "search_memory"
    }
    fn description(&self) -> &str {
        "Searches saved long-term memories by meaning, not just exact words."
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Search
    }
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
    fn timeout(&self) -> Duration {
        Duration::from_secs(10)
    }
    fn input_schema(&self) -> InputSchema {
        search_input_schema()
    }
    fn execute(&self, args: &ToolArgs) -> ToolOutcome {
        let query = match require_query(args) {
            Ok(q) => q,
            Err(outcome) => return outcome,
        };
        match self.0.search_memory(query, limit_from(args)) {
            Ok(hits) => hits_to_outcome("memory", query, hits),
            Err(e) => err_outcome(e),
        }
    }
}

pub struct SearchCommandsTool(pub Arc<SearchEngine>);

impl Tool for SearchCommandsTool {
    fn name(&self) -> &str {
        "search_commands"
    }
    fn description(&self) -> &str {
        "Searches available tools and saved workflows by name or description."
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Search
    }
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
    fn timeout(&self) -> Duration {
        Duration::from_secs(10)
    }
    fn input_schema(&self) -> InputSchema {
        search_input_schema()
    }
    fn execute(&self, args: &ToolArgs) -> ToolOutcome {
        let query = match require_query(args) {
            Ok(q) => q,
            Err(outcome) => return outcome,
        };
        match self.0.search_commands(query, limit_from(args)) {
            Ok(hits) => hits_to_outcome("commands", query, hits),
            Err(e) => err_outcome(e),
        }
    }
}

pub struct GlobalSearchTool(pub Arc<SearchEngine>);

impl Tool for GlobalSearchTool {
    fn name(&self) -> &str {
        "global_search"
    }
    fn description(&self) -> &str {
        "Searches files, memories, and commands together, ranked into one list."
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Search
    }
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
    fn timeout(&self) -> Duration {
        // Roughly the sum of the three individual tools' budgets: this
        // one calls all three providers in sequence within a single
        // `execute`, not in parallel (`Tool::execute` is synchronous --
        // see tool-framework's doc comment on why).
        Duration::from_secs(20)
    }
    fn input_schema(&self) -> InputSchema {
        search_input_schema()
    }
    fn execute(&self, args: &ToolArgs) -> ToolOutcome {
        let query = match require_query(args) {
            Ok(q) => q,
            Err(outcome) => return outcome,
        };
        match self.0.search_all(query, limit_from(args)) {
            Ok(hits) => hits_to_outcome("all", query, hits),
            Err(e) => err_outcome(e),
        }
    }
}

/// Registers every search tool, for `app` to call once at startup
/// alongside `linux_services::tools::register_all` -- not done in this
/// phase, since `app` still can't be compile-verified in this sandbox
/// (Phase 2's UI toolchain gap; see `docs/ROADMAP.md`), the same reason
/// Phases 8-12 each left it untouched too.
pub fn register_all(
    registry: &mut tool_framework::ToolRegistry,
    engine: Arc<SearchEngine>,
) -> common::AppResult<()> {
    registry.register(Arc::new(SearchFilesTool(engine.clone())))?;
    registry.register(Arc::new(SearchMemoryTool(engine.clone())))?;
    registry.register(Arc::new(SearchCommandsTool(engine.clone())))?;
    registry.register(Arc::new(GlobalSearchTool(engine)))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command_search::CommandSearchProvider;
    use crate::file_search::FileSearchProvider;
    use crate::memory_search::MemorySearchProvider;
    use embeddings::HashingEmbedder;
    use memory::{MemoryCategory, MemoryStore};
    use std::sync::Mutex;
    use tool_framework::{ToolRegistry, ToolRouter};

    fn test_engine(dir: &std::path::Path) -> Arc<SearchEngine> {
        std::fs::write(dir.join("findme.txt"), "").unwrap();

        let memory_store = MemoryStore::in_memory(Box::<HashingEmbedder>::default()).unwrap();
        memory_store
            .remember(MemoryCategory::Knowledge, "findme", "a findable memory", 5)
            .unwrap();

        let command_registry = ToolRegistry::new();
        Arc::new(SearchEngine::new(
            FileSearchProvider::new(vec![dir.to_path_buf()]),
            MemorySearchProvider::new(Arc::new(Mutex::new(memory_store))),
            CommandSearchProvider::new(&command_registry, None).unwrap(),
        ))
    }

    fn router(dir: &std::path::Path) -> ToolRouter {
        let mut registry = ToolRegistry::new();
        register_all(&mut registry, test_engine(dir)).unwrap();
        ToolRouter::new(registry, PermissionLevel::ReadOnly)
    }

    #[tokio::test]
    async fn search_files_tool_runs_through_the_real_pipeline() {
        let dir = std::env::temp_dir().join("search-tools-test-files");
        std::fs::create_dir_all(&dir).unwrap();

        let mut args = ToolArgs::new();
        args.insert("query".into(), serde_json::json!("findme"));
        let result = router(&dir).execute("search_files", args).await;
        assert!(result.success, "{}", result.message);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn search_memory_tool_runs_through_the_real_pipeline() {
        let dir = std::env::temp_dir().join("search-tools-test-memory");
        std::fs::create_dir_all(&dir).unwrap();

        let mut args = ToolArgs::new();
        args.insert("query".into(), serde_json::json!("findable memory"));
        let result = router(&dir).execute("search_memory", args).await;
        assert!(result.success, "{}", result.message);
        let data = result.data.unwrap();
        assert!(data
            .as_array()
            .unwrap()
            .iter()
            .any(|h| h["title"] == "findme"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn global_search_tool_missing_query_fails_validation() {
        let dir = std::env::temp_dir().join("search-tools-test-validation");
        std::fs::create_dir_all(&dir).unwrap();

        let result = router(&dir).execute("global_search", ToolArgs::new()).await;
        assert!(!result.success);
        assert_eq!(
            result.status,
            tool_framework::ResultStatus::ValidationFailed
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn global_search_tool_merges_across_providers() {
        let dir = std::env::temp_dir().join("search-tools-test-global");
        std::fs::create_dir_all(&dir).unwrap();

        let mut args = ToolArgs::new();
        args.insert("query".into(), serde_json::json!("findme"));
        let result = router(&dir).execute("global_search", args).await;
        assert!(result.success, "{}", result.message);
        let kinds: std::collections::HashSet<String> = result
            .data
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|h| h["kind"].as_str().unwrap().to_string())
            .collect();
        assert!(kinds.contains("file"));
        assert!(kinds.contains("memory"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn a_custom_limit_is_honored() {
        let dir = std::env::temp_dir().join("search-tools-test-limit");
        std::fs::create_dir_all(&dir).unwrap();
        for i in 0..5 {
            std::fs::write(dir.join(format!("dup-{i}.txt")), "").unwrap();
        }

        let mut args = ToolArgs::new();
        args.insert("query".into(), serde_json::json!("dup"));
        args.insert("limit".into(), serde_json::json!(2));
        let result = router(&dir).execute("search_files", args).await;
        assert!(result.success, "{}", result.message);
        assert_eq!(result.data.unwrap().as_array().unwrap().len(), 2);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn all_search_tools_are_registered() {
        let dir = std::env::temp_dir().join("search-tools-test-registered");
        std::fs::create_dir_all(&dir).unwrap();

        assert_eq!(
            router(&dir).registry().list(),
            vec![
                "global_search",
                "search_commands",
                "search_files",
                "search_memory",
            ]
        );

        std::fs::remove_dir_all(&dir).ok();
    }
}
