//! `SearchEngine` -- the "Global Search" module (Development_plan Phase
//! 13): one entry point over [`FileSearchProvider`], [`MemorySearchProvider`],
//! and [`CommandSearchProvider`], merging their results into a single
//! ranked list.
//!
//! "Search settings" (TCLIS SS4) is deliberately not a fourth provider
//! here: it would mean indexing the Settings UI's labels/keys, which
//! don't exist as real, inspectable data yet -- Phase 2's UI has its own
//! unresolved sandbox constraint (`docs/ROADMAP.md`), and PRD SS6.7's
//! settings panel is still a specification, not code to search over.
//! Adding it once there's a real settings screen is a small, additive
//! change: one more [`SearchProvider`] impl, one more field here.

use crate::command_search::CommandSearchProvider;
use crate::file_search::FileSearchProvider;
use crate::memory_search::MemorySearchProvider;
use crate::provider::SearchProvider;
use crate::types::SearchHit;
use common::AppResult;

pub struct SearchEngine {
    pub file: FileSearchProvider,
    pub memory: MemorySearchProvider,
    pub command: CommandSearchProvider,
}

impl SearchEngine {
    pub fn new(
        file: FileSearchProvider,
        memory: MemorySearchProvider,
        command: CommandSearchProvider,
    ) -> Self {
        Self {
            file,
            memory,
            command,
        }
    }

    pub fn search_files(&self, query: &str, limit: usize) -> AppResult<Vec<SearchHit>> {
        self.file.search(query, limit)
    }

    pub fn search_memory(&self, query: &str, limit: usize) -> AppResult<Vec<SearchHit>> {
        self.memory.search(query, limit)
    }

    pub fn search_commands(&self, query: &str, limit: usize) -> AppResult<Vec<SearchHit>> {
        self.command.search(query, limit)
    }

    /// Queries every provider and merges the results into one list,
    /// highest score first (ties broken by kind, then title -- purely
    /// for deterministic output; TDS SS9 doesn't require a particular
    /// tie order, just a stable one). Each provider is asked for up to
    /// `limit` of its own results before merging, not `limit / 3`, so a
    /// query that's genuinely relevant to only one provider still gets a
    /// full page of results rather than being padded out with weaker
    /// matches from the other two.
    pub fn search_all(&self, query: &str, limit: usize) -> AppResult<Vec<SearchHit>> {
        let mut hits = Vec::new();
        hits.extend(self.file.search(query, limit)?);
        hits.extend(self.memory.search(query, limit)?);
        hits.extend(self.command.search(query, limit)?);

        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.kind.as_str().cmp(b.kind.as_str()))
                .then_with(|| a.title.cmp(&b.title))
        });
        hits.truncate(limit);
        Ok(hits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SearchKind;
    use embeddings::HashingEmbedder;
    use memory::{MemoryCategory, MemoryStore};
    use std::sync::{Arc, Mutex};
    use tool_framework::{stubs::EchoTool, ToolRegistry};

    fn engine_with_a_hit_in_every_provider(dir: &std::path::Path) -> SearchEngine {
        std::fs::write(dir.join("shared-term-notes.txt"), "").unwrap();

        let memory_store = MemoryStore::in_memory(Box::<HashingEmbedder>::default()).unwrap();
        memory_store
            .remember(
                MemoryCategory::Knowledge,
                "shared-term",
                "a memory that also mentions the shared term",
                5,
            )
            .unwrap();

        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(EchoTool)).unwrap();
        // EchoTool doesn't mention "shared-term" -- command search stays
        // empty for this query, proving search_all tolerates a provider
        // contributing zero hits rather than erroring.

        SearchEngine::new(
            FileSearchProvider::new(vec![dir.to_path_buf()]),
            MemorySearchProvider::new(Arc::new(Mutex::new(memory_store))),
            CommandSearchProvider::new(&registry, None).unwrap(),
        )
    }

    #[test]
    fn search_all_merges_hits_from_every_provider() {
        let dir = std::env::temp_dir().join("search-engine-test-merge");
        std::fs::create_dir_all(&dir).unwrap();
        let engine = engine_with_a_hit_in_every_provider(&dir);

        let hits = engine.search_all("shared-term", 10).unwrap();
        let kinds: std::collections::HashSet<SearchKind> = hits.iter().map(|h| h.kind).collect();
        assert!(kinds.contains(&SearchKind::File), "{hits:?}");
        assert!(kinds.contains(&SearchKind::Memory), "{hits:?}");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn search_all_is_sorted_best_first() {
        let dir = std::env::temp_dir().join("search-engine-test-sorted");
        std::fs::create_dir_all(&dir).unwrap();
        let engine = engine_with_a_hit_in_every_provider(&dir);

        let hits = engine.search_all("shared-term", 10).unwrap();
        for pair in hits.windows(2) {
            assert!(pair[0].score >= pair[1].score, "{hits:?}");
        }

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn search_all_respects_the_overall_limit() {
        let dir = std::env::temp_dir().join("search-engine-test-limit");
        std::fs::create_dir_all(&dir).unwrap();
        let engine = engine_with_a_hit_in_every_provider(&dir);

        let hits = engine.search_all("shared-term", 1).unwrap();
        assert_eq!(hits.len(), 1);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn per_kind_searches_only_return_their_own_kind() {
        let dir = std::env::temp_dir().join("search-engine-test-per-kind");
        std::fs::create_dir_all(&dir).unwrap();
        let engine = engine_with_a_hit_in_every_provider(&dir);

        assert!(engine
            .search_files("shared-term", 10)
            .unwrap()
            .iter()
            .all(|h| h.kind == SearchKind::File));
        assert!(engine
            .search_memory("shared-term", 10)
            .unwrap()
            .iter()
            .all(|h| h.kind == SearchKind::Memory));

        std::fs::remove_dir_all(&dir).ok();
    }
}
