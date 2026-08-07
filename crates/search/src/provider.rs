//! The common interface every search source implements, so
//! [`crate::engine::SearchEngine`] can query and rank them uniformly
//! (Development_plan Phase 13 "Global Search").

use crate::types::SearchHit;
use common::AppResult;

pub trait SearchProvider: Send + Sync {
    /// Returns up to `limit` hits for `query`, already sorted best-first
    /// by this provider's own notion of relevance. An empty (or
    /// whitespace-only) `query` returns an empty result, not every
    /// entry -- Phase 13 is search, not a full listing (`list_directory`/
    /// `list_memories`/`list_tools` already cover that, TCLIS SS4/Apis
    /// SS6-7).
    fn search(&self, query: &str, limit: usize) -> AppResult<Vec<SearchHit>>;
}
