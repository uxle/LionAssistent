//! Remembering and searching long-term memories end to end (PRD 6.5,
//! Archd "Long-Term Memory"). Run with:
//!
//!     cargo run -p memory --example remember_and_search

use embeddings::HashingEmbedder;
use memory::{MemoryCategory, MemoryStore};

fn main() {
    // An in-memory store: no file, gone when the process exits. Real
    // callers use `MemoryStore::open("data/database/assistant.db", ...)`
    // instead -- see crates/app/src/main.rs's `open_memory` for how the
    // real app wires this up, including Phase 18's caching embedder.
    let store = MemoryStore::in_memory(Box::<HashingEmbedder>::default()).unwrap();

    store
        .remember(
            MemoryCategory::Preference,
            "editor",
            "the user prefers vim keybindings and a dark theme",
            5,
        )
        .unwrap();
    store
        .remember(
            MemoryCategory::Preference,
            "shell",
            "the user's default shell is zsh with starship prompt",
            4,
        )
        .unwrap();
    store
        .remember(
            MemoryCategory::Project,
            "current-project",
            "working on a Rust CLI tool for log analysis",
            5,
        )
        .unwrap();

    // Semantic search: ranked by the configured Embedder's similarity,
    // not exact keyword matching -- "keyboard shortcuts" finds the vim
    // memory even without either word appearing verbatim in it.
    let hits = store.search_memory("keyboard shortcuts", 2).unwrap();
    println!("search: \"keyboard shortcuts\" (top 2)");
    for item in &hits {
        println!("  [{:?}] {}: {}", item.category, item.title, item.content);
    }

    // Repeating the identical search is served from Phase 18's
    // search-result cache -- same results, no re-embedding or
    // re-scanning the index.
    store.search_memory("keyboard shortcuts", 2).unwrap();
    let stats = store.search_cache_stats();
    println!(
        "\nsearch cache: {} hit(s), {} miss(es)",
        stats.hits, stats.misses
    );
}
