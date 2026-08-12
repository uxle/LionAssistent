//! Benchmark for `Cache`/`SharedCache` (Development_plan Phase 19
//! "Benchmarks"). No `criterion` -- see `vector-db/benches/search.rs`'s
//! doc comment for why (the same workspace-wide `half`/rustc-version
//! wall applies here too, confirmed the same way).
//!
//! The interesting question for `Cache` specifically is what
//! `evict_least_recently_used`'s brute-force linear scan (the crate's
//! own doc comment: "a linear scan is fast enough... at the scale this
//! workspace's own caches actually run") actually costs as capacity
//! grows -- this benchmark is the empirical check behind that claim, not
//! just the claim itself.
//!
//! Run with `cargo bench -p cache`.

use cache::{Cache, CacheConfig, SharedCache};
use std::time::{Duration, Instant};

const WARMUP_ITERS: usize = 5;
const TIMED_ITERS: usize = 100;

struct Stats {
    min: Duration,
    median: Duration,
    mean: Duration,
}

fn time_it(mut f: impl FnMut()) -> Stats {
    for _ in 0..WARMUP_ITERS {
        f();
    }
    let mut samples = Vec::with_capacity(TIMED_ITERS);
    for _ in 0..TIMED_ITERS {
        let start = Instant::now();
        f();
        samples.push(start.elapsed());
    }
    samples.sort();
    let total: Duration = samples.iter().sum();
    Stats {
        min: samples[0],
        median: samples[samples.len() / 2],
        mean: total / samples.len() as u32,
    }
}

fn fmt_duration(d: Duration) -> String {
    let micros = d.as_secs_f64() * 1_000_000.0;
    if micros < 1000.0 {
        format!("{micros:.3}\u{b5}s")
    } else {
        format!("{:.3}ms", micros / 1000.0)
    }
}

fn print_row(label: &str, stats: &Stats) {
    println!(
        "  {label:<38} min {:>10}   median {:>10}   mean {:>10}",
        fmt_duration(stats.min),
        fmt_duration(stats.median),
        fmt_duration(stats.mean),
    );
}

/// Fills a cache to exactly its capacity with keys `0..capacity`, all
/// touched once via `get` in ascending order so there's a well-defined
/// least-recently-used key (`0`) for the next `put` to evict.
fn full_cache(capacity: usize) -> Cache<u32, u32> {
    let mut cache = Cache::new(CacheConfig::with_capacity(capacity));
    for k in 0..capacity as u32 {
        cache.put(k, k);
    }
    for k in 0..capacity as u32 {
        cache.get(&k);
    }
    cache
}

fn main() {
    println!("cache::Cache / SharedCache -- {TIMED_ITERS} timed iterations after {WARMUP_ITERS} warmup\n");

    println!("put() that triggers eviction, by cache capacity (brute-force LRU scan cost):");
    for capacity in [16usize, 128, 1_024, 8_192] {
        let mut cache = full_cache(capacity);
        let mut next_key = capacity as u32;
        let stats = time_it(|| {
            cache.put(next_key, next_key);
            next_key += 1;
        });
        print_row(&format!("capacity = {capacity}"), &stats);
    }

    println!("\nget() hit vs. miss, at capacity = 1,024:");
    let mut cache = full_cache(1_024);
    let hit_stats = time_it(|| {
        std::hint::black_box(cache.get(std::hint::black_box(&512)));
    });
    print_row("hit", &hit_stats);
    let miss_stats = time_it(|| {
        std::hint::black_box(cache.get(std::hint::black_box(&999_999)));
    });
    print_row("miss", &miss_stats);

    println!(
        "\nCache vs. SharedCache (Mutex overhead), single-threaded get() hit at capacity = 1,024:"
    );
    let bare = full_cache(1_024);
    let mut bare = bare;
    let bare_stats = time_it(|| {
        std::hint::black_box(bare.get(std::hint::black_box(&512)));
    });
    print_row("Cache (bare)", &bare_stats);

    let shared: SharedCache<u32, u32> = SharedCache::new(CacheConfig::with_capacity(1_024));
    for k in 0..1024u32 {
        shared.put(k, k);
    }
    let shared_stats = time_it(|| {
        std::hint::black_box(shared.get(std::hint::black_box(&512)));
    });
    print_row("SharedCache (Arc<Mutex<_>>)", &shared_stats);
}
