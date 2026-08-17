//! Multi-agent orchestration (Development_plan Phase 25 "Multi-Agent
//! System": Planner Agent, Research Agent, Coding Agent, Memory Agent,
//! Security Agent; Ai_reasoning SS4 "Agent Architecture", SS13 "Agent
//! Communication", SS15 "Extensibility" -- "these can be added without
//! redesigning the Coordinator Agent").
//!
//! # What's genuinely new here, and what's a wrapper
//!
//! [`message`] and [`coordinator`] are this phase's real, new
//! architecture: structured [`message::AgentMessage`]s dispatched by
//! [`coordinator::Coordinator`], deliberately shaped to mirror
//! `tool_framework`'s `Tool`/`ToolRegistry`/`ToolRouter` one layer up
//! (see `coordinator`'s own doc comment for exactly how and why).
//!
//! Every concrete agent, by contrast, wraps an already-real crate
//! rather than reimplementing its capability -- consistent with
//! Ai_reasoning SS10's "never invent tool results" applied one level up
//! ("never invent agent capabilities" that don't trace to real,
//! already-verified code):
//!
//! - [`planner_agent::PlannerAgent`] wraps `planner::Planner` (Phase 6).
//! - [`memory_agent::MemoryAgent`] wraps `memory::MemoryStore` (Phase 5).
//! - [`research_agent::ResearchAgent`] wraps `search::SearchEngine`
//!   (Phase 13) -- and, worth being explicit about given PRD SS3's "no
//!   cloud dependency" non-goal: "research" here means searching this
//!   assistant's own local files, memory, and commands, never the web.
//!   No web-search capability exists anywhere in this workspace to wrap.
//! - [`code_agent::CodeAgent`] wraps `coding_agent`'s real analysis/
//!   diagnostics/generation functions (Phase 24). Named `code_agent`
//!   rather than `coding_agent` specifically to avoid colliding with
//!   the `coding-agent` crate dependency's own name in scope.
//! - [`security_agent::SecurityAgent`] wraps `tool_framework::
//!   ToolRegistry` lookups against real, already-assigned
//!   `PermissionLevel`s -- it reports what a plan's tools are actually
//!   permissioned at, not a heuristic risk score invented for this
//!   phase.

pub mod code_agent;
pub mod coordinator;
pub mod memory_agent;
pub mod message;
pub mod planner_agent;
pub mod research_agent;
pub mod security_agent;

pub(crate) fn module_err(context: impl std::fmt::Display) -> common::AppError {
    common::AppError::Module {
        module: "agents",
        message: context.to_string(),
    }
}
