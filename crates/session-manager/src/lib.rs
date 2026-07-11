//! `session-manager` -- in-memory session tracking (DBDMS SS3 "Sessions"
//! table; TDS SS2 "Session Manager" responsibilities).
//!
//! Persisted storage (SQLite-backed) lands in Phase 5 Memory -- sessions
//! here live only for the process's lifetime.

use common::{now, AppError, AppResult, Id, Timestamp};
use conversation::{Conversation, Message};
use std::collections::HashMap;

/// One session (DBDMS SS3: id, started_at, ended_at, model, title,
/// summary), plus the conversation it contains.
#[derive(Debug, Clone)]
pub struct Session {
    pub id: Id,
    pub started_at: Timestamp,
    pub ended_at: Option<Timestamp>,
    pub model: String,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub conversation: Conversation,
}

impl Session {
    fn new(model: impl Into<String>) -> Self {
        Self {
            id: Id::new(),
            started_at: now(),
            ended_at: None,
            model: model.into(),
            title: None,
            summary: None,
            conversation: Conversation::new(),
        }
    }
}

#[derive(Debug, Default)]
pub struct SessionManager {
    sessions: HashMap<Id, Session>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_session(&mut self, model: impl Into<String>) -> Id {
        let session = Session::new(model);
        let id = session.id;
        self.sessions.insert(id, session);
        id
    }

    pub fn get(&self, id: Id) -> Option<&Session> {
        self.sessions.get(&id)
    }

    pub fn get_mut(&mut self, id: Id) -> Option<&mut Session> {
        self.sessions.get_mut(&id)
    }

    pub fn append_message(&mut self, id: Id, message: Message) -> AppResult<()> {
        let session = self.sessions.get_mut(&id).ok_or_else(|| AppError::Module {
            module: "session-manager",
            message: format!("unknown session {id}"),
        })?;
        session.conversation.push(message);
        Ok(())
    }

    pub fn end_session(&mut self, id: Id) {
        if let Some(session) = self.sessions.get_mut(&id) {
            session.ended_at = Some(now());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conversation::Role;

    #[test]
    fn creates_and_retrieves_sessions() {
        let mut mgr = SessionManager::new();
        let id = mgr.create_session("gemma-3-1b");
        assert_eq!(mgr.get(id).unwrap().model, "gemma-3-1b");
    }

    #[test]
    fn appends_messages_to_the_right_session() {
        let mut mgr = SessionManager::new();
        let id = mgr.create_session("gemma-3-1b");
        mgr.append_message(id, Message::new(Role::User, "hi"))
            .unwrap();
        assert_eq!(mgr.get(id).unwrap().conversation.messages().len(), 1);
    }

    #[test]
    fn unknown_session_returns_error_not_panic() {
        let mut mgr = SessionManager::new();
        let fake_id = Id::new();
        assert!(mgr
            .append_message(fake_id, Message::new(Role::User, "hi"))
            .is_err());
    }

    #[test]
    fn ending_a_session_sets_ended_at() {
        let mut mgr = SessionManager::new();
        let id = mgr.create_session("gemma-3-1b");
        assert!(mgr.get(id).unwrap().ended_at.is_none());
        mgr.end_session(id);
        assert!(mgr.get(id).unwrap().ended_at.is_some());
    }
}
