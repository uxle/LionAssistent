//! [`CachingEmbedder`] -- memoizes any [`Embedder`]'s output
//! (Development_plan Phase 18 "Caching"). Worth having even for the
//! already-fast, dependency-free [`crate::HashingEmbedder`]:
//! `memory::MemoryStore::search_memory` re-embeds its query string on
//! every call, and a user re-running the same or a similar search
//! shortly after -- or `remember`ing text whose title and content
//! happen to overlap with something embedded moments earlier -- is an
//! entirely ordinary thing to do, not a contrived edge case.

use crate::Embedder;
use cache::{CacheConfig, CacheStats, SharedCache};

/// Wraps any `Embedder`, caching `embed()` results by exact input text.
/// Cloning is cheap and shares the same underlying cache (`SharedCache`
/// is itself an `Arc` handle), matching `event_bus::EventBus`'s own
/// cheap-clone-shares-state convention.
#[derive(Clone)]
pub struct CachingEmbedder<E> {
    inner: E,
    cache: SharedCache<String, Vec<f32>>,
}

impl<E: Embedder> CachingEmbedder<E> {
    /// Wraps `inner` with a default-sized (256-entry, no TTL) cache.
    pub fn new(inner: E) -> Self {
        Self::with_config(inner, CacheConfig::default())
    }

    /// Wraps `inner` with an explicitly configured cache -- e.g. a
    /// tighter capacity, or a TTL so memoized vectors don't outlive
    /// changes to whatever's producing them (`config::PerformanceConfig`
    /// carries user-tunable defaults for this).
    pub fn with_config(inner: E, config: CacheConfig) -> Self {
        Self {
            inner,
            cache: SharedCache::new(config),
        }
    }

    pub fn cache_stats(&self) -> CacheStats {
        self.cache.stats()
    }

    /// Drops every memoized vector without discarding cumulative
    /// [`CacheStats`] -- worth calling if the wrapped embedder's own
    /// behavior could have changed underneath it (e.g. a swapped
    /// model), so stale vectors don't linger for whatever's left of the
    /// cache's capacity/TTL window.
    pub fn clear_cache(&self) {
        self.cache.clear();
    }
}

impl<E: Embedder> Embedder for CachingEmbedder<E> {
    fn dimension(&self) -> usize {
        self.inner.dimension()
    }

    fn embed(&self, text: &str) -> Vec<f32> {
        if let Some(cached) = self.cache.get(&text.to_string()) {
            return cached;
        }
        let vector = self.inner.embed(text);
        self.cache.put(text.to_string(), vector.clone());
        vector
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::HashingEmbedder;

    #[test]
    fn produces_identical_output_to_the_unwrapped_embedder() {
        let plain = HashingEmbedder::default();
        let cached = CachingEmbedder::new(HashingEmbedder::default());
        assert_eq!(plain.embed("hello world"), cached.embed("hello world"));
    }

    #[test]
    fn dimension_delegates_to_the_inner_embedder() {
        let cached = CachingEmbedder::new(HashingEmbedder::new(64));
        assert_eq!(cached.dimension(), 64);
        assert_eq!(cached.embed("anything").len(), 64);
    }

    #[test]
    fn repeated_identical_text_is_a_cache_hit_on_the_second_call() {
        let cached = CachingEmbedder::new(HashingEmbedder::default());
        let first = cached.embed("the quick brown fox");
        let second = cached.embed("the quick brown fox");

        assert_eq!(first, second);
        let stats = cached.cache_stats();
        assert_eq!(stats.misses, 1, "first call should be a miss");
        assert_eq!(stats.hits, 1, "second identical call should be a hit");
    }

    #[test]
    fn distinct_text_is_a_separate_cache_entry() {
        let cached = CachingEmbedder::new(HashingEmbedder::default());
        cached.embed("first");
        cached.embed("second");

        let stats = cached.cache_stats();
        assert_eq!(stats.misses, 2);
        assert_eq!(stats.hits, 0);
    }

    #[test]
    fn clear_cache_forces_recomputation_with_identical_results() {
        let cached = CachingEmbedder::new(HashingEmbedder::default());
        let before = cached.embed("some text");
        cached.clear_cache();
        let after = cached.embed("some text");

        assert_eq!(
            before, after,
            "clearing the cache must not change what's returned"
        );
        assert_eq!(
            cached.cache_stats().misses,
            2,
            "recomputation after clear_cache should be a fresh miss"
        );
    }

    #[test]
    fn cache_capacity_is_respected_and_evicts_under_pressure() {
        let cached =
            CachingEmbedder::with_config(HashingEmbedder::default(), CacheConfig::with_capacity(4));
        for i in 0..20 {
            cached.embed(&format!("text number {i}"));
        }
        assert!(cached.cache_stats().evictions > 0);
    }

    #[test]
    fn cloning_shares_the_same_cache() {
        let a = CachingEmbedder::new(HashingEmbedder::default());
        let b = a.clone();

        a.embed("shared text");
        b.embed("shared text");

        assert_eq!(
            b.cache_stats().hits,
            1,
            "the clone should observe the original's cached entry as a hit"
        );
    }
}
