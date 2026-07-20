//! `workflow-engine` -- named, reusable, persistent sequences of real
//! tool calls: builder, scheduler, task queue, and executor
//! (Development_plan Phase 10; Apis SS15 Workflow API; DBDMS SS3
//! "Workflows"/"Workflow Runs").
//!
//! Runs through `tool-framework`'s real [`tool_framework::ToolRouter`],
//! so a workflow step needing confirmation goes through exactly the same
//! Phase 9 Permission Manager pipeline any other tool call does -- there
//! is no separate, weaker path for workflow-triggered actions.

pub mod control;
pub mod executor;
pub mod run;
pub mod scheduler;
pub mod store;
pub mod workflow;

pub use control::RunControl;
pub use executor::run_workflow;
pub use run::{RunStatus, StepOutcome, WorkflowRun};
pub use scheduler::{Schedule, Scheduler};
pub use store::WorkflowStore;
pub use workflow::{StepCondition, Workflow, WorkflowError, WorkflowStep};
