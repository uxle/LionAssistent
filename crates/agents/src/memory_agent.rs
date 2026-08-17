//! Wraps `memory::MemoryStore` (Development_plan Phase 5) behind the
//! [`crate::coordinator::Agent`] interface -- Ai_reasoning SS4's "Memory
//! Agent: Retrieve relevant memories, Store important information,
//! ... Forget outdated information based on policy" maps directly onto
//! `MemoryStore::search_memory`/`remember`/`forget`, already real.
//!
//! `Arc<Mutex<MemoryStore>>`, not a bare `MemoryStore`: matches
//! `search::MemorySearchProvider`'s own established sharing pattern for
//! this exact type (see that module) rather than inventing a different
//! one.
//!
//! # Payload contract
//!
//! Request `{"action": "remember", "category": "preference"|"workflow"|
//! "reminder"|"knowledge"|"project"|"custom", "title", "content",
//! "importance"}` -> reply `{"id": "<uuid>"}`.
//!
//! Request `{"action": "recall", "query", "top_k"}` -> reply
//! `{"items": [{"id", "category", "title", "content", "importance",
//! "confidence"}, ...]}`.
//!
//! Request `{"action": "forget", "id": "<uuid>"}` -> reply
//! `{"forgotten": true|false}` -- `false` for an unknown id is a real,
//! valid outcome (`MemoryStore::forget`'s own documented behavior), not
//! an error.

use crate::coordinator::Agent;
use crate::message::AgentMessage;
use crate::module_err;
use common::{AppResult, Id};
use memory::{MemoryCategory, MemoryStore};
use std::str::FromStr;
use std::sync::{Arc, Mutex};

pub struct MemoryAgent {
    store: Arc<Mutex<MemoryStore>>,
}

impl MemoryAgent {
    pub fn new(store: Arc<Mutex<MemoryStore>>) -> Self {
        Self { store }
    }
}

fn get_str<'a>(payload: &'a serde_json::Value, field: &str) -> AppResult<&'a str> {
    payload
        .get(field)
        .and_then(|v| v.as_str())
        .ok_or_else(|| module_err(format!("payload missing required string field {field:?}")))
}

impl Agent for MemoryAgent {
    fn name(&self) -> &str {
        "memory"
    }

    fn handle(&self, message: &AgentMessage) -> AppResult<AgentMessage> {
        let action = get_str(&message.payload, "action")?;
        let store = self
            .store
            .lock()
            .map_err(|e| module_err(format!("memory store lock was poisoned: {e}")))?;

        match action {
            "remember" => {
                let category_str = get_str(&message.payload, "category")?;
                let category = MemoryCategory::from_str(category_str)
                    .map_err(|e| module_err(format!("invalid category {category_str:?}: {e}")))?;
                let title = get_str(&message.payload, "title")?;
                let content = get_str(&message.payload, "content")?;
                let importance = message
                    .payload
                    .get("importance")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| {
                        module_err("payload missing required integer field \"importance\"")
                    })? as u8;

                let id = store.remember(category, title, content, importance)?;
                Ok(message.reply(serde_json::json!({ "id": id.to_string() })))
            }
            "recall" => {
                let query = get_str(&message.payload, "query")?;
                let top_k = message
                    .payload
                    .get("top_k")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(5) as usize;

                let items = store.search_memory(query, top_k)?;
                let items: Vec<serde_json::Value> = items
                    .iter()
                    .map(|m| {
                        serde_json::json!({
                            "id": m.id.to_string(),
                            "category": m.category.as_str(),
                            "title": m.title,
                            "content": m.content,
                            "importance": m.importance,
                            "confidence": m.confidence,
                        })
                    })
                    .collect();
                Ok(message.reply(serde_json::json!({ "items": items })))
            }
            "forget" => {
                let id_str = get_str(&message.payload, "id")?;
                let id = Id::from_str(id_str)
                    .map_err(|e| module_err(format!("invalid id {id_str:?}: {e}")))?;
                let forgotten = store.forget(id)?;
                Ok(message.reply(serde_json::json!({ "forgotten": forgotten })))
            }
            other => Err(module_err(format!(
                "unknown action {other:?} -- expected \"remember\", \"recall\", or \"forget\""
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::Priority;
    use embeddings::HashingEmbedder;

    fn test_agent() -> MemoryAgent {
        let store = MemoryStore::in_memory(Box::<HashingEmbedder>::default()).unwrap();
        MemoryAgent::new(Arc::new(Mutex::new(store)))
    }

    #[test]
    fn remembers_and_recalls_a_real_memory_round_trip() {
        let agent = test_agent();

        let remember_msg = AgentMessage::new(
            "test",
            "memory",
            serde_json::json!({
                "action": "remember",
                "category": "preference",
                "title": "editor",
                "content": "the user prefers vim keybindings",
                "importance": 5,
            }),
            Priority::Normal,
        );
        let remember_reply = agent.handle(&remember_msg).unwrap();
        assert!(remember_reply.payload["id"].as_str().is_some());

        let recall_msg = AgentMessage::new(
            "test",
            "memory",
            serde_json::json!({"action": "recall", "query": "keybindings", "top_k": 5}),
            Priority::Normal,
        );
        let recall_reply = agent.handle(&recall_msg).unwrap();
        let items = recall_reply.payload["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["title"], "editor");
    }

    #[test]
    fn forgets_a_real_memory() {
        let agent = test_agent();
        let remember_msg = AgentMessage::new(
            "test",
            "memory",
            serde_json::json!({
                "action": "remember",
                "category": "knowledge",
                "title": "fact",
                "content": "some fact",
                "importance": 3,
            }),
            Priority::Normal,
        );
        let id = agent.handle(&remember_msg).unwrap().payload["id"]
            .as_str()
            .unwrap()
            .to_string();

        let forget_msg = AgentMessage::new(
            "test",
            "memory",
            serde_json::json!({"action": "forget", "id": id}),
            Priority::Normal,
        );
        let forget_reply = agent.handle(&forget_msg).unwrap();
        assert_eq!(forget_reply.payload["forgotten"], true);
    }

    #[test]
    fn forgetting_an_unknown_id_is_a_real_false_not_an_error() {
        let agent = test_agent();
        let forget_msg = AgentMessage::new(
            "test",
            "memory",
            serde_json::json!({"action": "forget", "id": Id::new().to_string()}),
            Priority::Normal,
        );
        let reply = agent.handle(&forget_msg).unwrap();
        assert_eq!(reply.payload["forgotten"], false);
    }

    #[test]
    fn an_invalid_category_is_a_clean_error() {
        let agent = test_agent();
        let message = AgentMessage::new(
            "test",
            "memory",
            serde_json::json!({
                "action": "remember",
                "category": "not_a_real_category",
                "title": "x",
                "content": "y",
                "importance": 1,
            }),
            Priority::Normal,
        );
        assert!(agent.handle(&message).is_err());
    }

    #[test]
    fn an_unknown_action_is_a_clean_error() {
        let agent = test_agent();
        let message = AgentMessage::new(
            "test",
            "memory",
            serde_json::json!({"action": "levitate"}),
            Priority::Normal,
        );
        assert!(agent.handle(&message).is_err());
    }
}
