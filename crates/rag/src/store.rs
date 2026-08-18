//! The knowledge base itself (Development_plan Phase 26 "Knowledge
//! Base", "Local Search"). SQLite for durable storage plus an
//! in-memory `vector_db::VectorIndex` mirror for search -- the same
//! persistence shape `memory::MemoryStore` already proved out, applied
//! to documents and their chunks instead of short personal memories
//! (see this crate's own doc comment for why they're a separate store,
//! not an extension of `MemoryStore`).

use crate::chunking::chunk_text;
use crate::pdf::extract_pdf_text;
use common::{now, AppError, AppResult, Id};
use embeddings::Embedder;
use rusqlite::{params, Connection, OptionalExtension};
use std::cell::RefCell;
use std::path::Path;
use vector_db::VectorIndex;

fn db_err(e: rusqlite::Error) -> AppError {
    AppError::Module {
        module: "rag",
        message: e.to_string(),
    }
}

fn module_err(context: impl std::fmt::Display) -> AppError {
    AppError::Module {
        module: "rag",
        message: context.to_string(),
    }
}

fn parse_id(s: &str) -> AppResult<Id> {
    s.parse()
        .map_err(|e: uuid::Error| module_err(format!("invalid id {s:?}: {e}")))
}

fn vector_to_blob(vector: &[f32]) -> Vec<u8> {
    vector.iter().flat_map(|f| f.to_le_bytes()).collect()
}

#[derive(Debug, Clone, PartialEq)]
pub struct DocumentInfo {
    pub id: Id,
    pub source_path: String,
    pub title: String,
    pub chunk_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ChunkHit {
    pub chunk_id: Id,
    pub document_id: Id,
    pub document_title: String,
    pub chunk_index: usize,
    pub content: String,
    pub score: f32,
}

fn create_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS documents (
            id          TEXT PRIMARY KEY,
            source_path TEXT NOT NULL,
            title       TEXT NOT NULL,
            added_at    TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS chunks (
            id          TEXT PRIMARY KEY,
            document_id TEXT NOT NULL REFERENCES documents(id),
            chunk_index INTEGER NOT NULL,
            content     TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_chunks_document_id ON chunks(document_id);

        CREATE TABLE IF NOT EXISTS chunk_embeddings (
            id         TEXT PRIMARY KEY,
            chunk_id   TEXT NOT NULL REFERENCES chunks(id),
            vector     BLOB NOT NULL,
            model      TEXT NOT NULL,
            dimension  INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_chunk_embeddings_chunk_id ON chunk_embeddings(chunk_id);
        ",
    )
}

pub struct KnowledgeBase {
    conn: Connection,
    embedder: Box<dyn Embedder>,
    /// In-memory mirror of `chunk_embeddings`, keyed by chunk id --
    /// identical role to `MemoryStore::index`.
    index: RefCell<VectorIndex<Id>>,
}

impl KnowledgeBase {
    pub fn open(path: impl AsRef<Path>, embedder: Box<dyn Embedder>) -> AppResult<Self> {
        Self::from_connection(Connection::open(path).map_err(db_err)?, embedder)
    }

    pub fn in_memory(embedder: Box<dyn Embedder>) -> AppResult<Self> {
        Self::from_connection(Connection::open_in_memory().map_err(db_err)?, embedder)
    }

    fn from_connection(conn: Connection, embedder: Box<dyn Embedder>) -> AppResult<Self> {
        create_schema(&conn).map_err(db_err)?;
        let mut index = VectorIndex::new();
        let rows: Vec<(String, Vec<u8>)> = conn
            .prepare("SELECT chunk_id, vector FROM chunk_embeddings")
            .map_err(db_err)?
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(db_err)?
            .collect::<Result<_, _>>()
            .map_err(db_err)?;
        for (chunk_id, blob) in rows {
            if let Ok(id) = chunk_id.parse::<Id>() {
                let vector = blob
                    .chunks_exact(4)
                    .map(|c| f32::from_le_bytes(c.try_into().expect("chunks_exact(4)")))
                    .collect();
                index.insert(id, vector);
            }
        }

        Ok(Self {
            conn,
            embedder,
            index: RefCell::new(index),
        })
    }

    /// Reads `path` (PDF via `pdf::extract_pdf_text` if the extension is
    /// `.pdf`, UTF-8 text otherwise), chunks it (`chunking::chunk_text`),
    /// embeds and stores every chunk. `title` defaults to `path`'s file
    /// name when not given. An empty document (a PDF with no text
    /// layer, or a chunk-less zero-length file) is indexed successfully
    /// with zero chunks rather than treated as an error -- a real,
    /// honest outcome (see `pdf::extract_pdf_text`'s own doc comment).
    pub fn index_document(
        &self,
        path: &Path,
        title: Option<&str>,
        max_words: usize,
        overlap_words: usize,
    ) -> AppResult<DocumentInfo> {
        let is_pdf = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("pdf"))
            .unwrap_or(false);

        let text = if is_pdf {
            extract_pdf_text(path)?
        } else {
            std::fs::read_to_string(path)
                .map_err(|e| module_err(format!("reading {}: {e}", path.display())))?
        };

        let title = title.map(str::to_string).unwrap_or_else(|| {
            path.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string_lossy().to_string())
        });

        let chunks = chunk_text(&text, max_words, overlap_words)?;

        let document_id = Id::new();
        self.conn
            .execute(
                "INSERT INTO documents (id, source_path, title, added_at) VALUES (?1, ?2, ?3, ?4)",
                params![
                    document_id.to_string(),
                    path.to_string_lossy(),
                    title,
                    format_timestamp(now()),
                ],
            )
            .map_err(db_err)?;

        for (chunk_index, content) in chunks.iter().enumerate() {
            let chunk_id = Id::new();
            self.conn
                .execute(
                    "INSERT INTO chunks (id, document_id, chunk_index, content) \
                     VALUES (?1, ?2, ?3, ?4)",
                    params![
                        chunk_id.to_string(),
                        document_id.to_string(),
                        chunk_index,
                        content
                    ],
                )
                .map_err(db_err)?;

            let vector = self.embedder.embed(content);
            self.conn
                .execute(
                    "INSERT INTO chunk_embeddings (id, chunk_id, vector, model, dimension) \
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        Id::new().to_string(),
                        chunk_id.to_string(),
                        vector_to_blob(&vector),
                        "hashing-v1",
                        vector.len(),
                    ],
                )
                .map_err(db_err)?;

            self.index.borrow_mut().insert(chunk_id, vector);
        }

        Ok(DocumentInfo {
            id: document_id,
            source_path: path.to_string_lossy().to_string(),
            title,
            chunk_count: chunks.len(),
        })
    }

    /// Semantic search over every indexed chunk, ranked by the
    /// configured `Embedder`'s cosine similarity, highest first.
    pub fn search(&self, query: &str, top_k: usize) -> AppResult<Vec<ChunkHit>> {
        let query_vector = self.embedder.embed(query);
        let hits = self.index.borrow().search(&query_vector, top_k);

        let mut results = Vec::with_capacity(hits.len());
        for (chunk_id, score) in hits {
            let row: Option<(String, i64, String, String, String)> = self
                .conn
                .query_row(
                    "SELECT c.document_id, c.chunk_index, c.content, d.title, d.id \
                     FROM chunks c JOIN documents d ON c.document_id = d.id \
                     WHERE c.id = ?1",
                    params![chunk_id.to_string()],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                        ))
                    },
                )
                .optional()
                .map_err(db_err)?;

            if let Some((_, chunk_index, content, document_title, document_id)) = row {
                results.push(ChunkHit {
                    chunk_id,
                    document_id: parse_id(&document_id)?,
                    document_title,
                    chunk_index: chunk_index as usize,
                    content,
                    score,
                });
            }
        }
        Ok(results)
    }

    pub fn list_documents(&self) -> AppResult<Vec<DocumentInfo>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT d.id, d.source_path, d.title, \
                 (SELECT COUNT(*) FROM chunks c WHERE c.document_id = d.id) \
                 FROM documents d ORDER BY d.added_at DESC",
            )
            .map_err(db_err)?;
        let rows = stmt
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let source_path: String = row.get(1)?;
                let title: String = row.get(2)?;
                let chunk_count: i64 = row.get(3)?;
                Ok((id, source_path, title, chunk_count))
            })
            .map_err(db_err)?;

        let mut documents = Vec::new();
        for row in rows {
            let (id, source_path, title, chunk_count) = row.map_err(db_err)?;
            documents.push(DocumentInfo {
                id: parse_id(&id)?,
                source_path,
                title,
                chunk_count: chunk_count as usize,
            });
        }
        Ok(documents)
    }

    /// Removes a document and every one of its chunks/embeddings.
    /// Returns whether anything was actually deleted, matching
    /// `MemoryStore::forget`'s own honest "unknown id is `false`, not an
    /// error" behavior.
    pub fn forget_document(&self, id: Id) -> AppResult<bool> {
        let chunk_ids: Vec<String> = self
            .conn
            .prepare("SELECT id FROM chunks WHERE document_id = ?1")
            .map_err(db_err)?
            .query_map(params![id.to_string()], |row| row.get(0))
            .map_err(db_err)?
            .collect::<Result<_, _>>()
            .map_err(db_err)?;

        // Children first: chunk_embeddings -> chunks -> documents.
        for chunk_id in &chunk_ids {
            self.conn
                .execute(
                    "DELETE FROM chunk_embeddings WHERE chunk_id = ?1",
                    params![chunk_id],
                )
                .map_err(db_err)?;
        }
        self.conn
            .execute(
                "DELETE FROM chunks WHERE document_id = ?1",
                params![id.to_string()],
            )
            .map_err(db_err)?;
        let changed = self
            .conn
            .execute(
                "DELETE FROM documents WHERE id = ?1",
                params![id.to_string()],
            )
            .map_err(db_err)?;

        let mut index = self.index.borrow_mut();
        for chunk_id in &chunk_ids {
            if let Ok(parsed) = parse_id(chunk_id) {
                index.remove(&parsed);
            }
        }

        Ok(changed > 0)
    }
}

fn format_timestamp(t: common::Timestamp) -> String {
    t.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use embeddings::HashingEmbedder;

    fn test_kb() -> KnowledgeBase {
        KnowledgeBase::in_memory(Box::new(HashingEmbedder::new(64))).unwrap()
    }

    #[test]
    fn indexes_a_real_text_file_and_finds_it_by_search() {
        let dir = std::env::temp_dir().join("rag-store-test-text");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("notes.txt");
        std::fs::write(
            &path,
            "The quarterly report shows revenue growth driven by the new agent architecture. \
             Customer satisfaction also improved significantly this quarter.",
        )
        .unwrap();

        let kb = test_kb();
        let info = kb.index_document(&path, None, 200, 40).unwrap();
        assert_eq!(info.title, "notes.txt");
        assert_eq!(info.chunk_count, 1);

        let hits = kb.search("quarterly revenue growth", 5).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].document_title, "notes.txt");
        assert!(hits[0].content.contains("revenue growth"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_long_document_is_split_into_multiple_real_chunks() {
        let dir = std::env::temp_dir().join("rag-store-test-long");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("long.txt");
        let words: Vec<String> = (0..300).map(|i| format!("word{i}")).collect();
        std::fs::write(&path, words.join(" ")).unwrap();

        let kb = test_kb();
        let info = kb.index_document(&path, Some("Long Doc"), 100, 20).unwrap();
        assert!(
            info.chunk_count > 1,
            "expected multiple chunks, got {}",
            info.chunk_count
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn explicit_title_overrides_the_filename_default() {
        let dir = std::env::temp_dir().join("rag-store-test-title");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("untitled.txt");
        std::fs::write(&path, "some content").unwrap();

        let kb = test_kb();
        let info = kb
            .index_document(&path, Some("My Custom Title"), 200, 40)
            .unwrap();
        assert_eq!(info.title, "My Custom Title");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn list_documents_reflects_real_indexed_state() {
        let dir = std::env::temp_dir().join("rag-store-test-list");
        std::fs::create_dir_all(&dir).unwrap();
        let path_a = dir.join("a.txt");
        let path_b = dir.join("b.txt");
        std::fs::write(&path_a, "document a content").unwrap();
        std::fs::write(&path_b, "document b content").unwrap();

        let kb = test_kb();
        kb.index_document(&path_a, None, 200, 40).unwrap();
        kb.index_document(&path_b, None, 200, 40).unwrap();

        let docs = kb.list_documents().unwrap();
        assert_eq!(docs.len(), 2);
        let titles: std::collections::HashSet<&str> =
            docs.iter().map(|d| d.title.as_str()).collect();
        assert!(titles.contains("a.txt"));
        assert!(titles.contains("b.txt"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn forgetting_a_document_removes_it_from_search_and_listing() {
        let dir = std::env::temp_dir().join("rag-store-test-forget");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("temp-doc.txt");
        std::fs::write(
            &path,
            "a fact about narwhals that nothing else in this store mentions",
        )
        .unwrap();

        let kb = test_kb();
        let info = kb.index_document(&path, None, 200, 40).unwrap();
        assert_eq!(kb.search("narwhals", 5).unwrap().len(), 1);

        let forgotten = kb.forget_document(info.id).unwrap();
        assert!(forgotten);
        assert_eq!(kb.search("narwhals", 5).unwrap().len(), 0);
        assert_eq!(kb.list_documents().unwrap().len(), 0);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn forgetting_an_unknown_id_is_a_real_false_not_an_error() {
        let kb = test_kb();
        assert!(!kb.forget_document(Id::new()).unwrap());
    }

    #[test]
    fn a_real_pdf_with_no_text_layer_indexes_successfully_with_zero_chunks() {
        // Confirms the honest "empty is a real outcome, not an error"
        // behavior documented in `pdf::extract_pdf_text` actually holds
        // all the way up through `index_document`, not just at the
        // extraction layer -- an empty-content-stream PDF is spec-valid
        // and genuinely has no text, the same as a real scanned-image PDF.
        let dir = std::env::temp_dir().join("rag-store-test-empty-pdf");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("empty.pdf");

        let objects: Vec<Vec<u8>> = vec![
            b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
            b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
            b"<< /Type /Page /Parent 2 0 R /Resources << >> /MediaBox [0 0 300 150] \
              /Contents 4 0 R >>"
                .to_vec(),
            b"<< /Length 0 >>\nstream\n\nendstream".to_vec(),
        ];
        let mut out: Vec<u8> = b"%PDF-1.4\n".to_vec();
        let mut offsets = Vec::new();
        for (i, body) in objects.iter().enumerate() {
            offsets.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
            out.extend_from_slice(body);
            out.extend_from_slice(b"\nendobj\n");
        }
        let xref_offset = out.len();
        let n = objects.len() + 1;
        out.extend_from_slice(format!("xref\n0 {n}\n").as_bytes());
        out.extend_from_slice(b"0000000000 65535 f \n");
        for off in &offsets {
            out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(
            format!("trailer\n<< /Size {n} /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF")
                .as_bytes(),
        );
        std::fs::write(&path, out).unwrap();

        let kb = test_kb();
        let info = kb.index_document(&path, None, 200, 40).unwrap();
        assert_eq!(info.chunk_count, 0);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn persists_across_reopening_the_same_file() {
        let dir = std::env::temp_dir().join("rag-store-test-persist");
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("kb.sqlite3");
        let doc_path = dir.join("doc.txt");
        std::fs::write(&doc_path, "a persistent fact about the aurora borealis").unwrap();

        {
            let kb = KnowledgeBase::open(&db_path, Box::new(HashingEmbedder::new(64))).unwrap();
            kb.index_document(&doc_path, None, 200, 40).unwrap();
        }
        {
            let kb = KnowledgeBase::open(&db_path, Box::new(HashingEmbedder::new(64))).unwrap();
            let hits = kb.search("aurora borealis", 5).unwrap();
            assert_eq!(
                hits.len(),
                1,
                "the reopened store should find the previously indexed chunk"
            );
        }

        std::fs::remove_dir_all(&dir).ok();
    }
}
