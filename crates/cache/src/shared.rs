//! [`SharedCache`] -- an `Arc<Mutex<Cache>>` wrapper for callers that
//! need to hand the same cache to more than one owner or thread/task.
//! `Cache` itself has no internal locking (same reasoning
//! `memory::MemoryStore` documents for staying unshared and letting
//! callers like `search::MemorySearchProvider` wrap it themselves).

use crate::lru::{Cache, CacheConfig, CacheStats};
use std::hash::Hash;
use std::sync::{Arc, Mutex};

/// A cheap-to-clone, thread-safe handle to a shared [`Cache`]. Cloning a
/// `SharedCache` clones the `Arc`, not the cache -- every clone reads
/// and writes the same underlying storage, the same sharing model
/// `event_bus::EventBus` already uses for its own handle type.
pub struct SharedCache<K, V> {
    inner: Arc<Mutex<Cache<K, V>>>,
}

impl<K, V> SharedCache<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    pub fn new(config: CacheConfig) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Cache::new(config))),
        }
    }

    pub fn get(&self, key: &K) -> Option<V> {
        self.inner.lock().expect("cache lock poisoned").get(key)
    }

    pub fn put(&self, key: K, value: V) {
        self.inner
            .lock()
            .expect("cache lock poisoned")
            .put(key, value);
    }

    pub fn remove(&self, key: &K) -> Option<V> {
        self.inner.lock().expect("cache lock poisoned").remove(key)
    }

    pub fn contains_key(&self, key: &K) -> bool {
        self.inner
            .lock()
            .expect("cache lock poisoned")
            .contains_key(key)
    }

    pub fn clear(&self) {
        self.inner.lock().expect("cache lock poisoned").clear();
    }

    pub fn len(&self) -> usize {
        self.inner.lock().expect("cache lock poisoned").len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.lock().expect("cache lock poisoned").is_empty()
    }

    pub fn stats(&self) -> CacheStats {
        self.inner.lock().expect("cache lock poisoned").stats()
    }

    pub fn reset_stats(&self) {
        self.inner
            .lock()
            .expect("cache lock poisoned")
            .reset_stats();
    }
}

impl<K, V> Clone for SharedCache<K, V> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn shared_cache_roundtrips() {
        let c: SharedCache<&str, i32> = SharedCache::new(CacheConfig::with_capacity(4));
        c.put("a", 1);
        assert_eq!(c.get(&"a"), Some(1));
    }

    #[test]
    fn clones_share_the_same_underlying_storage() {
        let a: SharedCache<&str, i32> = SharedCache::new(CacheConfig::with_capacity(4));
        let b = a.clone();

        a.put("x", 42);
        assert_eq!(
            b.get(&"x"),
            Some(42),
            "a clone should see writes made through the original handle"
        );

        b.put("y", 7);
        assert_eq!(a.get(&"y"), Some(7));
        assert_eq!(a.len(), 2);
    }

    #[test]
    fn concurrent_access_from_many_threads_does_not_lose_data() {
        let cache: SharedCache<u32, u32> = SharedCache::new(CacheConfig::with_capacity(64));
        let handles: Vec<_> = (0..8)
            .map(|t| {
                let cache = cache.clone();
                thread::spawn(move || {
                    for i in 0..8 {
                        let key = t * 8 + i;
                        cache.put(key, key * 10);
                    }
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(cache.len(), 64, "capacity comfortably fits all 64 keys");
        for key in 0..64u32 {
            assert_eq!(cache.get(&key), Some(key * 10));
        }
    }

    #[test]
    fn concurrent_puts_beyond_capacity_stay_internally_consistent() {
        // A tighter capacity than the number of distinct keys means real
        // eviction happens concurrently -- this test isn't about which
        // specific keys survive (that's covered single-threaded above),
        // just that the cache never panics, deadlocks, or ends up over
        // capacity under concurrent pressure.
        let cache: SharedCache<u32, u32> = SharedCache::new(CacheConfig::with_capacity(8));
        let handles: Vec<_> = (0..8)
            .map(|t| {
                let cache = cache.clone();
                thread::spawn(move || {
                    for i in 0..50 {
                        cache.put(t * 50 + i, i);
                    }
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }

        assert!(cache.len() <= 8, "cache must never exceed its capacity");
        assert!(cache.stats().evictions > 0);
    }

    #[test]
    fn reset_stats_and_clear_are_visible_across_clones() {
        let a: SharedCache<&str, i32> = SharedCache::new(CacheConfig::with_capacity(4));
        let b = a.clone();
        a.put("a", 1);
        b.get(&"a");

        assert_eq!(a.stats().hits, 1);
        b.clear();
        assert!(a.is_empty());
        a.reset_stats();
        assert_eq!(b.stats().hits, 0);
    }

    #[test]
    fn ttl_still_applies_through_the_shared_wrapper() {
        let c: SharedCache<&str, i32> =
            SharedCache::new(CacheConfig::with_capacity(4).with_ttl(Duration::from_millis(20)));
        c.put("a", 1);
        thread::sleep(Duration::from_millis(80));
        assert_eq!(c.get(&"a"), None);
    }
}
