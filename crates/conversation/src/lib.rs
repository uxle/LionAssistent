//! `conversation` -- the message and conversation model (DBDMS SS3
//! "Messages" table; TDS SS2 "Conversation Manager" responsibilities).

use common::{now, Id, Timestamp};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    User,
    Assistant,
    System,
    Tool,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::System => "system",
            Role::Tool => "tool",
        }
    }
}

/// One chat message (DBDMS SS3 Messages: id, session_id, role, content,
/// token_count, timestamp -- `session_id` lives on `Session` in
/// `session-manager` rather than here, since a `Conversation` is already
/// scoped to one session).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: Id,
    pub role: Role,
    pub content: String,
    /// Populated once the tokenizer exists (Phase 4). `None` until then.
    pub token_count: Option<u32>,
    pub timestamp: Timestamp,
}

impl Message {
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        Self {
            id: Id::new(),
            role,
            content: content.into(),
            token_count: None,
            timestamp: now(),
        }
    }
}

/// An ordered list of messages within one session (TDS SS2 "Conversation
/// Manager"). Persistence (SQLite-backed) lands in Phase 5 Memory --
/// this is in-memory only.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Conversation {
    messages: Vec<Message>,
}

impl Conversation {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, message: Message) {
        self.messages.push(message);
    }

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    pub fn last_user_message(&self) -> Option<&Message> {
        self.messages.iter().rev().find(|m| m.role == Role::User)
    }

    pub fn len(&self) -> usize {
        self.messages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_messages_in_order() {
        let mut conv = Conversation::new();
        conv.push(Message::new(Role::User, "hi"));
        conv.push(Message::new(Role::Assistant, "hello"));
        assert_eq!(conv.len(), 2);
        assert_eq!(conv.messages()[0].role, Role::User);
        assert_eq!(conv.messages()[1].role, Role::Assistant);
    }

    #[test]
    fn finds_last_user_message() {
        let mut conv = Conversation::new();
        conv.push(Message::new(Role::User, "first"));
        conv.push(Message::new(Role::Assistant, "reply"));
        conv.push(Message::new(Role::User, "second"));
        assert_eq!(conv.last_user_message().unwrap().content, "second");
    }

    #[test]
    fn empty_conversation_has_no_last_user_message() {
        assert!(Conversation::new().last_user_message().is_none());
    }
}
