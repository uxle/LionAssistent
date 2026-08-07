//! "Command Search" (Development_plan Phase 13): search over things the
//! assistant can actually *do* -- registered `tool_framework::Tool`s and
//! saved, enabled `workflow_engine::Workflow`s.
//!
//! This builds a snapshot index up front via [`CommandSearchProvider::new`]
//! and re-scans on explicit [`CommandSearchProvider::reindex`] calls,
//! rather than querying `ToolRegistry`/`WorkflowStore` live on every
//! search. Two reasons, not one: neither is internally thread-safe for
//! arbitrary concurrent sharing the way this crate would need (see
//! `crate::memory_search`'s doc comment on the identical constraint for
//! `MemoryStore`), and unlike memories, tools and workflows change
//! rarely -- only at startup or when a plugin loads/unloads (PRD SS6.9)
//! or a workflow is saved -- so a snapshot rebuilt on demand is the
//! simpler, still-correct design here.

use crate::provider::SearchProvider;
use crate::ranking::score_text;
use crate::types::{SearchHit, SearchKind};
use common::AppResult;
use std::sync::RwLock;
use tool_framework::ToolRegistry;
use workflow_engine::WorkflowStore;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommandSource {
    Tool,
    Workflow,
}

#[derive(Debug, Clone)]
struct CommandEntry {
    name: String,
    description: String,
    /// Debug-formatted `ToolCategory` for `Tool`-sourced entries (e.g.
    /// `"FileManagement"`); unused (empty) for `Workflow`-sourced ones,
    /// which report `source` as `"Workflow"` instead -- see `search`'s
    /// `source_label`.
    category: String,
    source: CommandSource,
}

pub struct CommandSearchProvider {
    entries: RwLock<Vec<CommandEntry>>,
}

impl CommandSearchProvider {
    /// Builds the initial index. `workflows` is optional -- a caller
    /// without a `WorkflowStore` handy (a unit test, say, or a run with
    /// workflows disabled) still gets a working tool-only command search
    /// rather than being forced to construct one just to skip it.
    pub fn new(registry: &ToolRegistry, workflows: Option<&WorkflowStore>) -> AppResult<Self> {
        let provider = Self {
            entries: RwLock::new(Vec::new()),
        };
        provider.reindex(registry, workflows)?;
        Ok(provider)
    }

    /// Rebuilds the index from scratch. Call again after tools or
    /// workflows change at runtime to keep results current; nothing here
    /// does that automatically, since this crate has no way to observe
    /// those changes on its own -- Archd SS9's event bus would be the
    /// natural trigger once search is wired into `assistant-core`, which
    /// is itself still pending the same `llm-runtime`/UI blockers
    /// `tool-framework` and `linux-services` already documented (see
    /// this crate's top-level docs).
    pub fn reindex(
        &self,
        registry: &ToolRegistry,
        workflows: Option<&WorkflowStore>,
    ) -> AppResult<()> {
        let mut entries = Vec::new();

        for name in registry.list() {
            if let Some(tool) = registry.get(name) {
                entries.push(CommandEntry {
                    name: tool.name().to_string(),
                    description: tool.description().to_string(),
                    category: format!("{:?}", tool.category()),
                    source: CommandSource::Tool,
                });
            }
        }

        if let Some(store) = workflows {
            for (id, _name) in store.list_workflows()? {
                if let Some(workflow) = store.load_workflow(id)? {
                    if workflow.enabled {
                        entries.push(CommandEntry {
                            name: workflow.name,
                            description: workflow.description,
                            category: String::new(),
                            source: CommandSource::Workflow,
                        });
                    }
                }
            }
        }

        *self
            .entries
            .write()
            .expect("command search index lock poisoned") = entries;
        Ok(())
    }
}

impl SearchProvider for CommandSearchProvider {
    fn search(&self, query: &str, limit: usize) -> AppResult<Vec<SearchHit>> {
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }

        let entries = self
            .entries
            .read()
            .expect("command search index lock poisoned");

        let mut hits: Vec<SearchHit> = entries
            .iter()
            .filter_map(|entry| {
                let haystack = format!("{} {}", entry.name, entry.description);
                let score = score_text(query, &haystack);
                if score <= 0.0 {
                    return None;
                }
                let source_label = match entry.source {
                    CommandSource::Tool => entry.category.as_str(),
                    CommandSource::Workflow => "Workflow",
                };
                Some(SearchHit {
                    kind: SearchKind::Command,
                    title: entry.name.clone(),
                    snippet: format!("{} ({source_label})", entry.description),
                    score,
                    source_id: entry.name.clone(),
                })
            })
            .collect();

        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.title.cmp(&b.title))
        });
        hits.truncate(limit);
        Ok(hits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tool_framework::stubs::EchoTool;

    fn registry_with_echo() -> ToolRegistry {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(EchoTool)).unwrap();
        registry
    }

    #[test]
    fn finds_a_tool_by_name() {
        let provider = CommandSearchProvider::new(&registry_with_echo(), None).unwrap();
        let hits = provider.search("echo", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].title, "echo");
        assert_eq!(hits[0].kind, SearchKind::Command);
    }

    #[test]
    fn finds_a_tool_by_a_word_in_its_description() {
        let provider = CommandSearchProvider::new(&registry_with_echo(), None).unwrap();
        // EchoTool's description mentions "echoes" / "back" -- search on
        // a word likely to appear in a description, not the tool name.
        let hits = provider.search("back", 10).unwrap();
        assert!(hits.iter().any(|h| h.title == "echo"), "{hits:?}");
    }

    #[test]
    fn without_a_workflow_store_only_tools_are_indexed() {
        let provider = CommandSearchProvider::new(&registry_with_echo(), None).unwrap();
        let hits = provider.search("echo", 10).unwrap();
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn includes_enabled_workflows_when_a_store_is_given() {
        // An empty tool registry, deliberately: this test is about
        // whether the workflow shows up at all, and coupling it to
        // whatever EchoTool's description happens to say (long enough
        // text can accidentally satisfy the subsequence fallback for
        // almost any short query) would make it fragile for reasons
        // unrelated to what it's actually checking.
        let store = WorkflowStore::in_memory().unwrap();
        let wf = workflow_engine::Workflow::new("nightly-backup", "backs up the project nightly");
        store.save_workflow(&wf).unwrap();

        let provider = CommandSearchProvider::new(&ToolRegistry::new(), Some(&store)).unwrap();
        let hits = provider.search("backup", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].title, "nightly-backup");
    }

    #[test]
    fn excludes_disabled_workflows() {
        let store = WorkflowStore::in_memory().unwrap();
        let mut wf = workflow_engine::Workflow::new("archived-job", "an old disabled workflow");
        wf.enabled = false;
        store.save_workflow(&wf).unwrap();

        let provider = CommandSearchProvider::new(&ToolRegistry::new(), Some(&store)).unwrap();
        let hits = provider.search("archived", 10).unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn reindex_picks_up_newly_registered_tools() {
        let mut registry = ToolRegistry::new();
        let provider = CommandSearchProvider::new(&registry, None).unwrap();
        assert!(provider.search("echo", 10).unwrap().is_empty());

        registry.register(Arc::new(EchoTool)).unwrap();
        provider.reindex(&registry, None).unwrap();
        assert_eq!(provider.search("echo", 10).unwrap().len(), 1);
    }

    #[test]
    fn respects_the_limit() {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(EchoTool)).unwrap();
        registry
            .register(Arc::new(tool_framework::stubs::SlowTool))
            .unwrap();
        let provider = CommandSearchProvider::new(&registry, None).unwrap();
        // Both stub tools' names/descriptions are short enough that a
        // broad single-letter-ish query could match both; assert the
        // limit is honored regardless of how many actually match.
        let hits = provider.search("o", 1).unwrap();
        assert!(hits.len() <= 1);
    }
}
