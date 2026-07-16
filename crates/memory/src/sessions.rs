//! Session and message persistence (DBDMS SS3 "Sessions"/"Messages";
//! Apis SS6 `store_message`/`load_history`).

use crate::{db_err, format_timestamp, parse_id, parse_timestamp, MemoryStore};
use common::{AppResult, Id};
use conversation::{Conversation, Message};
use rusqlite::{params, OptionalExtension};
use session_manager::Session;

/// A lightweight session listing (Apis SS6 `load_history` overview),
/// without pulling every message into memory.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionSummary {
    pub id: Id,
    pub started_at: common::Timestamp,
    pub model: String,
    pub title: Option<String>,
}

impl MemoryStore {
    /// Persists a session and its conversation so far. Safe to call
    /// repeatedly as a conversation grows: session metadata is upserted,
    /// and messages are inserted only if not already present (matched by
    /// id), so re-saving never duplicates history.
    pub fn save_session(&self, session: &Session) -> AppResult<()> {
        self.conn
            .execute(
                "INSERT INTO sessions (id, started_at, ended_at, model, title, summary)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(id) DO UPDATE SET
                     ended_at = excluded.ended_at,
                     title = excluded.title,
                     summary = excluded.summary",
                params![
                    session.id.to_string(),
                    format_timestamp(session.started_at),
                    session.ended_at.map(format_timestamp),
                    session.model,
                    session.title,
                    session.summary,
                ],
            )
            .map_err(db_err)?;

        for message in session.conversation.messages() {
            self.conn
                .execute(
                    "INSERT OR IGNORE INTO messages
                        (id, session_id, role, content, token_count, timestamp)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        message.id.to_string(),
                        session.id.to_string(),
                        message.role.as_str(),
                        message.content,
                        message.token_count,
                        format_timestamp(message.timestamp),
                    ],
                )
                .map_err(db_err)?;
        }
        Ok(())
    }

    /// Loads a full session (all messages) by id, or `None` if it was
    /// never saved.
    pub fn load_session(&self, id: Id) -> AppResult<Option<Session>> {
        let row = self
            .conn
            .prepare(
                "SELECT started_at, ended_at, model, title, summary
                 FROM sessions WHERE id = ?1",
            )
            .map_err(db_err)?
            .query_row(params![id.to_string()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            })
            .optional()
            .map_err(db_err)?;

        let Some((started_at, ended_at, model, title, summary)) = row else {
            return Ok(None);
        };

        let mut conversation = Conversation::new();
        let message_rows: Vec<(String, String, String, Option<u32>, String)> = self
            .conn
            .prepare(
                "SELECT id, role, content, token_count, timestamp
                 FROM messages WHERE session_id = ?1 ORDER BY timestamp ASC",
            )
            .map_err(db_err)?
            .query_map(params![id.to_string()], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })
            .map_err(db_err)?
            .collect::<Result<_, _>>()
            .map_err(db_err)?;

        for (msg_id, role, content, token_count, timestamp) in message_rows {
            conversation.push(Message {
                id: parse_id(&msg_id)?,
                role: role.parse().map_err(|e: conversation::ParseRoleError| {
                    common::AppError::Module {
                        module: "memory",
                        message: e.to_string(),
                    }
                })?,
                content,
                token_count,
                timestamp: parse_timestamp(&timestamp)?,
            });
        }

        Ok(Some(Session {
            id,
            started_at: parse_timestamp(&started_at)?,
            ended_at: ended_at.map(|s| parse_timestamp(&s)).transpose()?,
            model,
            title,
            summary,
            conversation,
        }))
    }

    /// Lists saved sessions, newest first (Apis SS6 `load_history`).
    pub fn list_sessions(&self) -> AppResult<Vec<SessionSummary>> {
        let rows: Vec<(String, String, String, Option<String>)> = self
            .conn
            .prepare("SELECT id, started_at, model, title FROM sessions ORDER BY started_at DESC")
            .map_err(db_err)?
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .map_err(db_err)?
            .collect::<Result<_, _>>()
            .map_err(db_err)?;

        rows.into_iter()
            .map(|(id, started_at, model, title)| {
                Ok(SessionSummary {
                    id: parse_id(&id)?,
                    started_at: parse_timestamp(&started_at)?,
                    model,
                    title,
                })
            })
            .collect()
    }
}
