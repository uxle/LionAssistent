//! Long-term memory: facts the user has chosen to save, with semantic
//! search over them (DBDMS SS3 "Memories"/"Embeddings"; Apis SS6
//! `remember`/`forget`/`search_memory`; Archd SS6 "Long-Term Memory").

use crate::{db_err, format_timestamp, parse_id, MemoryStore};
use common::{now, AppError, AppResult, Id};
use rusqlite::{params, OptionalExtension};
use vector_db::VectorIndex;

/// Matches DBDMS SS3 "Memories" table's `category` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryCategory {
    Preference,
    Workflow,
    Reminder,
    Knowledge,
    Project,
    Custom,
}

impl MemoryCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Preference => "preference",
            Self::Workflow => "workflow",
            Self::Reminder => "reminder",
            Self::Knowledge => "knowledge",
            Self::Project => "project",
            Self::Custom => "custom",
        }
    }
}

impl std::str::FromStr for MemoryCategory {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "preference" => Self::Preference,
            "workflow" => Self::Workflow,
            "reminder" => Self::Reminder,
            "knowledge" => Self::Knowledge,
            "project" => Self::Project,
            "custom" => Self::Custom,
            other => return Err(format!("unknown memory category: {other:?}")),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MemoryItem {
    pub id: Id,
    pub category: MemoryCategory,
    pub title: String,
    pub content: String,
    pub importance: u8,
    pub confidence: f32,
}

impl MemoryStore {
    /// Saves a long-term memory (PRD 6.5, Archd "Long-Term Memory": facts
    /// the user has chosen to save). This store doesn't decide *when*
    /// that consent has been given -- Ai_reasoning SS17 requires explicit
    /// approval before storage, which is the caller's job, same as
    /// `EventBus::publish` not deciding what's worth publishing.
    pub fn remember(
        &self,
        category: MemoryCategory,
        title: &str,
        content: &str,
        importance: u8,
    ) -> AppResult<Id> {
        let id = Id::new();
        let timestamp = format_timestamp(now());
        self.conn
            .execute(
                "INSERT INTO memories
                    (id, category, title, content, importance, confidence, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
                params![
                    id.to_string(),
                    category.as_str(),
                    title,
                    content,
                    importance,
                    1.0f32,
                    timestamp,
                ],
            )
            .map_err(db_err)?;

        let vector = self.embedder.embed(&format!("{title}\n{content}"));
        self.conn
            .execute(
                "INSERT INTO embeddings (id, memory_id, vector, model, dimension)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    Id::new().to_string(),
                    id.to_string(),
                    vector_to_blob(&vector),
                    "hashing-v1",
                    vector.len(),
                ],
            )
            .map_err(db_err)?;

        self.index.borrow_mut().insert(id, vector);
        Ok(id)
    }

    /// Deletes a memory and its embedding. Returns whether anything was
    /// actually deleted, rather than erroring on an already-gone id.
    pub fn forget(&self, id: Id) -> AppResult<bool> {
        // Child row first: `embeddings.memory_id` references `memories.id`.
        self.conn
            .execute(
                "DELETE FROM embeddings WHERE memory_id = ?1",
                params![id.to_string()],
            )
            .map_err(db_err)?;
        let changed = self
            .conn
            .execute(
                "DELETE FROM memories WHERE id = ?1",
                params![id.to_string()],
            )
            .map_err(db_err)?;
        self.index.borrow_mut().remove(&id);
        Ok(changed > 0)
    }

    /// Semantic search over saved memories (Apis SS6 `search_memory`),
    /// ranked by the configured `Embedder`'s similarity, highest first.
    pub fn search_memory(&self, query: &str, top_k: usize) -> AppResult<Vec<MemoryItem>> {
        let query_vector = self.embedder.embed(query);
        let hits = self.index.borrow().search(&query_vector, top_k);

        let mut results = Vec::with_capacity(hits.len());
        for (id, _score) in hits {
            if let Some(item) = self.get_memory(id)? {
                results.push(item);
            }
        }
        Ok(results)
    }

    /// Lists every saved memory, newest first.
    pub fn list_memories(&self) -> AppResult<Vec<MemoryItem>> {
        let ids: Vec<String> = self
            .conn
            .prepare("SELECT id FROM memories ORDER BY created_at DESC")
            .map_err(db_err)?
            .query_map([], |row| row.get(0))
            .map_err(db_err)?
            .collect::<Result<_, _>>()
            .map_err(db_err)?;

        ids.into_iter()
            .map(|s| parse_id(&s))
            .collect::<AppResult<Vec<_>>>()?
            .into_iter()
            .filter_map(|id| self.get_memory(id).transpose())
            .collect()
    }

    fn get_memory(&self, id: Id) -> AppResult<Option<MemoryItem>> {
        let row = self
            .conn
            .prepare(
                "SELECT category, title, content, importance, confidence
                 FROM memories WHERE id = ?1",
            )
            .map_err(db_err)?
            .query_row(params![id.to_string()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, u8>(3)?,
                    row.get::<_, f32>(4)?,
                ))
            })
            .optional()
            .map_err(db_err)?;

        let Some((category, title, content, importance, confidence)) = row else {
            return Ok(None);
        };

        Ok(Some(MemoryItem {
            id,
            category: category.parse().map_err(|e: String| AppError::Module {
                module: "memory",
                message: e,
            })?,
            title,
            content,
            importance,
            confidence,
        }))
    }
}

pub(crate) fn load_index(conn: &rusqlite::Connection) -> rusqlite::Result<VectorIndex<Id>> {
    let mut index = VectorIndex::new();
    let rows: Vec<(String, Vec<u8>)> = conn
        .prepare("SELECT memory_id, vector FROM embeddings")?
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_, _>>()?;

    for (memory_id, blob) in rows {
        if let Ok(id) = memory_id.parse::<Id>() {
            index.insert(id, blob_to_vector(&blob));
        }
    }
    Ok(index)
}

fn vector_to_blob(vector: &[f32]) -> Vec<u8> {
    vector.iter().flat_map(|f| f.to_le_bytes()).collect()
}

fn blob_to_vector(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes(c.try_into().expect("chunks_exact(4) guarantees length 4")))
        .collect()
}
