//! Structured inter-agent messages (Ai_reasoning SS13 "Agent
//! Communication": "Agents communicate through structured messages
//! containing: sender, receiver, task identifier, payload, timestamp,
//! priority. This allows future parallel execution and distributed
//! processing.") -- every field below traces to that sentence directly.

use common::{Id, Timestamp};
use serde_json::Value;

/// Reuses `planner::Priority` rather than defining a second, near-
/// identical enum: it already exists (Phase 6), already has exactly the
/// three levels Ai_reasoning SS13 implies without enumerating
/// (`Low`/`Normal`/`High` reads as the obvious default set, and
/// `planner::Plan` already carries one per plan), and a `Plan`
/// [`crate::planner_agent::PlannerAgent`] hands back can flow straight
/// into a message's `priority` with no conversion.
pub use planner::Priority;

/// One message in the agent-communication layer this module's own doc
/// comment quotes. `payload` is deliberately `serde_json::Value` rather
/// than a closed enum of every possible agent request/reply shape --
/// each concrete agent in this crate documents its own expected
/// payload/reply shape (see [`crate::planner_agent`],
/// [`crate::memory_agent`], [`crate::research_agent`],
/// [`crate::code_agent`], [`crate::security_agent`]), the same
/// "payload interpreted by whoever receives it" relationship
/// `tool_framework::ToolArgs` already has with individual tools.
///
/// Not `Serialize`/`Deserialize` itself, even though `payload` is JSON
/// under the hood -- nothing in this phase actually serializes a whole
/// `AgentMessage` (dispatch is in-process, via
/// [`crate::coordinator::Coordinator`]), and `planner::Priority`
/// doesn't derive those traits either. Ai_reasoning SS13's own
/// "distributed processing" framing anticipates a future where that's
/// genuinely needed; adding the derive now, unused, would be exactly
/// the kind of unverified-because-untested surface this project avoids
/// adding ahead of a real need.
#[derive(Debug, Clone)]
pub struct AgentMessage {
    pub sender: String,
    pub receiver: String,
    pub task_id: Id,
    pub payload: Value,
    pub timestamp: Timestamp,
    pub priority: Priority,
}

impl AgentMessage {
    /// Starts a new task: a fresh `task_id`, `timestamp` set to now.
    /// Use [`AgentMessage::reply`] for a response to an existing one,
    /// which keeps the same `task_id` instead.
    pub fn new(
        sender: impl Into<String>,
        receiver: impl Into<String>,
        payload: Value,
        priority: Priority,
    ) -> Self {
        Self {
            sender: sender.into(),
            receiver: receiver.into(),
            task_id: Id::new(),
            payload,
            timestamp: common::now(),
            priority,
        }
    }

    /// Builds a reply to `self`: sender/receiver swapped, same
    /// `task_id` (so a caller can correlate a reply with the request
    /// that produced it), same `priority`, a fresh `timestamp`.
    pub fn reply(&self, payload: Value) -> Self {
        Self {
            sender: self.receiver.clone(),
            receiver: self.sender.clone(),
            task_id: self.task_id,
            payload,
            timestamp: common::now(),
            priority: self.priority,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_message_gets_a_fresh_task_id() {
        let a = AgentMessage::new(
            "coordinator",
            "planner",
            serde_json::json!({}),
            Priority::Normal,
        );
        let b = AgentMessage::new(
            "coordinator",
            "planner",
            serde_json::json!({}),
            Priority::Normal,
        );
        assert_ne!(a.task_id, b.task_id);
    }

    #[test]
    fn reply_swaps_sender_and_receiver_and_keeps_the_task_id() {
        let request = AgentMessage::new(
            "coordinator",
            "planner",
            serde_json::json!({"input": "hello"}),
            Priority::High,
        );
        let response = request.reply(serde_json::json!({"ok": true}));

        assert_eq!(response.sender, "planner");
        assert_eq!(response.receiver, "coordinator");
        assert_eq!(response.task_id, request.task_id);
        assert_eq!(response.priority, Priority::High);
    }
}
