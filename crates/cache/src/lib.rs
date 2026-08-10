//! `cache` -- a small, dependency-free, capacity- and TTL-bounded cache
//! (Development_plan Phase 18 "Caching").
//!
//! [`Cache`] is the single-threaded core: least-recently-used eviction
//! once `capacity` is exceeded, plus an optional per-entry TTL. Eviction
//! is a brute-force scan for the least-recently-used entry, deliberately
//! -- at the scale this workspace's own caches actually run (tens to a
//! few hundred entries: `embeddings::CachingEmbedder`'s memoized
//! vectors, `memory::MemoryStore`'s search-result cache), a linear scan
//! is fast enough and avoids the bug surface of a hand-rolled intrusive
//! linked list. This is the same reasoning `vector_db::VectorIndex`
//! already documents for its own brute-force similarity search --
//! revisit both if a cache size far larger than this project's actual
//! use ever shows up in a profile.
//!
//! [`SharedCache`] wraps [`Cache`] in `Arc<Mutex<_>>` for callers that
//! need to hand the same cache to more than one owner or thread --
//! following the exact pattern `search::MemorySearchProvider` already
//! established for `memory::MemoryStore` and `permission-manager`
//! established for its own `Connection`, rather than inventing a new
//! sharing convention.

mod lru;
mod shared;

pub use lru::{Cache, CacheConfig, CacheStats};
pub use shared::SharedCache;
