//! Wires the knowledge base up as real [`tool_framework::Tool`]s,
//! following `linux-services::tools`' exact shape.

use crate::chunking::{DEFAULT_MAX_WORDS, DEFAULT_OVERLAP_WORDS};
use crate::store::KnowledgeBase;
use common::Id;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tool_framework::{
    InputSchema, ParamKind, ParamSchema, PermissionLevel, Tool, ToolArgs, ToolCategory, ToolOutcome,
};

fn err_outcome(e: impl std::fmt::Display) -> ToolOutcome {
    ToolOutcome {
        success: false,
        message: e.to_string(),
        data: None,
    }
}

pub struct IndexDocumentTool {
    kb: Arc<Mutex<KnowledgeBase>>,
}

impl IndexDocumentTool {
    pub fn new(kb: Arc<Mutex<KnowledgeBase>>) -> Self {
        Self { kb }
    }
}

impl Tool for IndexDocumentTool {
    fn name(&self) -> &str {
        "index_document"
    }
    fn description(&self) -> &str {
        "Indexes a text or PDF file into the local knowledge base: extracts its text, splits it \
         into overlapping chunks, and embeds each chunk for later semantic search."
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Development
    }
    /// `Moderate`, not `ReadOnly`: like `read_file`, this reads a
    /// caller-named path (not `screenshot`'s "exposes whatever's on
    /// screen" shape), but unlike `read_file`/`extract_text_from_image`/
    /// `describe_image`, it also *creates new persistent data* -- rows
    /// in the knowledge base's own database, taking up real storage
    /// that outlives this one call. That's the same "user data
    /// modification" TCLIS SS6 already puts `write_file`/`create_file`
    /// at `Moderate` for; anchored at the same level for the same
    /// reason, not the `ReadOnly` level a pure read would get.
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Moderate
    }
    fn timeout(&self) -> Duration {
        Duration::from_secs(30)
    }
    fn input_schema(&self) -> InputSchema {
        InputSchema::new(vec![
            ParamSchema::required("path", ParamKind::String),
            ParamSchema::optional("title", ParamKind::String),
            ParamSchema::optional("max_words", ParamKind::Integer),
            ParamSchema::optional("overlap_words", ParamKind::Integer),
        ])
    }
    fn execute(&self, args: &ToolArgs) -> ToolOutcome {
        let Some(path) = args.get("path").and_then(|v| v.as_str()) else {
            return err_outcome("missing required parameter \"path\"");
        };
        let title = args.get("title").and_then(|v| v.as_str());
        let max_words = args
            .get("max_words")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(DEFAULT_MAX_WORDS);
        let overlap_words = args
            .get("overlap_words")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(DEFAULT_OVERLAP_WORDS);

        let kb = match self.kb.lock() {
            Ok(kb) => kb,
            Err(e) => return err_outcome(format!("knowledge base lock was poisoned: {e}")),
        };
        match kb.index_document(Path::new(path), title, max_words, overlap_words) {
            Ok(info) => ToolOutcome {
                success: true,
                message: format!(
                    "indexed {:?} as {} chunk(s) under the title {:?}",
                    path, info.chunk_count, info.title
                ),
                data: Some(serde_json::json!({
                    "id": info.id.to_string(),
                    "title": info.title,
                    "chunk_count": info.chunk_count,
                })),
            },
            Err(e) => err_outcome(e),
        }
    }
}

pub struct SearchKnowledgeBaseTool {
    kb: Arc<Mutex<KnowledgeBase>>,
}

impl SearchKnowledgeBaseTool {
    pub fn new(kb: Arc<Mutex<KnowledgeBase>>) -> Self {
        Self { kb }
    }
}

impl Tool for SearchKnowledgeBaseTool {
    fn name(&self) -> &str {
        "search_knowledge_base"
    }
    fn description(&self) -> &str {
        "Semantic search over every chunk indexed into the local knowledge base, ranked by \
         similarity to the query."
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Development
    }
    /// `ReadOnly`: no side effects, matches `search_memory`'s own
    /// precedent (accessible only via `agents::memory_agent` today,
    /// same reasoning nonetheless) and `extract_text_from_image`/
    /// `describe_image`'s.
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
    fn timeout(&self) -> Duration {
        Duration::from_secs(10)
    }
    fn input_schema(&self) -> InputSchema {
        InputSchema::new(vec![
            ParamSchema::required("query", ParamKind::String),
            ParamSchema::optional("top_k", ParamKind::Integer),
        ])
    }
    fn execute(&self, args: &ToolArgs) -> ToolOutcome {
        let Some(query) = args.get("query").and_then(|v| v.as_str()) else {
            return err_outcome("missing required parameter \"query\"");
        };
        let top_k = args.get("top_k").and_then(|v| v.as_u64()).unwrap_or(5) as usize;

        let kb = match self.kb.lock() {
            Ok(kb) => kb,
            Err(e) => return err_outcome(format!("knowledge base lock was poisoned: {e}")),
        };
        match kb.search(query, top_k) {
            Ok(hits) => ToolOutcome {
                success: true,
                message: format!("found {} matching chunk(s)", hits.len()),
                data: Some(serde_json::json!({
                    "hits": hits.iter().map(|h| serde_json::json!({
                        "chunk_id": h.chunk_id.to_string(),
                        "document_id": h.document_id.to_string(),
                        "document_title": h.document_title,
                        "chunk_index": h.chunk_index,
                        "content": h.content,
                        "score": h.score,
                    })).collect::<Vec<_>>(),
                })),
            },
            Err(e) => err_outcome(e),
        }
    }
}

pub struct ListIndexedDocumentsTool {
    kb: Arc<Mutex<KnowledgeBase>>,
}

impl ListIndexedDocumentsTool {
    pub fn new(kb: Arc<Mutex<KnowledgeBase>>) -> Self {
        Self { kb }
    }
}

impl Tool for ListIndexedDocumentsTool {
    fn name(&self) -> &str {
        "list_indexed_documents"
    }
    fn description(&self) -> &str {
        "Lists every document currently indexed in the local knowledge base, newest first."
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Development
    }
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
    fn timeout(&self) -> Duration {
        Duration::from_secs(10)
    }
    fn input_schema(&self) -> InputSchema {
        InputSchema::new(vec![])
    }
    fn execute(&self, _args: &ToolArgs) -> ToolOutcome {
        let kb = match self.kb.lock() {
            Ok(kb) => kb,
            Err(e) => return err_outcome(format!("knowledge base lock was poisoned: {e}")),
        };
        match kb.list_documents() {
            Ok(documents) => ToolOutcome {
                success: true,
                message: format!("{} document(s) indexed", documents.len()),
                data: Some(serde_json::json!({
                    "documents": documents.iter().map(|d| serde_json::json!({
                        "id": d.id.to_string(),
                        "source_path": d.source_path,
                        "title": d.title,
                        "chunk_count": d.chunk_count,
                    })).collect::<Vec<_>>(),
                })),
            },
            Err(e) => err_outcome(e),
        }
    }
}

pub struct ForgetDocumentTool {
    kb: Arc<Mutex<KnowledgeBase>>,
}

impl ForgetDocumentTool {
    pub fn new(kb: Arc<Mutex<KnowledgeBase>>) -> Self {
        Self { kb }
    }
}

impl Tool for ForgetDocumentTool {
    fn name(&self) -> &str {
        "forget_document"
    }
    fn description(&self) -> &str {
        "Permanently removes a document and all of its indexed chunks from the local knowledge \
         base."
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Development
    }
    /// `Destructive`, matching `delete_file`'s precedent: this is
    /// irreversible data loss (every chunk and embedding for that
    /// document, gone), the same consequence shape TCLIS SS6 already
    /// puts at Level 3 -- not `Moderate`, which is for data *creation*
    /// (`index_document` itself), a meaningfully lower-stakes action
    /// than deleting something that can't be recovered.
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Destructive
    }
    fn timeout(&self) -> Duration {
        Duration::from_secs(10)
    }
    fn input_schema(&self) -> InputSchema {
        InputSchema::new(vec![ParamSchema::required("id", ParamKind::String)])
    }
    fn execute(&self, args: &ToolArgs) -> ToolOutcome {
        let Some(id_str) = args.get("id").and_then(|v| v.as_str()) else {
            return err_outcome("missing required parameter \"id\"");
        };
        let id: Id = match id_str.parse() {
            Ok(id) => id,
            Err(e) => return err_outcome(format!("invalid id {id_str:?}: {e}")),
        };

        let kb = match self.kb.lock() {
            Ok(kb) => kb,
            Err(e) => return err_outcome(format!("knowledge base lock was poisoned: {e}")),
        };
        match kb.forget_document(id) {
            Ok(forgotten) => ToolOutcome {
                success: true,
                message: if forgotten {
                    format!("removed document {id_str}")
                } else {
                    format!("no document found with id {id_str}")
                },
                data: Some(serde_json::json!({ "forgotten": forgotten })),
            },
            Err(e) => err_outcome(e),
        }
    }
}

pub fn register_all(
    registry: &mut tool_framework::ToolRegistry,
    kb: Arc<Mutex<KnowledgeBase>>,
) -> common::AppResult<()> {
    registry.register(Arc::new(IndexDocumentTool::new(kb.clone())))?;
    registry.register(Arc::new(SearchKnowledgeBaseTool::new(kb.clone())))?;
    registry.register(Arc::new(ListIndexedDocumentsTool::new(kb.clone())))?;
    registry.register(Arc::new(ForgetDocumentTool::new(kb)))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use embeddings::HashingEmbedder;
    use tool_framework::{ResultStatus, ToolRegistry, ToolRouter};

    fn router() -> (ToolRouter, Arc<Mutex<KnowledgeBase>>) {
        let kb = Arc::new(Mutex::new(
            KnowledgeBase::in_memory(Box::new(HashingEmbedder::new(64))).unwrap(),
        ));
        let mut registry = ToolRegistry::new();
        register_all(&mut registry, kb.clone()).unwrap();
        (
            ToolRouter::new(registry, PermissionLevel::Administrator),
            kb,
        )
    }

    #[test]
    fn all_tools_are_registered() {
        let (_, kb) = router();
        let mut registry = ToolRegistry::new();
        register_all(&mut registry, kb).unwrap();
        let mut names = registry.list();
        names.sort();
        assert_eq!(
            names,
            vec![
                "forget_document",
                "index_document",
                "list_indexed_documents",
                "search_knowledge_base",
            ]
        );
    }

    #[tokio::test]
    async fn index_search_list_and_forget_work_end_to_end_through_the_router() {
        let (router, _) = router();
        let dir = std::env::temp_dir().join("rag-tools-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("doc.txt");
        std::fs::write(
            &path,
            "a fact about glaciers that nothing else here mentions",
        )
        .unwrap();

        let mut index_args = ToolArgs::new();
        index_args.insert("path".into(), serde_json::json!(path.to_str().unwrap()));
        let index_result = router.execute("index_document", index_args).await;
        assert_eq!(
            index_result.status,
            ResultStatus::Ok,
            "{}",
            index_result.message
        );
        let doc_id = index_result.data.unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();

        let mut search_args = ToolArgs::new();
        search_args.insert("query".into(), serde_json::json!("glaciers"));
        let search_result = router.execute("search_knowledge_base", search_args).await;
        assert_eq!(search_result.status, ResultStatus::Ok);
        assert_eq!(
            search_result.data.unwrap()["hits"]
                .as_array()
                .unwrap()
                .len(),
            1
        );

        let list_result = router
            .execute("list_indexed_documents", ToolArgs::new())
            .await;
        assert_eq!(
            list_result.data.unwrap()["documents"]
                .as_array()
                .unwrap()
                .len(),
            1
        );

        let mut forget_args = ToolArgs::new();
        forget_args.insert("id".into(), serde_json::json!(doc_id));
        let forget_result = router.execute("forget_document", forget_args).await;
        assert_eq!(forget_result.status, ResultStatus::Ok);
        assert_eq!(forget_result.data.unwrap()["forgotten"], true);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn missing_required_params_fail_validation_cleanly() {
        let (router, _) = router();
        assert_eq!(
            router
                .execute("index_document", ToolArgs::new())
                .await
                .status,
            ResultStatus::ValidationFailed
        );
        assert_eq!(
            router
                .execute("search_knowledge_base", ToolArgs::new())
                .await
                .status,
            ResultStatus::ValidationFailed
        );
        assert_eq!(
            router
                .execute("forget_document", ToolArgs::new())
                .await
                .status,
            ResultStatus::ValidationFailed
        );
    }
}
