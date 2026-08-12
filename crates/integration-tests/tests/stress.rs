//! Stress tests: correctness (not just "doesn't panic") at a scale two
//! or three orders of magnitude past what any unit test in `cache`,
//! `vector-db`, `memory`, or `event-bus` uses on its own.

use std::sync::Arc;
use std::time::{Duration, Instant};

/// `vector_db`'s own 128-entry equivalence test (Phase 18) proves the
/// `select_nth_unstable_by` optimization matches a naive full-sort
/// reference. This goes an order of magnitude further -- 5,000 entries,
/// closer to "a long-lived user's real memory count" than a unit test's
/// scale -- and checks it through `memory::MemoryStore::search_memory`
/// end to end (embedding, index, Phase 18's search-result cache all
/// included), not just `VectorIndex::search` in isolation.
#[test]
fn memory_search_stays_correct_and_fast_with_thousands_of_entries() {
    let store =
        memory::MemoryStore::in_memory(Box::<embeddings::HashingEmbedder>::default()).unwrap();

    for i in 0..5_000 {
        store
            .remember(
                memory::MemoryCategory::Knowledge,
                &format!("fact {i}"),
                &format!("this is memory number {i} about topic {}", i % 37),
                3,
            )
            .unwrap();
    }
    // One memory that's a strong, distinctive match for a query nothing
    // else in the store resembles.
    let needle_id = store
        .remember(
            memory::MemoryCategory::Preference,
            "unique marker",
            "zzyzx quokka telemetry unicorn distinctive marker text",
            5,
        )
        .unwrap();

    let start = Instant::now();
    let hits = store
        .search_memory("zzyzx quokka telemetry unicorn distinctive marker text", 5)
        .unwrap();
    let elapsed = start.elapsed();

    assert!(!hits.is_empty());
    assert_eq!(
        hits[0].id, needle_id,
        "the one near-exact match must rank first out of 5,001 entries"
    );
    assert!(
        elapsed < Duration::from_secs(2),
        "search over 5,001 entries took {elapsed:?}, expected well under 2s"
    );
}

/// `cache`'s own concurrency tests (Phase 18) use 8 threads / 64 keys.
/// This uses 32 threads and 20,000 total operations, mixing puts, gets,
/// and removes from every thread at once against one shared, tightly
/// capacity-bound cache -- checking the invariant that actually matters
/// under real contention: capacity is never exceeded, no matter how
/// interleaved the operations are.
#[test]
fn shared_cache_holds_its_capacity_invariant_under_heavy_concurrent_mixed_load() {
    let shared: cache::SharedCache<u32, u32> =
        cache::SharedCache::new(cache::CacheConfig::with_capacity(100));

    let handles: Vec<_> = (0..32u32)
        .map(|t| {
            let shared = shared.clone();
            std::thread::spawn(move || {
                for i in 0..625u32 {
                    let key = (t * 625 + i) % 500; // heavy key overlap across threads, on purpose
                    match i % 3 {
                        0 => {
                            shared.put(key, key * 2);
                        }
                        1 => {
                            shared.get(&key);
                        }
                        _ => {
                            shared.remove(&key);
                        }
                    }
                }
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }

    assert!(
        shared.len() <= 100,
        "capacity invariant violated under concurrent load: len() = {}",
        shared.len()
    );
}

/// `event_bus`'s crate-local tests (Phase 1) cover a handful of
/// publishes to a handful of subscribers. This uses 50 concurrent
/// subscribers and 2,000 published events, checking the one property a
/// broadcast bus actually promises under load: every subscriber that was
/// listening the whole time receives every event, in order, none lost.
#[tokio::test]
async fn every_subscriber_receives_every_event_under_concurrent_publish_load() {
    let bus = event_bus::EventBus::new(4096);
    let subscriber_count = 50;
    let event_count = 2_000;

    let mut receiver_tasks = Vec::with_capacity(subscriber_count);
    for _ in 0..subscriber_count {
        let mut rx = bus.subscribe();
        receiver_tasks.push(tokio::spawn(async move {
            let mut received = Vec::with_capacity(event_count);
            for _ in 0..event_count {
                match rx.recv().await {
                    Ok(event_bus::Event::MemoryUpdated) => received.push(()),
                    Ok(_) => panic!("unexpected event variant"),
                    Err(e) => panic!("receiver lagged or bus closed: {e}"),
                }
            }
            received.len()
        }));
    }

    // Every `bus.subscribe()` call above already ran synchronously, in
    // this task, before any spawned receiver task even started -- so
    // every one of the 50 receivers is registered with the broadcast
    // channel before the first `publish` below, regardless of whether
    // the spawned tasks have reached their first `.recv().await` yet.
    // (A subscriber that calls `subscribe()` *after* an event is
    // published legitimately misses it -- that's documented `EventBus`
    // behavior, not a bug -- but that's not what's happening here.)
    let publisher = Arc::new(bus);
    for _ in 0..event_count {
        publisher.publish(event_bus::Event::MemoryUpdated);
    }

    for task in receiver_tasks {
        let received = task.await.unwrap();
        assert_eq!(received, event_count);
    }
}
