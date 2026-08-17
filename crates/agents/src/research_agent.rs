//! Wraps `search::SearchEngine` (Development_plan Phase 13) behind the
//! [`crate::coordinator::Agent`] interface. Ai_reasoning SS15 names a
//! future "Research Agent" without defining its scope further -- this
//! crate's own doc comment settles that explicitly: **local research
//! only**. `SearchEngine::search_all` covers files, remembered
//! long-term memories, and this assistant's own registered tools/
//! workflows -- nothing here reaches the network. PRD SS3's own
//! non-goals list "No cloud dependency for the core assistant," and no
//! web-search capability exists anywhere in this workspace for a
//! "Research Agent" to wrap even if it wanted to.
//!
//! `Arc<SearchEngine>`, matching `search::tools::SearchFilesTool`'s own
//! established sharing pattern for this exact type.
//!
//! # Payload contract
//!
//! Request: `{"query": "...", "limit": N}` (`limit` optional, defaults
//! to 10). Reply: `{"hits": [{"kind": "file"|"memory"|"command",
//! "title", "snippet", "score", "source_id"}, ...]}` -- a direct
//! rendering of the real `SearchHit`s `search_all` returns.

use crate::coordinator::Agent;
use crate::message::AgentMessage;
use crate::module_err;
use common::AppResult;
use search::SearchEngine;
use std::sync::Arc;

pub struct ResearchAgent {
    engine: Arc<SearchEngine>,
}

impl ResearchAgent {
    pub fn new(engine: Arc<SearchEngine>) -> Self {
        Self { engine }
    }
}

impl Agent for ResearchAgent {
    fn name(&self) -> &str {
        "research"
    }

    fn handle(&self, message: &AgentMessage) -> AppResult<AgentMessage> {
        let query = message
            .payload
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| module_err("payload missing required string field \"query\""))?;
        let limit = message
            .payload
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(10) as usize;

        let hits = self.engine.search_all(query, limit)?;
        let hits: Vec<serde_json::Value> = hits
            .iter()
            .map(|h| {
                serde_json::json!({
                    "kind": h.kind.as_str(),
                    "title": h.title,
                    "snippet": h.snippet,
                    "score": h.score,
                    "source_id": h.source_id,
                })
            })
            .collect();

        Ok(message.reply(serde_json::json!({ "hits": hits })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::Priority;
    use embeddings::HashingEmbedder;
    use memory::MemoryStore;
    use search::{CommandSearchProvider, FileSearchProvider, MemorySearchProvider};
    use std::sync::Mutex;
    use tool_framework::{stubs::EchoTool, ToolRegistry};

    fn test_agent(dir: &std::path::Path) -> ResearchAgent {
        std::fs::write(dir.join("shared-term-notes.txt"), "").unwrap();

        let memory_store = MemoryStore::in_memory(Box::<HashingEmbedder>::default()).unwrap();
        memory_store
            .remember(
                memory::MemoryCategory::Knowledge,
                "shared-term",
                "a memory that also mentions the shared term",
                5,
            )
            .unwrap();

        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(EchoTool)).unwrap();

        let engine = SearchEngine::new(
            FileSearchProvider::new(vec![dir.to_path_buf()]),
            MemorySearchProvider::new(Arc::new(Mutex::new(memory_store))),
            CommandSearchProvider::new(&registry, None).unwrap(),
        );

        ResearchAgent::new(Arc::new(engine))
    }

    #[test]
    fn finds_real_hits_across_files_and_memory_through_one_message() {
        let dir = std::env::temp_dir().join("research-agent-test");
        std::fs::create_dir_all(&dir).unwrap();
        let agent = test_agent(&dir);

        let message = AgentMessage::new(
            "test",
            "research",
            serde_json::json!({"query": "shared-term", "limit": 10}),
            Priority::Normal,
        );
        let reply = agent.handle(&message).unwrap();
        let hits = reply.payload["hits"].as_array().unwrap();
        let kinds: std::collections::HashSet<&str> =
            hits.iter().map(|h| h["kind"].as_str().unwrap()).collect();

        assert!(kinds.contains("file"), "{hits:?}");
        assert!(kinds.contains("memory"), "{hits:?}");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn respects_the_requested_limit() {
        let dir = std::env::temp_dir().join("research-agent-test-limit");
        std::fs::create_dir_all(&dir).unwrap();
        let agent = test_agent(&dir);

        let message = AgentMessage::new(
            "test",
            "research",
            serde_json::json!({"query": "shared-term", "limit": 1}),
            Priority::Normal,
        );
        let reply = agent.handle(&message).unwrap();
        assert_eq!(reply.payload["hits"].as_array().unwrap().len(), 1);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_query_is_a_clean_error() {
        let dir = std::env::temp_dir().join("research-agent-test-missing-query");
        std::fs::create_dir_all(&dir).unwrap();
        let agent = test_agent(&dir);

        let message =
            AgentMessage::new("test", "research", serde_json::json!({}), Priority::Normal);
        assert!(agent.handle(&message).is_err());

        std::fs::remove_dir_all(&dir).ok();
    }
}
