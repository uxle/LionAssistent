//! Persistence for workflows and their run history (DBDMS SS3
//! "Workflows"/"Workflow Runs"). Same `rusqlite` pin and pattern as
//! `memory` (Phase 5) and `permission-manager` (Phase 9).

use crate::run::{RunStatus, WorkflowRun};
use crate::workflow::{Workflow, WorkflowStep};
use common::{AppError, AppResult, Id, Timestamp};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

fn create_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS workflows (
            id          TEXT PRIMARY KEY,
            name        TEXT NOT NULL,
            description TEXT NOT NULL,
            enabled     INTEGER NOT NULL,
            steps_json  TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS workflow_runs (
            id            TEXT PRIMARY KEY,
            workflow_id   TEXT NOT NULL REFERENCES workflows(id),
            status        TEXT NOT NULL,
            started_at    TEXT NOT NULL,
            finished_at   TEXT NOT NULL,
            outcomes_json TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_workflow_runs_workflow_id ON workflow_runs(workflow_id);
        ",
    )
}

fn db_err(e: rusqlite::Error) -> AppError {
    AppError::Module {
        module: "workflow-engine",
        message: e.to_string(),
    }
}

fn json_err(e: serde_json::Error) -> AppError {
    AppError::Module {
        module: "workflow-engine",
        message: e.to_string(),
    }
}

fn format_timestamp(t: Timestamp) -> String {
    t.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
}

fn parse_timestamp(s: &str) -> AppResult<Timestamp> {
    s.parse().map_err(|e: chrono::ParseError| AppError::Module {
        module: "workflow-engine",
        message: format!("invalid timestamp {s:?}: {e}"),
    })
}

fn parse_id(s: &str) -> AppResult<Id> {
    s.parse().map_err(|e: uuid::Error| AppError::Module {
        module: "workflow-engine",
        message: format!("invalid id {s:?}: {e}"),
    })
}

pub struct WorkflowStore {
    conn: Connection,
}

impl WorkflowStore {
    pub fn open(path: impl AsRef<Path>) -> AppResult<Self> {
        let conn = Connection::open(path).map_err(db_err)?;
        create_schema(&conn).map_err(db_err)?;
        Ok(Self { conn })
    }

    pub fn in_memory() -> AppResult<Self> {
        let conn = Connection::open_in_memory().map_err(db_err)?;
        create_schema(&conn).map_err(db_err)?;
        Ok(Self { conn })
    }

    /// Saves (or updates, if already saved) a workflow's definition.
    pub fn save_workflow(&self, workflow: &Workflow) -> AppResult<()> {
        let steps_json = serde_json::to_string(workflow.steps()).map_err(json_err)?;
        self.conn
            .execute(
                "INSERT INTO workflows (id, name, description, enabled, steps_json)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(id) DO UPDATE SET
                     name = excluded.name,
                     description = excluded.description,
                     enabled = excluded.enabled,
                     steps_json = excluded.steps_json",
                params![
                    workflow.id.to_string(),
                    workflow.name,
                    workflow.description,
                    workflow.enabled as i64,
                    steps_json
                ],
            )
            .map_err(db_err)?;
        Ok(())
    }

    pub fn load_workflow(&self, id: Id) -> AppResult<Option<Workflow>> {
        let row: Option<(String, String, i64, String)> = self
            .conn
            .query_row(
                "SELECT name, description, enabled, steps_json FROM workflows WHERE id = ?1",
                params![id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()
            .map_err(db_err)?;

        let Some((name, description, enabled, steps_json)) = row else {
            return Ok(None);
        };
        let steps: Vec<WorkflowStep> = serde_json::from_str(&steps_json).map_err(json_err)?;
        Ok(Some(Workflow::from_parts(
            id,
            name,
            description,
            enabled != 0,
            steps,
        )))
    }

    /// `(id, name)` pairs for every saved workflow, alphabetical by name.
    pub fn list_workflows(&self) -> AppResult<Vec<(Id, String)>> {
        let rows: Vec<(String, String)> = self
            .conn
            .prepare("SELECT id, name FROM workflows ORDER BY name ASC")
            .map_err(db_err)?
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(db_err)?
            .collect::<Result<_, _>>()
            .map_err(db_err)?;

        rows.into_iter()
            .map(|(id, name)| Ok((parse_id(&id)?, name)))
            .collect()
    }

    pub fn delete_workflow(&self, id: Id) -> AppResult<bool> {
        let changed = self
            .conn
            .execute(
                "DELETE FROM workflows WHERE id = ?1",
                params![id.to_string()],
            )
            .map_err(db_err)?;
        Ok(changed > 0)
    }

    /// Records a completed run (Apis SS15 `workflow_history`).
    pub fn save_run(&self, run: &WorkflowRun) -> AppResult<()> {
        let outcomes: Vec<serde_json::Value> = run
            .step_outcomes
            .iter()
            .map(|o| serde_json::json!({ "step_id": o.step_id, "success": o.success, "skipped": o.skipped, "message": o.message }))
            .collect();
        let outcomes_json = serde_json::to_string(&outcomes).map_err(json_err)?;

        self.conn
            .execute(
                "INSERT INTO workflow_runs (id, workflow_id, status, started_at, finished_at, outcomes_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    run.id.to_string(),
                    run.workflow_id.to_string(),
                    run.status.as_str(),
                    format_timestamp(run.started_at),
                    format_timestamp(run.finished_at),
                    outcomes_json,
                ],
            )
            .map_err(db_err)?;
        Ok(())
    }

    /// `(run_id, status, started_at)` for a workflow's most recent runs,
    /// newest first (Apis SS15 `workflow_history`).
    pub fn workflow_history(
        &self,
        workflow_id: Id,
        limit: usize,
    ) -> AppResult<Vec<(Id, RunStatus, Timestamp)>> {
        let rows: Vec<(String, String, String)> = self
            .conn
            .prepare(
                "SELECT id, status, started_at FROM workflow_runs
                 WHERE workflow_id = ?1 ORDER BY started_at DESC LIMIT ?2",
            )
            .map_err(db_err)?
            .query_map(params![workflow_id.to_string(), limit as i64], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })
            .map_err(db_err)?
            .collect::<Result<_, _>>()
            .map_err(db_err)?;

        rows.into_iter()
            .map(|(id, status, started_at)| {
                let status = match status.as_str() {
                    "completed" => RunStatus::Completed,
                    "failed" => RunStatus::Failed,
                    "cancelled" => RunStatus::Cancelled,
                    other => {
                        return Err(AppError::Module {
                            module: "workflow-engine",
                            message: format!("unknown run status in db: {other:?}"),
                        })
                    }
                };
                Ok((parse_id(&id)?, status, parse_timestamp(&started_at)?))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run::StepOutcome;
    use crate::workflow::StepCondition;
    use tool_framework::ToolArgs;

    #[test]
    fn round_trips_a_workflow_with_steps() {
        let store = WorkflowStore::in_memory().unwrap();
        let mut wf = Workflow::new("backup", "backs things up");
        let mut args = ToolArgs::new();
        args.insert("path".into(), serde_json::json!("/tmp/data"));
        wf.add_step("list_directory", args, vec![], StepCondition::Always);
        store.save_workflow(&wf).unwrap();

        let loaded = store.load_workflow(wf.id).unwrap().unwrap();
        assert_eq!(loaded.name, "backup");
        assert_eq!(loaded.steps().len(), 1);
        assert_eq!(loaded.steps()[0].tool_name, "list_directory");
    }

    #[test]
    fn loading_an_unknown_workflow_returns_none() {
        let store = WorkflowStore::in_memory().unwrap();
        assert!(store.load_workflow(Id::new()).unwrap().is_none());
    }

    #[test]
    fn saving_twice_updates_rather_than_duplicates() {
        let store = WorkflowStore::in_memory().unwrap();
        let mut wf = Workflow::new("v1", "first version");
        store.save_workflow(&wf).unwrap();
        wf.name = "v2".to_string();
        store.save_workflow(&wf).unwrap();

        assert_eq!(store.list_workflows().unwrap().len(), 1);
        assert_eq!(store.load_workflow(wf.id).unwrap().unwrap().name, "v2");
    }

    #[test]
    fn delete_workflow_removes_it() {
        let store = WorkflowStore::in_memory().unwrap();
        let wf = Workflow::new("temp", "");
        store.save_workflow(&wf).unwrap();
        assert!(store.delete_workflow(wf.id).unwrap());
        assert!(store.load_workflow(wf.id).unwrap().is_none());
    }

    #[test]
    fn records_and_lists_run_history_newest_first() {
        let store = WorkflowStore::in_memory().unwrap();
        let wf = Workflow::new("job", "");
        store.save_workflow(&wf).unwrap();

        let started_at = common::now();
        let run1 = WorkflowRun {
            id: Id::new(),
            workflow_id: wf.id,
            status: RunStatus::Completed,
            step_outcomes: vec![],
            started_at,
            finished_at: started_at,
        };
        let later = started_at + chrono::Duration::seconds(5);
        let run2 = WorkflowRun {
            id: Id::new(),
            workflow_id: wf.id,
            status: RunStatus::Failed,
            step_outcomes: vec![StepOutcome {
                step_id: 0,
                success: false,
                skipped: false,
                message: "oops".into(),
            }],
            started_at: later,
            finished_at: later,
        };
        store.save_run(&run1).unwrap();
        store.save_run(&run2).unwrap();

        let history = store.workflow_history(wf.id, 10).unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].1, RunStatus::Failed); // newest first
        assert_eq!(history[1].1, RunStatus::Completed);
    }
}
