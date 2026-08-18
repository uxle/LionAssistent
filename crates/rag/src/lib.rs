//! Document indexing, PDF text extraction, and a local knowledge base
//! with semantic search over both (Development_plan Phase 26 "RAG
//! System": Document Indexing, PDF Support, Knowledge Base, Local
//! Search; Archd SS20 names "Retrieval-Augmented Generation (RAG)"
//! explicitly as a future architecture extension).
//!
//! # Relationship to `memory`
//!
//! DBDMS's own data model (SS3) and Ai_reasoning SS9's "Memory
//! Strategy" already distinguish "Long-Term Memory" (short,
//! user-approved personal facts -- `memory::MemoryStore::remember`)
//! from "Semantic Memory" ("embedded documents and notes" -- what this
//! crate is). They're conceptually distinct even though both end up
//! persisted and searched by embedding similarity, so this crate is a
//! genuinely separate store (its own SQLite database, its own
//! `vector_db::VectorIndex`), not an extension bolted onto
//! `MemoryStore`. The persistence shape is deliberately the same as
//! `memory`'s, though -- SQLite for durability, an in-memory
//! `VectorIndex` mirror rebuilt on open and kept in sync on writes --
//! because that pattern is already proven in this workspace, not
//! because the two stores are the same thing.
//!
//! # What's real, module by module
//!
//! - [`chunking`]: real, deterministic, dependency-free.
//! - [`pdf`]: real, via `pdf-extract` (pinned -- see that module's own
//!   doc comment), verified against a hand-built, spec-checked PDF
//!   after ImageMagick's own PDF output turned out to have no
//!   extractable text layer at all (a real finding from building this
//!   phase, not assumed).
//! - [`store`]: real SQLite persistence and real cosine-similarity
//!   search via `vector_db::VectorIndex`, the same proven building
//!   blocks `memory::MemoryStore` already uses.

pub mod chunking;
pub mod pdf;
pub mod store;
pub mod tools;
