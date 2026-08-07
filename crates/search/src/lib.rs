//! `search` -- global, file, memory, and command search over the
//! assistant's own data and capabilities (Development_plan Phase 13;
//! TCLIS SS4 "Search" tool category).
//!
//! Four focused providers, one aggregator:
//!
//! - [`FileSearchProvider`] -- wraps Phase 8's
//!   `linux_services::files::search`.
//! - [`MemorySearchProvider`] -- wraps Phase 5's
//!   `memory::MemoryStore::search_memory`.
//! - [`CommandSearchProvider`] -- indexes Phase 7's registered
//!   `tool_framework::Tool`s and Phase 10's saved, enabled
//!   `workflow_engine::Workflow`s.
//! - [`SearchEngine`] -- the "Global Search" module: queries all three
//!   and merges them into one ranked list via [`ranking::score_text`].
//!
//! [`tools`] wires all four up as real, `ToolCategory::Search` tools
//! (declared in Phase 7, unused until now) through the same register/
//! validate/permission-check/execute/audit pipeline every other real
//! tool in this workspace goes through -- TCLIS SS1 is explicit that
//! nothing bypasses it, and search is no exception even though it's
//! entirely read-only.
//!
//! Like `tool-framework` and `linux-services` before it, this crate
//! isn't wired into `assistant-core`/`app`'s live message flow -- that
//! still needs a working `llm-runtime` (Phase 4) to turn a user's
//! request into a concrete tool call in the first place, or Phase 2's UI
//! toolchain gap resolved so `app` itself can even compile in this
//! sandbox. See `docs/ROADMAP.md` for specifics; both are unchanged by
//! this phase.

pub mod command_search;
pub mod engine;
pub mod file_search;
pub mod memory_search;
pub mod provider;
pub mod ranking;
pub mod tools;
pub mod types;

pub use command_search::CommandSearchProvider;
pub use engine::SearchEngine;
pub use file_search::FileSearchProvider;
pub use memory_search::MemorySearchProvider;
pub use provider::SearchProvider;
pub use types::{SearchHit, SearchKind};
