//! "Memory Search" (Development_plan Phase 13), over Phase 5's
//! `memory::MemoryStore`.
//!
//! `MemoryStore` is `Send` but not `Sync` -- its SQLite `Connection` and
//! `RefCell`-cached vector index aren't safe to share across threads
//! without help. `tool_framework::Tool` requires `Send + Sync` (the
//! router executes tools via `spawn_blocking`), so a tool wrapping
//! `MemoryStore` needs that help from somewhere. `permission-manager`
//! hit the identical constraint for its own `Connection` and solved it
//! by wrapping it in a `Mutex`; `Arc<Mutex<MemoryStore>>` here is that
//! same fix applied one level up, since this crate doesn't own
//! `MemoryStore`'s internals to change directly.

use crate::provider::SearchProvider;
use crate::types::{SearchHit, SearchKind};
use common::{AppError, AppResult};
use memory::MemoryStore;
use std::sync::{Arc, Mutex};

pub struct MemorySearchProvider {
    store: Arc<Mutex<MemoryStore>>,
}

impl MemorySearchProvider {
    pub fn new(store: Arc<Mutex<MemoryStore>>) -> Self {
        Self { store }
    }
}

impl SearchProvider for MemorySearchProvider {
    /// Ranked by `MemoryStore::search_memory`'s own semantic similarity,
    /// best first. That similarity value itself isn't part of `memory`'s
    /// public API (Apis SS6 `search_memory` returns items, not scores),
    /// so hits here get a score derived from rank position instead -- a
    /// smooth, strictly decreasing function, never zero -- rather than a
    /// number pretending to be directly comparable to a file/command
    /// match's real text-overlap score. That's honestly approximate
    /// (see `SearchHit::score`'s doc comment), but keeps
    /// `SearchEngine::search_all` able to interleave memory hits with
    /// the other two by relative position without overstating precision
    /// this crate doesn't have.
    fn search(&self, query: &str, limit: usize) -> AppResult<Vec<SearchHit>> {
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }

        let items = {
            let store = self.store.lock().map_err(|_| AppError::Module {
                module: "search::memory_search",
                message: "memory store lock poisoned".to_string(),
            })?;
            store.search_memory(query, limit)?
        };

        Ok(items
            .into_iter()
            .enumerate()
            .map(|(rank, item)| SearchHit {
                kind: SearchKind::Memory,
                title: item.title,
                snippet: item.content,
                score: rank_score(rank),
                source_id: item.id.to_string(),
            })
            .collect())
    }
}

/// Strictly decreasing, starts at `1.0`, asymptotically approaches (but
/// never reaches) `0.0` -- see this module's doc comment on why rank
/// stands in for a real similarity score here.
fn rank_score(rank: usize) -> f32 {
    1.0 / (1.0 + rank as f32 * 0.5)
}

#[cfg(test)]
mod tests {
    use super::*;
    use embeddings::HashingEmbedder;
    use memory::MemoryCategory;

    fn provider_with_memories() -> MemorySearchProvider {
        let store = MemoryStore::in_memory(Box::<HashingEmbedder>::default()).unwrap();
        store
            .remember(
                MemoryCategory::Preference,
                "editor",
                "the user prefers vim keybindings",
                5,
            )
            .unwrap();
        store
            .remember(
                MemoryCategory::Knowledge,
                "recipe",
                "grandmother's lasagna needs fresh basil",
                3,
            )
            .unwrap();
        MemorySearchProvider::new(Arc::new(Mutex::new(store)))
    }

    #[test]
    fn finds_the_semantically_closer_memory() {
        let provider = provider_with_memories();
        let hits = provider
            .search("what keybindings does the user like", 5)
            .unwrap();
        assert!(!hits.is_empty());
        assert_eq!(hits[0].title, "editor");
        assert_eq!(hits[0].kind, SearchKind::Memory);
    }

    #[test]
    fn scores_strictly_decrease_with_rank() {
        let provider = provider_with_memories();
        let hits = provider.search("keybindings recipe basil", 5).unwrap();
        for pair in hits.windows(2) {
            assert!(pair[0].score > pair[1].score);
        }
    }

    #[test]
    fn empty_query_returns_no_results() {
        let provider = provider_with_memories();
        assert!(provider.search("", 5).unwrap().is_empty());
    }

    #[test]
    fn source_id_round_trips_to_the_memory_id() {
        let provider = provider_with_memories();
        let hits = provider.search("vim keybindings", 1).unwrap();
        assert_eq!(hits.len(), 1);
        // Must parse back as a real Id -- a caller would use this to
        // jump straight to the memory (Archd SS4 "Memory Viewer").
        assert!(hits[0].source_id.parse::<common::Id>().is_ok());
    }
}
