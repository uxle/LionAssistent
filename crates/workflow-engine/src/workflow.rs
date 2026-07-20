//! Workflow definitions (Development_plan Phase 10 "Workflow Builder";
//! DBDMS SS3 "Workflows" table; Ai_reasoning SS14: sequential tasks,
//! conditional branches, parallel tasks where safe).
//!
//! A `Workflow` is deliberately different from Phase 6's `planner::Plan`:
//! a `Plan` is ephemeral, created from one classified request, honestly
//! uncertain about which concrete tool (if any) it should call. A
//! `Workflow` is named, reusable, and persisted -- its steps reference
//! real, concrete tools and arguments a user or template has explicitly
//! defined, meant to be run again and again rather than once.

use common::Id;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tool_framework::ToolArgs;

/// Whether a step should run, based on an earlier step's outcome
/// (Ai_reasoning SS14 "conditional branches"). Deliberately this simple
/// rather than a full expression language: "run this only if that step
/// succeeded/failed" covers the common real cases (fallback actions,
/// cleanup-on-failure) without needing an expression evaluator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepCondition {
    Always,
    IfStepSucceeded(usize),
    IfStepFailed(usize),
}

impl StepCondition {
    fn referenced_step(&self) -> Option<usize> {
        match self {
            StepCondition::Always => None,
            StepCondition::IfStepSucceeded(id) | StepCondition::IfStepFailed(id) => Some(*id),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub id: usize,
    pub tool_name: String,
    pub args: ToolArgs,
    pub depends_on: Vec<usize>,
    pub condition: StepCondition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowError {
    UnknownDependency {
        step: usize,
        depends_on: usize,
    },
    /// A step's condition references another step that isn't (directly
    /// or transitively) a dependency, so there's no guarantee that step
    /// has already run by the time the condition is checked.
    ConditionNotOrdered {
        step: usize,
        references: usize,
    },
    Cycle(Vec<usize>),
}

impl std::fmt::Display for WorkflowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownDependency { step, depends_on } => {
                write!(f, "step {step} depends on unknown step {depends_on}")
            }
            Self::ConditionNotOrdered { step, references } => {
                write!(f, "step {step}'s condition references step {references}, which isn't in its depends_on")
            }
            Self::Cycle(ids) => write!(f, "dependency cycle among steps {ids:?}"),
        }
    }
}

impl std::error::Error for WorkflowError {}

#[derive(Debug, Clone)]
pub struct Workflow {
    pub id: Id,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    steps: Vec<WorkflowStep>,
}

impl Workflow {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            id: Id::new(),
            name: name.into(),
            description: description.into(),
            enabled: true,
            steps: Vec::new(),
        }
    }

    /// Reconstructs a `Workflow` from previously-saved parts (used by
    /// `WorkflowStore` when loading). `new` is for building a fresh one.
    pub fn from_parts(
        id: Id,
        name: String,
        description: String,
        enabled: bool,
        steps: Vec<WorkflowStep>,
    ) -> Self {
        Self {
            id,
            name,
            description,
            enabled,
            steps,
        }
    }

    pub fn add_step(
        &mut self,
        tool_name: impl Into<String>,
        args: ToolArgs,
        depends_on: Vec<usize>,
        condition: StepCondition,
    ) -> usize {
        let id = self.steps.len();
        self.steps.push(WorkflowStep {
            id,
            tool_name: tool_name.into(),
            args,
            depends_on,
            condition,
        });
        id
    }

    pub fn steps(&self) -> &[WorkflowStep] {
        &self.steps
    }

    pub fn validate(&self) -> Result<(), WorkflowError> {
        for step in &self.steps {
            if let Some(referenced) = step.condition.referenced_step() {
                if !step.depends_on.contains(&referenced) {
                    return Err(WorkflowError::ConditionNotOrdered {
                        step: step.id,
                        references: referenced,
                    });
                }
            }
        }
        self.parallel_stages().map(|_| ())
    }

    /// Groups steps into sequential stages; steps within a stage share
    /// no dependency relationship with each other, so nothing about
    /// their relative order matters. A real executor could dispatch a
    /// stage's steps concurrently, though this crate's own executor
    /// currently runs even same-stage steps one at a time -- see
    /// `executor::run_workflow`'s doc comment for why that's an honest
    /// simplification rather than a missing feature.
    pub fn parallel_stages(&self) -> Result<Vec<Vec<usize>>, WorkflowError> {
        let n = self.steps.len();
        let mut in_degree = vec![0usize; n];
        let mut dependents: Vec<Vec<usize>> = vec![Vec::new(); n];

        for step in &self.steps {
            for &dep in &step.depends_on {
                if dep >= n {
                    return Err(WorkflowError::UnknownDependency {
                        step: step.id,
                        depends_on: dep,
                    });
                }
                dependents[dep].push(step.id);
                in_degree[step.id] += 1;
            }
        }

        let mut stages = Vec::new();
        let mut remaining: HashSet<usize> = (0..n).collect();
        let mut current_stage: Vec<usize> = (0..n).filter(|&i| in_degree[i] == 0).collect();

        while !current_stage.is_empty() {
            for &id in &current_stage {
                remaining.remove(&id);
            }
            let mut next_stage = Vec::new();
            for &id in &current_stage {
                for &dependent in &dependents[id] {
                    in_degree[dependent] -= 1;
                    if in_degree[dependent] == 0 {
                        next_stage.push(dependent);
                    }
                }
            }
            stages.push(current_stage);
            current_stage = next_stage;
        }

        if remaining.is_empty() {
            Ok(stages)
        } else {
            Err(WorkflowError::Cycle(remaining.into_iter().collect()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn independent_steps_share_one_stage() {
        let mut wf = Workflow::new("test", "");
        wf.add_step("a", ToolArgs::new(), vec![], StepCondition::Always);
        wf.add_step("b", ToolArgs::new(), vec![], StepCondition::Always);
        assert_eq!(wf.parallel_stages().unwrap(), vec![vec![0, 1]]);
    }

    #[test]
    fn a_linear_chain_is_all_separate_stages() {
        let mut wf = Workflow::new("test", "");
        let a = wf.add_step("a", ToolArgs::new(), vec![], StepCondition::Always);
        let b = wf.add_step("b", ToolArgs::new(), vec![a], StepCondition::Always);
        wf.add_step("c", ToolArgs::new(), vec![b], StepCondition::Always);
        assert_eq!(
            wf.parallel_stages().unwrap(),
            vec![vec![0], vec![1], vec![2]]
        );
    }

    #[test]
    fn a_diamond_groups_the_middle_two_steps_together() {
        let mut wf = Workflow::new("test", "");
        let a = wf.add_step("a", ToolArgs::new(), vec![], StepCondition::Always);
        let b = wf.add_step("b", ToolArgs::new(), vec![a], StepCondition::Always);
        let c = wf.add_step("c", ToolArgs::new(), vec![a], StepCondition::Always);
        wf.add_step("d", ToolArgs::new(), vec![b, c], StepCondition::Always);
        assert_eq!(
            wf.parallel_stages().unwrap(),
            vec![vec![0], vec![1, 2], vec![3]]
        );
    }

    #[test]
    fn detects_a_cycle() {
        let mut wf = Workflow::new("test", "");
        wf.add_step("a", ToolArgs::new(), vec![1], StepCondition::Always);
        wf.add_step("b", ToolArgs::new(), vec![0], StepCondition::Always);
        assert!(matches!(wf.parallel_stages(), Err(WorkflowError::Cycle(_))));
    }

    #[test]
    fn detects_an_unknown_dependency() {
        let mut wf = Workflow::new("test", "");
        wf.add_step("a", ToolArgs::new(), vec![99], StepCondition::Always);
        assert_eq!(
            wf.validate(),
            Err(WorkflowError::UnknownDependency {
                step: 0,
                depends_on: 99
            })
        );
    }

    #[test]
    fn a_condition_must_be_backed_by_a_real_dependency() {
        let mut wf = Workflow::new("test", "");
        let a = wf.add_step("a", ToolArgs::new(), vec![], StepCondition::Always);
        // References step `a` in its condition but never lists it in
        // depends_on -- nothing guarantees `a` has run yet.
        wf.add_step(
            "b",
            ToolArgs::new(),
            vec![],
            StepCondition::IfStepSucceeded(a),
        );
        assert_eq!(
            wf.validate(),
            Err(WorkflowError::ConditionNotOrdered {
                step: 1,
                references: 0
            })
        );
    }

    #[test]
    fn a_well_ordered_condition_passes_validation() {
        let mut wf = Workflow::new("test", "");
        let a = wf.add_step("a", ToolArgs::new(), vec![], StepCondition::Always);
        wf.add_step(
            "b",
            ToolArgs::new(),
            vec![a],
            StepCondition::IfStepSucceeded(a),
        );
        assert!(wf.validate().is_ok());
    }
}
