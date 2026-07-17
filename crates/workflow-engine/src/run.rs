//! Workflow run tracking (DBDMS SS3 "Workflow Runs": id, workflow_id,
//! started, finished, result).

use common::{Id, Timestamp};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStatus {
    Completed,
    Failed,
    Cancelled,
}

impl RunStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            RunStatus::Completed => "completed",
            RunStatus::Failed => "failed",
            RunStatus::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone)]
pub struct StepOutcome {
    pub step_id: usize,
    pub success: bool,
    pub skipped: bool,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct WorkflowRun {
    pub id: Id,
    pub workflow_id: Id,
    pub status: RunStatus,
    pub step_outcomes: Vec<StepOutcome>,
    pub started_at: Timestamp,
    pub finished_at: Timestamp,
}
