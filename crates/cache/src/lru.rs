//! The single-threaded cache core: capacity-bounded LRU eviction plus an
//! optional per-entry TTL. See the crate doc comment for why eviction is
//! a deliberately simple linear scan rather than an intrusive
//! doubly-linked list.

use std::collections::HashMap;
use std::hash::Hash;
use std::time::{Duration, Instant};

/// Tunable knobs for a [`Cache`]. `capacity` is coerced to at least `1`
/// (a zero-capacity cache that can never store anything is almost
/// certainly a misconfiguration, not an intentional "always empty"
/// cache -- the same defensive-minimum reasoning `metrics::PerformanceMonitor::new`
/// already applies to its own `max_history`).
#[derive(Debug, Clone, Copy)]
pub struct CacheConfig {
    pub capacity: usize,
    /// `None` means entries never expire on their own (capacity-based
    /// eviction only).
    pub ttl: Option<Duration>,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            capacity: 256,
            ttl: None,
        }
    }
}

impl CacheConfig {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            ttl: None,
        }
    }

    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = Some(ttl);
        self
    }
}

/// Cumulative counters for one [`Cache`]'s lifetime, exposed so a caller
/// (or a future diagnostics tool, the same shape as
/// `diagnostics::get_diagnostics_report` already ties `metrics` and
/// `telemetry` together) can tell whether a cache is actually earning
/// its complexity. Not reset by [`Cache::clear`] -- clearing the stored
/// entries and resetting the usage counters are different operations,
/// matching `metrics::MetricsRegistry`'s own `reset()` being separate
/// from anything that merely reads it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub insertions: u64,
    pub evictions: u64,
    pub expirations: u64,
}

impl CacheStats {
    /// Hit rate in `[0.0, 1.0]`. `0.0`, not `NaN`, if nothing has been
    /// looked up yet.
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

struct Entry<V> {
    value: V,
    inserted_at: Instant,
    last_used: u64,
}

fn entry_expired<V>(ttl: Option<Duration>, entry: &Entry<V>) -> bool {
    match ttl {
        Some(ttl) => entry.inserted_at.elapsed() >= ttl,
        None => false,
    }
}

/// A capacity- and (optionally) TTL-bounded cache. Not thread-safe on
/// its own -- see [`crate::SharedCache`] for a `Send + Sync` wrapper.
pub struct Cache<K, V> {
    config: CacheConfig,
    entries: HashMap<K, Entry<V>>,
    /// Logical clock, incremented on every access; each entry remembers
    /// the tick it was last touched on, which is what eviction scans
    /// for the minimum of.
    clock: u64,
    stats: CacheStats,
}

impl<K, V> Cache<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    pub fn new(config: CacheConfig) -> Self {
        Self {
            config: CacheConfig {
                capacity: config.capacity.max(1),
                ttl: config.ttl,
            },
            entries: HashMap::new(),
            clock: 0,
            stats: CacheStats::default(),
        }
    }

    /// Looks up `key`. A present-but-expired entry is removed and
    /// counted as both an expiration and a miss. A hit refreshes the
    /// entry's recency, protecting it from the next capacity eviction.
    pub fn get(&mut self, key: &K) -> Option<V> {
        if let Some(entry) = self.entries.get(key) {
            if entry_expired(self.config.ttl, entry) {
                self.entries.remove(key);
                self.stats.expirations += 1;
                self.stats.misses += 1;
                return None;
            }
        } else {
            self.stats.misses += 1;
            return None;
        }

        self.clock += 1;
        let clock = self.clock;
        let entry = self
            .entries
            .get_mut(key)
            .expect("just confirmed present and unexpired above");
        entry.last_used = clock;
        self.stats.hits += 1;
        Some(entry.value.clone())
    }

    /// Returns whether `key` is present and unexpired, without affecting
    /// recency or hit/miss statistics -- for callers that want to check
    /// without counting as a real lookup.
    pub fn contains_key(&self, key: &K) -> bool {
        self.entries
            .get(key)
            .is_some_and(|e| !entry_expired(self.config.ttl, e))
    }

    /// Inserts or updates `key`. Updating an existing key refreshes both
    /// its value and its TTL clock (a `put` counts as a fresh use, the
    /// same as a `get`) and never counts as an eviction. A brand new key
    /// past capacity evicts the least-recently-used entry first.
    pub fn put(&mut self, key: K, value: V) {
        self.clock += 1;
        let clock = self.clock;

        if let Some(entry) = self.entries.get_mut(&key) {
            entry.value = value;
            entry.inserted_at = Instant::now();
            entry.last_used = clock;
            return;
        }

        if self.entries.len() >= self.config.capacity {
            self.evict_least_recently_used();
        }

        self.entries.insert(
            key,
            Entry {
                value,
                inserted_at: Instant::now(),
                last_used: clock,
            },
        );
        self.stats.insertions += 1;
    }

    /// Removes `key` if present, returning its value. Deliberately not
    /// counted in `stats` -- an explicit removal is a different thing
    /// from the cache's own capacity- or TTL-driven housekeeping, and
    /// conflating them would make `evictions`/`expirations` less useful
    /// for judging whether a cache is sized appropriately.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.entries.remove(key).map(|e| e.value)
    }

    /// Drops every entry. Usage statistics are untouched -- call
    /// [`Cache::reset_stats`] separately if that's also wanted.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn stats(&self) -> CacheStats {
        self.stats
    }

    pub fn reset_stats(&mut self) {
        self.stats = CacheStats::default();
    }

    fn evict_least_recently_used(&mut self) {
        if let Some(lru_key) = self
            .entries
            .iter()
            .min_by_key(|(_, entry)| entry.last_used)
            .map(|(k, _)| k.clone())
        {
            self.entries.remove(&lru_key);
            self.stats.evictions += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    fn cache<K: Eq + Hash + Clone, V: Clone>(capacity: usize) -> Cache<K, V> {
        Cache::new(CacheConfig::with_capacity(capacity))
    }

    #[test]
    fn a_new_cache_is_empty() {
        let c: Cache<String, i32> = cache(4);
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }

    #[test]
    fn put_then_get_roundtrips() {
        let mut c = cache(4);
        c.put("a", 1);
        assert_eq!(c.get(&"a"), Some(1));
    }

    #[test]
    fn get_on_a_missing_key_is_a_counted_miss() {
        let mut c: Cache<&str, i32> = cache(4);
        assert_eq!(c.get(&"nope"), None);
        assert_eq!(c.stats().misses, 1);
        assert_eq!(c.stats().hits, 0);
    }

    #[test]
    fn updating_an_existing_key_does_not_grow_len_or_count_as_insertion_twice() {
        let mut c = cache(4);
        c.put("a", 1);
        c.put("a", 2);
        assert_eq!(c.len(), 1);
        assert_eq!(c.get(&"a"), Some(2));
        assert_eq!(c.stats().insertions, 1);
    }

    #[test]
    fn capacity_eviction_removes_the_least_recently_used_entry() {
        let mut c = cache(2);
        c.put("a", 1);
        c.put("b", 2);
        c.put("c", 3); // over capacity -- "a" has never been touched since insert, evict it

        assert_eq!(c.len(), 2);
        assert_eq!(c.get(&"a"), None);
        assert_eq!(c.get(&"b"), Some(2));
        assert_eq!(c.get(&"c"), Some(3));
        assert_eq!(c.stats().evictions, 1);
    }

    #[test]
    fn get_refreshes_recency_and_protects_from_eviction() {
        let mut c = cache(2);
        c.put("a", 1);
        c.put("b", 2);
        c.get(&"a"); // "a" is now more recently used than "b"
        c.put("c", 3); // over capacity -- "b" is now the least recently used

        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.get(&"b"), None);
        assert_eq!(c.get(&"c"), Some(3));
    }

    #[test]
    fn remove_drops_the_entry_without_counting_as_an_eviction() {
        let mut c = cache(4);
        c.put("a", 1);
        assert_eq!(c.remove(&"a"), Some(1));
        assert!(c.is_empty());
        assert_eq!(c.stats().evictions, 0);
    }

    #[test]
    fn removing_a_missing_key_returns_none() {
        let mut c: Cache<&str, i32> = cache(4);
        assert_eq!(c.remove(&"nope"), None);
    }

    #[test]
    fn clear_empties_the_cache_but_keeps_cumulative_stats() {
        let mut c = cache(4);
        c.put("a", 1);
        c.get(&"a");
        c.clear();

        assert!(c.is_empty());
        assert_eq!(c.stats().hits, 1, "clear() should not reset stats");
    }

    #[test]
    fn reset_stats_zeroes_the_counters_without_touching_entries() {
        let mut c = cache(4);
        c.put("a", 1);
        c.get(&"a");
        c.reset_stats();

        assert_eq!(c.stats(), CacheStats::default());
        assert_eq!(c.len(), 1, "reset_stats() should not clear entries");
    }

    #[test]
    fn hit_rate_is_zero_not_nan_before_any_lookups() {
        let c: Cache<&str, i32> = cache(4);
        assert_eq!(c.stats().hit_rate(), 0.0);
    }

    #[test]
    fn hit_rate_reflects_hits_over_total_lookups() {
        let mut c = cache(4);
        c.put("a", 1);
        c.get(&"a"); // hit
        c.get(&"a"); // hit
        c.get(&"missing"); // miss
        assert!((c.stats().hit_rate() - (2.0 / 3.0)).abs() < 1e-9);
    }

    #[test]
    fn contains_key_does_not_affect_recency_or_stats() {
        let mut c = cache(2);
        c.put("a", 1);
        c.put("b", 2);
        assert!(c.contains_key(&"a"));
        assert_eq!(c.stats().hits, 0, "contains_key should not count as a hit");

        c.put("c", 3); // "a" was never *used* via get(), so it's still the LRU victim
        assert!(!c.contains_key(&"a"));
    }

    #[test]
    fn zero_capacity_is_coerced_to_at_least_one() {
        let mut c: Cache<&str, i32> = Cache::new(CacheConfig::with_capacity(0));
        c.put("a", 1);
        assert_eq!(c.get(&"a"), Some(1));
    }

    #[test]
    fn entries_never_expire_when_no_ttl_is_configured() {
        let mut c: Cache<&str, i32> = Cache::new(CacheConfig::with_capacity(4));
        c.put("a", 1);
        sleep(Duration::from_millis(20));
        assert_eq!(c.get(&"a"), Some(1));
    }

    #[test]
    fn an_entry_older_than_its_ttl_is_treated_as_a_miss_and_removed() {
        let mut c: Cache<&str, i32> =
            Cache::new(CacheConfig::with_capacity(4).with_ttl(Duration::from_millis(20)));
        c.put("a", 1);
        sleep(Duration::from_millis(80)); // generous margin over the 20ms TTL to avoid flakiness

        assert_eq!(c.get(&"a"), None);
        assert_eq!(c.stats().expirations, 1);
        assert!(c.is_empty(), "expired entry should have been removed");
    }

    #[test]
    fn an_entry_well_within_its_ttl_is_still_a_hit() {
        let mut c: Cache<&str, i32> =
            Cache::new(CacheConfig::with_capacity(4).with_ttl(Duration::from_secs(60)));
        c.put("a", 1);
        sleep(Duration::from_millis(10));
        assert_eq!(c.get(&"a"), Some(1));
    }

    #[test]
    fn updating_a_key_refreshes_its_ttl_clock() {
        let mut c: Cache<&str, i32> =
            Cache::new(CacheConfig::with_capacity(4).with_ttl(Duration::from_millis(60)));
        c.put("a", 1);
        sleep(Duration::from_millis(40));
        c.put("a", 2); // refreshes inserted_at
        sleep(Duration::from_millis(40)); // 80ms since first put, but only 40ms since the refresh

        assert_eq!(
            c.get(&"a"),
            Some(2),
            "a fresh put should restart the TTL window"
        );
    }
}
