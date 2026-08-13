//! Basic `Cache`/`SharedCache` usage. Run with:
//!
//!     cargo run -p cache --example basic_usage

use cache::{Cache, CacheConfig, SharedCache};
use std::time::Duration;

fn main() {
    // A plain, single-threaded Cache: capacity-bounded, no expiry.
    let mut cache: Cache<String, u64> = Cache::new(CacheConfig::with_capacity(3));
    cache.put("a".to_string(), 1);
    cache.put("b".to_string(), 2);
    cache.put("c".to_string(), 3);
    println!("a = {:?}", cache.get(&"a".to_string())); // hit, and now "a" is the most recently used

    // Over capacity: evicts the least-recently-used entry ("b", since
    // "a" was just touched above and "c" was inserted after it).
    cache.put("d".to_string(), 4);
    println!("b after eviction = {:?}", cache.get(&"b".to_string())); // None
    println!("stats = {:?}", cache.stats());

    // A cache with a TTL: entries expire on their own, not just under
    // capacity pressure.
    let mut ttl_cache: Cache<&str, &str> =
        Cache::new(CacheConfig::with_capacity(10).with_ttl(Duration::from_millis(50)));
    ttl_cache.put("session-token", "abc123");
    println!(
        "immediately: {:?}",
        ttl_cache.get(&"session-token") // Some("abc123")
    );
    std::thread::sleep(Duration::from_millis(80));
    println!(
        "after the TTL window: {:?}",
        ttl_cache.get(&"session-token") // None -- expired, not evicted for space
    );

    // SharedCache: the thread-safe handle used when more than one owner
    // needs the same cache -- e.g. embeddings::CachingEmbedder wraps one
    // of these internally.
    let shared: SharedCache<u32, String> = SharedCache::new(CacheConfig::with_capacity(100));
    let worker = {
        let shared = shared.clone(); // cheap: clones the Arc, not the cache
        std::thread::spawn(move || {
            shared.put(1, "written from another thread".to_string());
        })
    };
    worker.join().unwrap();
    println!("from the main thread: {:?}", shared.get(&1));
}
