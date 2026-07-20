//! Actually running a [`Workflow`](crate::Workflow) (Apis SS15
//! `execute_workflow`), through `tool-framework`'s real
//! [`tool_framework::ToolRouter`] -- the same pipeline Phase 9's
//! Permission Manager gates, so a workflow step requiring confirmation
//! goes through exactly the same authorization every other tool call
//! does. No special-cased "workflows bypass permissions" path exists.
//!
//! Stages from [`Workflow::parallel_stages`] are run in order, but
//! **steps within a stage are still run one at a time** here, not
//! concurrently. `parallel_stages` proves *which* steps have no
//! ordering dependency on each other -- genuinely useful information for
//! a future concurrent executor -- but dispatching them concurrently
//! today would mean either adding a new dependency (`futures::join_all`)
//! or wrapping `ToolRouter` in `Arc` to share across `tokio::spawn`ed
//! tasks, for a capability nothing in this codebase needs yet (every
//! registered tool so far completes in well under a second). Sequential
//! execution is correct, simpler, and easy to upgrade later without
//! changing `parallel_stages` at all.

use crate::control::RunControl;
use crate::run::{RunStatus, StepOutcome, WorkflowRun};
use crate::workflow::{StepCondition, Workflow};
use common::{now, Id};
use std::collections::HashSet;
use tool_framework::ToolRouter;

pub async fn run_workflow(
    workflow: &Workflow,
    router: &ToolRouter,
    control: &RunControl,
) -> WorkflowRun {
    let started_at = now();
    let run_id = Id::new();

    let stages = match workflow.parallel_stages() {
        Ok(stages) => stages,
        Err(e) => {
            return WorkflowRun {
                id: run_id,
                workflow_id: workflow.id,
                status: RunStatus::Failed,
                step_outcomes: vec![StepOutcome {
                    step_id: usize::MAX,
                    success: false,
                    skipped: false,
                    message: format!("workflow is not valid: {e}"),
                }],
                started_at,
                finished_at: now(),
            };
        }
    };

    let mut outcomes: Vec<StepOutcome> = Vec::new();
    let mut ran: HashSet<usize> = HashSet::new();
    let mut succeeded: HashSet<usize> = HashSet::new();
    let mut any_failed = false;
    let mut cancelled = false;

    'stages: for stage in stages {
        for step_id in stage {
            if !control.proceed().await {
                cancelled = true;
                break 'stages;
            }

            let step = &workflow.steps()[step_id];
            let should_run = match step.condition {
                StepCondition::Always => true,
                StepCondition::IfStepSucceeded(dep) => succeeded.contains(&dep),
                StepCondition::IfStepFailed(dep) => ran.contains(&dep) && !succeeded.contains(&dep),
            };

            if !should_run {
                outcomes.push(StepOutcome {
                    step_id,
                    success: false,
                    skipped: true,
                    message: "skipped: condition not met".to_string(),
                });
                continue;
            }

            let result = router.execute(&step.tool_name, step.args.clone()).await;
            ran.insert(step_id);
            if result.success {
                succeeded.insert(step_id);
            } else {
                any_failed = true;
            }
            outcomes.push(StepOutcome {
                step_id,
                success: result.success,
                skipped: false,
                message: result.message,
            });
        }
    }

    let status = if cancelled {
        RunStatus::Cancelled
    } else if any_failed {
        RunStatus::Failed
    } else {
        RunStatus::Completed
    };

    WorkflowRun {
        id: run_id,
        workflow_id: workflow.id,
        status,
        step_outcomes: outcomes,
        started_at,
        finished_at: now(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::StepCondition;
    use std::sync::Arc;
    use tool_framework::{PermissionLevel, ToolArgs, ToolRegistry};

    fn echo_router() -> ToolRouter {
        let mut registry = ToolRegistry::new();
        registry
            .register(Arc::new(tool_framework::stubs::EchoTool))
            .unwrap();
        ToolRouter::new(registry, PermissionLevel::Safe)
    }

    fn echo_args(text: &str) -> ToolArgs {
        let mut args = ToolArgs::new();
        args.insert("text".into(), serde_json::json!(text));
        args
    }

    #[tokio::test]
    async fn a_simple_workflow_completes() {
        let mut wf = Workflow::new("greet", "says hello twice");
        wf.add_step("echo", echo_args("hello"), vec![], StepCondition::Always);
        wf.add_step("echo", echo_args("world"), vec![], StepCondition::Always);

        let run = run_workflow(&wf, &echo_router(), &RunControl::new()).await;
        assert_eq!(run.status, RunStatus::Completed);
        assert_eq!(run.step_outcomes.len(), 2);
        assert!(run.step_outcomes.iter().all(|o| o.success));
    }

    #[tokio::test]
    async fn a_missing_tool_fails_the_run() {
        let mut wf = Workflow::new("broken", "");
        wf.add_step(
            "nonexistent-tool",
            ToolArgs::new(),
            vec![],
            StepCondition::Always,
        );

        let run = run_workflow(&wf, &echo_router(), &RunControl::new()).await;
        assert_eq!(run.status, RunStatus::Failed);
    }

    #[tokio::test]
    async fn if_step_failed_condition_runs_only_after_a_real_failure() {
        let mut wf = Workflow::new("fallback", "");
        let missing = wf.add_step(
            "nonexistent-tool",
            ToolArgs::new(),
            vec![],
            StepCondition::Always,
        );
        wf.add_step(
            "echo",
            echo_args("cleanup"),
            vec![missing],
            StepCondition::IfStepFailed(missing),
        );

        let run = run_workflow(&wf, &echo_router(), &RunControl::new()).await;
        assert_eq!(run.step_outcomes.len(), 2);
        assert!(!run.step_outcomes[0].success);
        assert!(run.step_outcomes[1].success); // cleanup ran because the first step failed
        assert!(!run.step_outcomes[1].skipped);
    }

    #[tokio::test]
    async fn if_step_succeeded_condition_is_skipped_after_a_failure() {
        let mut wf = Workflow::new("skip-on-failure", "");
        let missing = wf.add_step(
            "nonexistent-tool",
            ToolArgs::new(),
            vec![],
            StepCondition::Always,
        );
        wf.add_step(
            "echo",
            echo_args("only if ok"),
            vec![missing],
            StepCondition::IfStepSucceeded(missing),
        );

        let run = run_workflow(&wf, &echo_router(), &RunControl::new()).await;
        assert!(run.step_outcomes[1].skipped);
    }

    #[tokio::test]
    async fn cancelling_stops_the_run_before_remaining_steps() {
        let mut wf = Workflow::new("long", "");
        wf.add_step("echo", echo_args("first"), vec![], StepCondition::Always);
        wf.add_step("echo", echo_args("second"), vec![], StepCondition::Always);

        let control = RunControl::new();
        control.cancel();

        let run = run_workflow(&wf, &echo_router(), &control).await;
        assert_eq!(run.status, RunStatus::Cancelled);
        assert_eq!(run.step_outcomes.len(), 0);
    }

    #[tokio::test]
    async fn an_invalid_workflow_fails_immediately_with_a_clear_message() {
        let mut wf = Workflow::new("bad", "");
        wf.add_step("echo", ToolArgs::new(), vec![99], StepCondition::Always);

        let run = run_workflow(&wf, &echo_router(), &RunControl::new()).await;
        assert_eq!(run.status, RunStatus::Failed);
        assert!(run.step_outcomes[0].message.contains("not valid"));
    }
}
