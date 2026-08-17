//! The `Agent` trait and [`Coordinator`] that dispatches
//! [`AgentMessage`]s to registered agents by name (Ai_reasoning SS4
//! "Coordinator Agent": "Accept user requests, Coordinate all other
//! agents, Build final response").
//!
//! Deliberately shaped to mirror `tool_framework::{Tool, ToolRegistry,
//! ToolRouter}` one layer up -- lookup by name, `spawn_blocking` +
//! enforced timeout, a structured result that's never a bare Rust
//! error -- rather than inventing a different pattern for what's
//! structurally the same problem (route a named request to the right
//! handler, bound how long it's allowed to take, report what happened).
//! `Coordinator::dispatch` even reuses `ToolRouter::execute`'s exact
//! three-way `Ok(Ok(_))`/`Ok(Err(_))`/`Err(_elapsed)` match against
//! `tokio::time::timeout(..., spawn_blocking(...))` for the same
//! reason: it already correctly distinguishes "the handler ran and
//! failed" from "the handler panicked" from "the handler never finished
//! in time," and there's no reason to re-derive that here.
//!
//! # What this deliberately doesn't claim
//!
//! Ai_reasoning SS4's Coordinator Agent responsibilities also include
//! "Build final response" -- full request-to-response orchestration.
//! That's `assistant_core::AssistantCore::process_message`'s job for
//! the existing conversation pipeline, and this phase doesn't merge the
//! two. [`Coordinator`] is the message-routing substrate a future
//! integration would use, not a replacement for the pipeline that
//! already exists -- said explicitly rather than implied by silently
//! not mentioning it.

use crate::message::AgentMessage;
use common::AppResult;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// One participant in the multi-agent system. `handle` is synchronous
/// and takes `&self`, matching `tool_framework::Tool::execute`'s exact
/// reasoning (no agent here does true async I/O -- `MemoryStore`,
/// `SearchEngine`, and `coding_agent`'s functions are all blocking
/// calls under the hood) -- [`Coordinator`] provides the async layer on
/// top, the same split `ToolRouter` already has with `Tool`.
pub trait Agent: Send + Sync {
    fn name(&self) -> &str;
    /// How long [`Coordinator::dispatch`] will wait before treating this
    /// agent as unresponsive. Defaults to 30 seconds -- generous enough
    /// for `code_agent`'s `run_diagnostics` path (a real `cargo check`),
    /// the slowest real operation any agent in this crate performs.
    fn timeout(&self) -> Duration {
        Duration::from_secs(30)
    }
    fn handle(&self, message: &AgentMessage) -> AppResult<AgentMessage>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchStatus {
    Ok,
    UnknownAgent,
    TimedOut,
    Failed,
}

/// Structured outcome of one dispatch -- never a bare Rust `Result`
/// error reaching the caller, the identical "structured outcome, not
/// exceptional control flow" reasoning `tool_framework::ToolResult`'s
/// own doc comment gives for TCLIS SS10.
#[derive(Debug, Clone)]
pub struct DispatchResult {
    pub status: DispatchStatus,
    pub reply: Option<AgentMessage>,
    pub error: Option<String>,
    pub elapsed: Duration,
}

pub struct Coordinator {
    agents: HashMap<String, Arc<dyn Agent>>,
}

impl Default for Coordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl Coordinator {
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
        }
    }

    /// Registers `agent` under its own `name()`. A second registration
    /// under the same name replaces the first, the same "last write
    /// wins, no error" behavior `tool_framework::ToolRegistry::register`
    /// deliberately does *not* have (it errors on a duplicate name) --
    /// worth calling out as a genuine, considered difference: tools are
    /// typically registered once at startup from a fixed set of crates,
    /// where a silent duplicate is almost certainly a bug, while agents
    /// are few enough and named deliberately enough (by role, not by a
    /// generated or user-supplied name) that a caller intentionally
    /// swapping one out (e.g. a test double for `MemoryAgent`) is a
    /// real, legitimate use this shouldn't get in the way of.
    pub fn register(&mut self, agent: Arc<dyn Agent>) {
        self.agents.insert(agent.name().to_string(), agent);
    }

    pub fn list_agents(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.agents.keys().map(String::as_str).collect();
        names.sort_unstable();
        names
    }

    /// Routes `message` to the agent named in `message.receiver`.
    pub async fn dispatch(&self, message: AgentMessage) -> DispatchResult {
        let started = Instant::now();

        let Some(agent) = self.agents.get(&message.receiver).cloned() else {
            return DispatchResult {
                status: DispatchStatus::UnknownAgent,
                reply: None,
                error: Some(format!("no agent registered named {:?}", message.receiver)),
                elapsed: started.elapsed(),
            };
        };

        let timeout_duration = agent.timeout();
        let execution = {
            let agent = agent.clone();
            let message = message.clone();
            tokio::task::spawn_blocking(move || agent.handle(&message))
        };

        match tokio::time::timeout(timeout_duration, execution).await {
            Ok(Ok(Ok(reply))) => DispatchResult {
                status: DispatchStatus::Ok,
                reply: Some(reply),
                error: None,
                elapsed: started.elapsed(),
            },
            Ok(Ok(Err(app_err))) => DispatchResult {
                status: DispatchStatus::Failed,
                reply: None,
                error: Some(app_err.to_string()),
                elapsed: started.elapsed(),
            },
            Ok(Err(join_error)) => DispatchResult {
                status: DispatchStatus::Failed,
                reply: None,
                error: Some(format!(
                    "agent {:?} panicked during handling: {join_error}",
                    message.receiver
                )),
                elapsed: started.elapsed(),
            },
            Err(_elapsed) => DispatchResult {
                status: DispatchStatus::TimedOut,
                reply: None,
                error: Some(format!(
                    "agent {:?} did not respond within {timeout_duration:?}",
                    message.receiver
                )),
                elapsed: started.elapsed(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::Priority;

    struct EchoAgent;
    impl Agent for EchoAgent {
        fn name(&self) -> &str {
            "echo"
        }
        fn handle(&self, message: &AgentMessage) -> AppResult<AgentMessage> {
            Ok(message.reply(message.payload.clone()))
        }
    }

    struct AlwaysFailsAgent;
    impl Agent for AlwaysFailsAgent {
        fn name(&self) -> &str {
            "always_fails"
        }
        fn handle(&self, _message: &AgentMessage) -> AppResult<AgentMessage> {
            Err(common::AppError::Module {
                module: "agents",
                message: "deliberately fails for testing".to_string(),
            })
        }
    }

    struct SlowAgent;
    impl Agent for SlowAgent {
        fn name(&self) -> &str {
            "slow"
        }
        fn timeout(&self) -> Duration {
            Duration::from_millis(20)
        }
        fn handle(&self, message: &AgentMessage) -> AppResult<AgentMessage> {
            std::thread::sleep(Duration::from_millis(200));
            Ok(message.reply(serde_json::json!({})))
        }
    }

    struct PanicsAgent;
    impl Agent for PanicsAgent {
        fn name(&self) -> &str {
            "panics"
        }
        fn handle(&self, _message: &AgentMessage) -> AppResult<AgentMessage> {
            panic!("deliberate panic for testing");
        }
    }

    fn coordinator_with(agent: Arc<dyn Agent>) -> Coordinator {
        let mut c = Coordinator::new();
        c.register(agent);
        c
    }

    #[tokio::test]
    async fn dispatches_to_the_named_agent_and_gets_a_real_reply() {
        let coordinator = coordinator_with(Arc::new(EchoAgent));
        let message = AgentMessage::new(
            "test",
            "echo",
            serde_json::json!({"hello": "world"}),
            Priority::Normal,
        );
        let result = coordinator.dispatch(message).await;

        assert_eq!(result.status, DispatchStatus::Ok);
        let reply = result.reply.unwrap();
        assert_eq!(reply.payload, serde_json::json!({"hello": "world"}));
        assert_eq!(reply.sender, "echo");
        assert_eq!(reply.receiver, "test");
    }

    #[tokio::test]
    async fn dispatching_to_an_unregistered_agent_is_a_clean_status_not_a_panic() {
        let coordinator = Coordinator::new();
        let message = AgentMessage::new("test", "nobody", serde_json::json!({}), Priority::Normal);
        let result = coordinator.dispatch(message).await;

        assert_eq!(result.status, DispatchStatus::UnknownAgent);
        assert!(result.reply.is_none());
        assert!(result.error.unwrap().contains("nobody"));
    }

    #[tokio::test]
    async fn an_agent_returning_a_real_error_is_reported_as_failed_not_a_panic() {
        let coordinator = coordinator_with(Arc::new(AlwaysFailsAgent));
        let message = AgentMessage::new(
            "test",
            "always_fails",
            serde_json::json!({}),
            Priority::Normal,
        );
        let result = coordinator.dispatch(message).await;

        assert_eq!(result.status, DispatchStatus::Failed);
        assert!(result.error.unwrap().contains("deliberately fails"));
    }

    #[tokio::test]
    async fn an_agent_that_panics_is_isolated_and_reported_not_a_crash() {
        let coordinator = coordinator_with(Arc::new(PanicsAgent));
        let message = AgentMessage::new("test", "panics", serde_json::json!({}), Priority::Normal);
        let result = coordinator.dispatch(message).await;

        assert_eq!(result.status, DispatchStatus::Failed);
        assert!(result.error.unwrap().contains("panicked"));
    }

    #[tokio::test]
    async fn an_agent_that_runs_too_long_is_reported_as_timed_out() {
        let coordinator = coordinator_with(Arc::new(SlowAgent));
        let message = AgentMessage::new("test", "slow", serde_json::json!({}), Priority::Normal);
        let result = coordinator.dispatch(message).await;

        assert_eq!(result.status, DispatchStatus::TimedOut);
    }

    #[test]
    fn list_agents_is_sorted_and_reflects_registrations() {
        let mut coordinator = Coordinator::new();
        coordinator.register(Arc::new(SlowAgent));
        coordinator.register(Arc::new(EchoAgent));
        assert_eq!(coordinator.list_agents(), vec!["echo", "slow"]);
    }

    #[test]
    fn registering_a_second_agent_under_the_same_name_replaces_the_first() {
        let mut coordinator = Coordinator::new();
        coordinator.register(Arc::new(EchoAgent));
        coordinator.register(Arc::new(EchoAgent));
        assert_eq!(coordinator.list_agents(), vec!["echo"]);
    }
}
