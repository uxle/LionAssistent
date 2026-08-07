//! `memory` -- SQLite-backed persistent memory: sessions/messages,
//! long-term memories the user has chosen to save, and semantic search
//! over them (Development_plan Phase 5; Archd SS6 "Memory System"; DBDMS
//! SS3 tables).
//!
//! Short-term memory (the live conversation) stays exactly where Phase 3
//! put it -- `session_manager::SessionManager`, in-memory. This crate is
//! what makes that durable across restarts, plus adds the long-term store
//! PRD 6.5 and Archd SS6 describe as a separate concept: facts the user
//! has explicitly chosen to save, independent of any one conversation.

mod long_term;
mod schema;
mod sessions;

pub use long_term::{MemoryCategory, MemoryItem};
pub use sessions::SessionSummary;

use common::{AppError, AppResult, Id, Timestamp};
use embeddings::Embedder;
use rusqlite::Connection;
use std::cell::RefCell;
use std::path::Path;
use vector_db::VectorIndex;

pub struct MemoryStore {
    conn: Connection,
    embedder: Box<dyn Embedder>,
    /// In-memory mirror of the `embeddings` table for fast search without
    /// hitting disk on every query; kept in sync by `remember`/`forget`
    /// and rebuilt from the database on `open`/`in_memory`.
    index: RefCell<VectorIndex<Id>>,
}

impl MemoryStore {
    /// Opens (creating if needed) a SQLite-backed store at `path`.
    pub fn open(path: impl AsRef<Path>, embedder: Box<dyn Embedder>) -> AppResult<Self> {
        Self::from_connection(Connection::open(path).map_err(db_err)?, embedder)
    }

    /// An in-memory store: no file, gone when it's dropped. Useful for
    /// tests, and for running with persistence turned off (PRD 6.7:
    /// memory settings are user-configurable).
    pub fn in_memory(embedder: Box<dyn Embedder>) -> AppResult<Self> {
        Self::from_connection(Connection::open_in_memory().map_err(db_err)?, embedder)
    }

    fn from_connection(conn: Connection, embedder: Box<dyn Embedder>) -> AppResult<Self> {
        schema::create_schema(&conn).map_err(db_err)?;
        let index = long_term::load_index(&conn).map_err(db_err)?;
        Ok(Self {
            conn,
            embedder,
            index: RefCell::new(index),
        })
    }

    /// A first, honest pass at Apis SS6 `summarize_history`: extractive
    /// (keeps the first and last few messages verbatim), not
    /// model-generated. A real abstractive summary needs a working
    /// `llm-runtime`, which has its own unresolved sandbox constraint --
    /// see `docs/ROADMAP.md` Phase 4. This is still genuinely useful for
    /// keeping a long conversation's topic visible in a session list.
    pub fn summarize_session(&self, id: Id) -> AppResult<Option<String>> {
        let Some(session) = self.load_session(id)? else {
            return Ok(None);
        };
        Ok(Some(extractive_summary(session.conversation.messages())))
    }
}

fn extractive_summary(messages: &[conversation::Message]) -> String {
    const HEAD: usize = 2;
    const TAIL: usize = 2;

    let describe = |m: &conversation::Message| format!("{}: {}", m.role.as_str(), m.content);

    if messages.len() <= HEAD + TAIL {
        return messages
            .iter()
            .map(describe)
            .collect::<Vec<_>>()
            .join(" / ");
    }

    let mut parts: Vec<String> = messages[..HEAD].iter().map(describe).collect();
    parts.push(format!(
        "... ({} more messages) ...",
        messages.len() - HEAD - TAIL
    ));
    parts.extend(messages[messages.len() - TAIL..].iter().map(describe));
    parts.join(" / ")
}

/// RFC 3339 with a fixed nanosecond-precision, zero-padded format, so
/// stored timestamps sort correctly as plain text (`ORDER BY` on the raw
/// column) without needing a separate numeric column.
pub(crate) fn format_timestamp(t: Timestamp) -> String {
    t.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
}

pub(crate) fn parse_timestamp(s: &str) -> AppResult<Timestamp> {
    s.parse().map_err(|e: chrono::ParseError| AppError::Module {
        module: "memory",
        message: format!("invalid timestamp {s:?}: {e}"),
    })
}

pub(crate) fn parse_id(s: &str) -> AppResult<Id> {
    s.parse().map_err(|e: uuid::Error| AppError::Module {
        module: "memory",
        message: format!("invalid id {s:?}: {e}"),
    })
}

pub(crate) fn db_err(e: rusqlite::Error) -> AppError {
    AppError::Module {
        module: "memory",
        message: e.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conversation::{Message, Role};
    use embeddings::HashingEmbedder;
    use session_manager::SessionManager;

    fn test_store() -> MemoryStore {
        MemoryStore::in_memory(Box::<HashingEmbedder>::default()).unwrap()
    }

    #[test]
    fn round_trips_a_session_with_messages() {
        let store = test_store();
        let mut manager = SessionManager::new();
        let id = manager.create_session("gemma-3-1b");
        manager
            .append_message(id, Message::new(Role::User, "hello"))
            .unwrap();
        manager
            .append_message(id, Message::new(Role::Assistant, "hi there"))
            .unwrap();
        store.save_session(manager.get(id).unwrap()).unwrap();

        let loaded = store.load_session(id).unwrap().unwrap();
        assert_eq!(loaded.model, "gemma-3-1b");
        assert_eq!(loaded.conversation.messages().len(), 2);
        assert_eq!(loaded.conversation.messages()[0].content, "hello");
        assert_eq!(loaded.conversation.messages()[1].role, Role::Assistant);
    }

    #[test]
    fn loading_an_unknown_session_returns_none() {
        assert!(test_store().load_session(Id::new()).unwrap().is_none());
    }

    #[test]
    fn saving_a_session_twice_does_not_duplicate_messages() {
        let store = test_store();
        let mut manager = SessionManager::new();
        let id = manager.create_session("gemma-3-1b");
        manager
            .append_message(id, Message::new(Role::User, "hi"))
            .unwrap();
        store.save_session(manager.get(id).unwrap()).unwrap();
        store.save_session(manager.get(id).unwrap()).unwrap();

        assert_eq!(
            store
                .load_session(id)
                .unwrap()
                .unwrap()
                .conversation
                .messages()
                .len(),
            1
        );
    }

    #[test]
    fn remembers_and_forgets_long_term_memories() {
        let store = test_store();
        let id = store
            .remember(
                MemoryCategory::Preference,
                "theme",
                "the user prefers dark mode",
                5,
            )
            .unwrap();

        let items = store.list_memories().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, id);

        assert!(store.forget(id).unwrap());
        assert!(store.list_memories().unwrap().is_empty());
    }

    #[test]
    fn forgetting_an_unknown_id_returns_false_not_error() {
        assert!(!test_store().forget(Id::new()).unwrap());
    }

    #[test]
    fn search_memory_finds_the_lexically_closer_entry() {
        let store = test_store();
        store
            .remember(
                MemoryCategory::Preference,
                "editor",
                "the user prefers vim keybindings",
                5,
            )
            .unwrap();
        store
            .remember(
                MemoryCategory::Knowledge,
                "recipe",
                "grandmother's lasagna needs fresh basil",
                3,
            )
            .unwrap();

        let hits = store
            .search_memory("what keybindings does the user like", 5)
            .unwrap();
        assert!(!hits.is_empty());
        assert_eq!(hits[0].title, "editor");
    }

    #[test]
    fn list_sessions_orders_newest_first() {
        let store = test_store();
        let mut manager = SessionManager::new();
        let first = manager.create_session("gemma-3-1b");
        store.save_session(manager.get(first).unwrap()).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let second = manager.create_session("gemma-3-1b");
        store.save_session(manager.get(second).unwrap()).unwrap();

        let sessions = store.list_sessions().unwrap();
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].id, second);
    }

    #[test]
    fn summarize_session_keeps_head_and_tail() {
        let store = test_store();
        let mut manager = SessionManager::new();
        let id = manager.create_session("gemma-3-1b");
        for i in 0..10 {
            manager
                .append_message(id, Message::new(Role::User, format!("message {i}")))
                .unwrap();
        }
        store.save_session(manager.get(id).unwrap()).unwrap();

        let summary = store.summarize_session(id).unwrap().unwrap();
        assert!(summary.contains("message 0"));
        assert!(summary.contains("message 9"));
        assert!(summary.contains("more messages"));
    }

    #[test]
    fn summarize_unknown_session_returns_none() {
        assert!(test_store().summarize_session(Id::new()).unwrap().is_none());
    }
}
