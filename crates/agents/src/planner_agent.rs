//! Wraps `planner::Planner` (Development_plan Phase 6) behind the
//! [`crate::coordinator::Agent`] interface -- Ai_reasoning SS4's
//! "Planner Agent: Break large tasks into steps, Select tools, Estimate
//! complexity, Retry failed steps where appropriate" is exactly what
//! `planner::Planner::create_plan` already does; this module adds
//! nothing to that capability beyond a message-shaped front door.
//!
//! # Payload contract
//!
//! Request: `{"input": "<user request text>"}`.
//!
//! Reply: `{"goal": ..., "priority": "Low"|"Normal"|"High",
//! "step_count": N, "steps": [{"id", "description", "tool",
//! "depends_on", "requires_confirmation"}, ...]}` -- a direct,
//! field-for-field JSON rendering of the real `planner::Plan` returned,
//! not a summary or reinterpretation of it.

use crate::coordinator::Agent;
use crate::message::AgentMessage;
use crate::module_err;
use common::AppResult;
use planner::Planner;
use reasoning::KeywordClassifier;

pub struct PlannerAgent {
    planner: Planner,
}

impl Default for PlannerAgent {
    /// `KeywordClassifier` -- the only real `IntentClassifier` this
    /// workspace has without a working `llm-runtime` (Phase 4's own
    /// unresolved toolchain wall). Matches `planner::Planner`'s own
    /// test setup exactly.
    fn default() -> Self {
        Self {
            planner: Planner::new(Box::new(KeywordClassifier)),
        }
    }
}

impl PlannerAgent {
    pub fn new(planner: Planner) -> Self {
        Self { planner }
    }
}

impl Agent for PlannerAgent {
    fn name(&self) -> &str {
        "planner"
    }

    fn handle(&self, message: &AgentMessage) -> AppResult<AgentMessage> {
        let input = message
            .payload
            .get("input")
            .and_then(|v| v.as_str())
            .ok_or_else(|| module_err("payload missing required string field \"input\""))?;

        let plan = self.planner.create_plan(input);
        let steps: Vec<serde_json::Value> = plan
            .steps()
            .iter()
            .map(|s| {
                serde_json::json!({
                    "id": s.id,
                    "description": s.description,
                    "tool": s.tool,
                    "depends_on": s.depends_on,
                    "requires_confirmation": s.requires_confirmation,
                })
            })
            .collect();

        Ok(message.reply(serde_json::json!({
            "goal": plan.goal,
            "priority": format!("{:?}", plan.priority),
            "step_count": plan.steps().len(),
            "steps": steps,
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::Priority;

    #[test]
    fn a_direct_question_produces_a_real_single_step_plan() {
        let agent = PlannerAgent::default();
        let message = AgentMessage::new(
            "test",
            "planner",
            serde_json::json!({"input": "What is Linux?"}),
            Priority::Normal,
        );
        let reply = agent.handle(&message).unwrap();

        assert_eq!(reply.payload["step_count"], 1);
        assert_eq!(reply.payload["steps"][0]["requires_confirmation"], false);
    }

    #[test]
    fn package_management_requests_come_back_flagged_for_confirmation() {
        let agent = PlannerAgent::default();
        let message = AgentMessage::new(
            "test",
            "planner",
            serde_json::json!({"input": "Install the ripgrep package"}),
            Priority::Normal,
        );
        let reply = agent.handle(&message).unwrap();

        assert_eq!(reply.payload["steps"][0]["requires_confirmation"], true);
    }

    #[test]
    fn missing_input_field_is_a_clean_error_not_a_panic() {
        let agent = PlannerAgent::default();
        let message = AgentMessage::new("test", "planner", serde_json::json!({}), Priority::Normal);
        assert!(agent.handle(&message).is_err());
    }

    #[test]
    fn reply_preserves_the_original_task_id() {
        let agent = PlannerAgent::default();
        let message = AgentMessage::new(
            "test",
            "planner",
            serde_json::json!({"input": "What time is it?"}),
            Priority::Normal,
        );
        let task_id = message.task_id;
        let reply = agent.handle(&message).unwrap();
        assert_eq!(reply.task_id, task_id);
    }
}
