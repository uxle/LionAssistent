//! Table definitions (DBDMS SS3). Kept as one batched statement since
//! these tables' shapes are locked together (foreign keys between them);
//! a real migration system (DBDMS SS14) is future work once the schema
//! actually needs to change under existing data.

use rusqlite::Connection;

pub(crate) fn create_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS sessions (
            id         TEXT PRIMARY KEY,
            started_at TEXT NOT NULL,
            ended_at   TEXT,
            model      TEXT NOT NULL,
            title      TEXT,
            summary    TEXT
        );

        CREATE TABLE IF NOT EXISTS messages (
            id          TEXT PRIMARY KEY,
            session_id  TEXT NOT NULL REFERENCES sessions(id),
            role        TEXT NOT NULL,
            content     TEXT NOT NULL,
            token_count INTEGER,
            timestamp   TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_messages_session_id ON messages(session_id);
        CREATE INDEX IF NOT EXISTS idx_messages_timestamp ON messages(timestamp);

        CREATE TABLE IF NOT EXISTS memories (
            id          TEXT PRIMARY KEY,
            category    TEXT NOT NULL,
            title       TEXT NOT NULL,
            content     TEXT NOT NULL,
            importance  INTEGER NOT NULL,
            confidence  REAL NOT NULL,
            created_at  TEXT NOT NULL,
            updated_at  TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_memories_category ON memories(category);

        CREATE TABLE IF NOT EXISTS embeddings (
            id         TEXT PRIMARY KEY,
            memory_id  TEXT NOT NULL REFERENCES memories(id),
            vector     BLOB NOT NULL,
            model      TEXT NOT NULL,
            dimension  INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_embeddings_memory_id ON embeddings(memory_id);
        ",
    )
}
