//! Wraps `tool_framework::ToolRegistry` lookups behind the
//! [`crate::coordinator::Agent`] interface. Ai_reasoning SS15 names a
//! future "Security Agent" without defining its scope -- this crate's
//! own doc comment settles that explicitly too: it reports the real,
//! already-assigned `PermissionLevel` for named tools (the same levels
//! `ToolRouter`/`PermissionManager` actually enforce), not a heuristic
//! or invented risk score. "Is this plan safe" isn't a question this
//! agent tries to answer beyond that -- Ai_reasoning SS17: "must not
//! fabricate completed actions" applies just as much to fabricated
//! *risk assessments* as to fabricated tool results.
//!
//! A natural pairing with [`crate::planner_agent::PlannerAgent`] **in
//! principle**, worth being honest about the current gap in: a
//! `Plan`'s steps each have an optional `tool` field
//! (`PlanStep::tool`) meant for exactly this -- collecting real tool
//! names from a plan to check their risk. `planner::Planner::
//! create_plan` doesn't actually populate it yet, though: picking a
//! *specific* tool (`read_file` vs. `write_file` vs. `list_directory`)
//! out of a broad `Intent` category (`Intent::FileManagement`) needs
//! real language understanding a keyword classifier can't honestly
//! provide -- the same reason `planner`'s own doc comment defers
//! multi-step decomposition to a working `llm-runtime`. This pairing
//! becomes fully real, not just structurally possible, once that
//! unblocks. Until then, this agent is exercised directly with
//! explicit tool names (as its own tests do), not yet fed from a real
//! `Plan`'s steps.
//!
//! # Payload contract
//!
//! Request: `{"tool_names": ["read_file", "run_diagnostics", ...]}`.
//!
//! Reply: `{"per_tool": [{"name", "found": bool, "level": "ReadOnly"|
//! "Safe"|"Moderate"|"Destructive"|"Administrator"|null}, ...],
//! "unknown_tools": [...], "highest_level": ...|null,
//! "requires_confirmation_count": N}` -- `requires_confirmation_count`
//! counts tools above `Safe`, matching `permission-manager`'s own
//! default `auto_approve_at_or_below: Safe` exactly (see
//! `permission_manager::SecurityRules`), not a threshold invented here.

use crate::coordinator::Agent;
use crate::message::AgentMessage;
use crate::module_err;
use common::{AppResult, PermissionLevel};
use std::sync::Arc;
use tool_framework::ToolRegistry;

pub struct SecurityAgent {
    registry: Arc<ToolRegistry>,
}

impl SecurityAgent {
    pub fn new(registry: Arc<ToolRegistry>) -> Self {
        Self { registry }
    }
}

impl Agent for SecurityAgent {
    fn name(&self) -> &str {
        "security"
    }

    fn handle(&self, message: &AgentMessage) -> AppResult<AgentMessage> {
        let tool_names: Vec<String> = message
            .payload
            .get("tool_names")
            .and_then(|v| v.as_array())
            .ok_or_else(|| module_err("payload missing required array field \"tool_names\""))?
            .iter()
            .map(|v| {
                v.as_str()
                    .map(str::to_string)
                    .ok_or_else(|| module_err("every entry in \"tool_names\" must be a string"))
            })
            .collect::<AppResult<Vec<String>>>()?;

        let mut per_tool = Vec::with_capacity(tool_names.len());
        let mut unknown_tools = Vec::new();
        let mut levels: Vec<PermissionLevel> = Vec::new();

        for name in &tool_names {
            match self.registry.get(name) {
                Some(tool) => {
                    let level = tool.permission_level();
                    levels.push(level);
                    per_tool.push(serde_json::json!({
                        "name": name,
                        "found": true,
                        "level": format!("{level:?}"),
                    }));
                }
                None => {
                    unknown_tools.push(name.clone());
                    per_tool.push(serde_json::json!({
                        "name": name,
                        "found": false,
                        "level": null,
                    }));
                }
            }
        }

        let highest_level = levels.iter().max().map(|l| format!("{l:?}"));
        let requires_confirmation_count = levels
            .iter()
            .filter(|l| **l > PermissionLevel::Safe)
            .count();

        Ok(message.reply(serde_json::json!({
            "per_tool": per_tool,
            "unknown_tools": unknown_tools,
            "highest_level": highest_level,
            "requires_confirmation_count": requires_confirmation_count,
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::Priority;
    use linux_services::tools as linux_tools;

    fn test_agent() -> SecurityAgent {
        let mut registry = ToolRegistry::new();
        linux_tools::register_all(&mut registry).unwrap();
        SecurityAgent::new(Arc::new(registry))
    }

    #[test]
    fn reports_the_real_permission_level_of_a_real_tool() {
        let agent = test_agent();
        let message = AgentMessage::new(
            "test",
            "security",
            serde_json::json!({"tool_names": ["read_file"]}),
            Priority::Normal,
        );
        let reply = agent.handle(&message).unwrap();
        assert_eq!(reply.payload["per_tool"][0]["found"], true);
        assert_eq!(reply.payload["per_tool"][0]["level"], "ReadOnly");
        assert_eq!(reply.payload["highest_level"], "ReadOnly");
        assert_eq!(reply.payload["requires_confirmation_count"], 0);
    }

    #[test]
    fn a_mix_of_tools_reports_the_genuinely_highest_level_among_them() {
        let agent = test_agent();
        let message = AgentMessage::new(
            "test",
            "security",
            serde_json::json!({"tool_names": ["read_file", "delete_file"]}),
            Priority::Normal,
        );
        let reply = agent.handle(&message).unwrap();

        // read_file is ReadOnly, delete_file is Destructive (TCLIS SS6) --
        // the real, already-assigned levels, not values invented here.
        assert_eq!(reply.payload["highest_level"], "Destructive");
        assert_eq!(reply.payload["requires_confirmation_count"], 1);
    }

    #[test]
    fn an_unknown_tool_name_is_reported_not_silently_dropped() {
        let agent = test_agent();
        let message = AgentMessage::new(
            "test",
            "security",
            serde_json::json!({"tool_names": ["not_a_real_tool"]}),
            Priority::Normal,
        );
        let reply = agent.handle(&message).unwrap();

        assert_eq!(reply.payload["per_tool"][0]["found"], false);
        assert_eq!(reply.payload["unknown_tools"][0], "not_a_real_tool");
        assert_eq!(reply.payload["highest_level"], serde_json::Value::Null);
    }

    #[test]
    fn empty_tool_list_is_a_clean_empty_report_not_an_error() {
        let agent = test_agent();
        let message = AgentMessage::new(
            "test",
            "security",
            serde_json::json!({"tool_names": []}),
            Priority::Normal,
        );
        let reply = agent.handle(&message).unwrap();
        assert_eq!(reply.payload["requires_confirmation_count"], 0);
        assert_eq!(reply.payload["highest_level"], serde_json::Value::Null);
    }
}
