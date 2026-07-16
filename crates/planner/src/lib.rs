//! `planner` -- turns a request into an ordered, dependency-aware plan
//! (Development_plan Phase 6; Ai_reasoning SS7 "Planning"; Apis SS5
//! Planner API).

use reasoning::{Intent, IntentClassification, IntentClassifier, ReasoningLevel};
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    Low,
    Normal,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Skipped,
}

/// One step of a [`Plan`] (Apis SS5 "ordered task list").
#[derive(Debug, Clone)]
pub struct PlanStep {
    pub id: usize,
    pub description: String,
    /// Name of the tool this step would call, once the Tool Framework
    /// (Phase 7) exists to actually look one up. `None` for steps that
    /// don't need one (e.g. "respond directly").
    pub tool: Option<String>,
    pub depends_on: Vec<usize>,
    pub requires_confirmation: bool,
    pub status: StepStatus,
}

#[derive(Debug, Clone)]
pub struct Plan {
    pub goal: String,
    pub priority: Priority,
    steps: Vec<PlanStep>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    UnknownDependency {
        step: usize,
        depends_on: usize,
    },
    UnknownStep(usize),
    /// The ids of steps that couldn't be ordered because they're part of
    /// (or depend on) a dependency cycle.
    Cycle(Vec<usize>),
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownDependency { step, depends_on } => {
                write!(f, "step {step} depends on unknown step {depends_on}")
            }
            Self::UnknownStep(id) => write!(f, "no step with id {id}"),
            Self::Cycle(ids) => write!(f, "dependency cycle among steps {ids:?}"),
        }
    }
}

impl std::error::Error for PlanError {}

impl Plan {
    pub fn new(goal: impl Into<String>, priority: Priority) -> Self {
        Self {
            goal: goal.into(),
            priority,
            steps: Vec::new(),
        }
    }

    /// Adds a step and returns its id, for use in later steps'
    /// `depends_on`.
    pub fn add_step(
        &mut self,
        description: impl Into<String>,
        tool: Option<String>,
        depends_on: Vec<usize>,
        requires_confirmation: bool,
    ) -> usize {
        let id = self.steps.len();
        self.steps.push(PlanStep {
            id,
            description: description.into(),
            tool,
            depends_on,
            requires_confirmation,
            status: StepStatus::Pending,
        });
        id
    }

    pub fn steps(&self) -> &[PlanStep] {
        &self.steps
    }

    pub fn mark(&mut self, step_id: usize, status: StepStatus) -> Result<(), PlanError> {
        let step = self
            .steps
            .get_mut(step_id)
            .ok_or(PlanError::UnknownStep(step_id))?;
        step.status = status;
        Ok(())
    }

    pub fn is_complete(&self) -> bool {
        !self.steps.is_empty()
            && self
                .steps
                .iter()
                .all(|s| matches!(s.status, StepStatus::Completed | StepStatus::Skipped))
    }

    /// Structural validation (Apis SS5 `validate_plan`): every dependency
    /// must reference a real step, and the dependency graph must be
    /// acyclic. Delegates to `execution_order`, which needs exactly the
    /// same checks to produce a valid ordering.
    pub fn validate(&self) -> Result<(), PlanError> {
        self.execution_order().map(|_| ())
    }

    /// A step order respecting every dependency (Kahn's algorithm; Apis
    /// SS5 `optimize_plan`-adjacent). Errors on an unknown dependency or
    /// a cycle rather than silently producing a partial or wrong order.
    pub fn execution_order(&self) -> Result<Vec<usize>, PlanError> {
        let n = self.steps.len();
        let mut in_degree = vec![0usize; n];
        let mut dependents: Vec<Vec<usize>> = vec![Vec::new(); n];

        for step in &self.steps {
            for &dep in &step.depends_on {
                if dep >= n {
                    return Err(PlanError::UnknownDependency {
                        step: step.id,
                        depends_on: dep,
                    });
                }
                dependents[dep].push(step.id);
                in_degree[step.id] += 1;
            }
        }

        let mut queue: VecDeque<usize> = (0..n).filter(|&i| in_degree[i] == 0).collect();
        let mut order = Vec::with_capacity(n);

        while let Some(id) = queue.pop_front() {
            order.push(id);
            for &dependent in &dependents[id] {
                in_degree[dependent] -= 1;
                if in_degree[dependent] == 0 {
                    queue.push_back(dependent);
                }
            }
        }

        if order.len() == n {
            Ok(order)
        } else {
            let remaining: Vec<usize> = (0..n).filter(|i| !order.contains(i)).collect();
            Err(PlanError::Cycle(remaining))
        }
    }
}

/// Classifies input and turns it into a [`Plan`] (Apis SS5 `create_plan`).
///
/// Honest about what it can and can't decompose: Direct/MemoryLookup/
/// SingleTool requests get one real, concrete step. MultiStep/
/// LongRunning requests get one step that says so explicitly, rather
/// than fabricating a breakdown with no Tool Framework (Phase 7) to
/// ground concrete steps in real capabilities (Ai_reasoning SS10: "Never
/// invent tool results" -- applies just as much to plan steps as to
/// pretending a tool ran).
pub struct Planner {
    classifier: Box<dyn IntentClassifier>,
}

impl Planner {
    pub fn new(classifier: Box<dyn IntentClassifier>) -> Self {
        Self { classifier }
    }

    pub fn classify(&self, input: &str) -> IntentClassification {
        self.classifier.classify(input)
    }

    pub fn create_plan(&self, input: &str) -> Plan {
        let classification = self.classify(input);
        let mut plan = Plan::new(input, priority_for(classification.level));

        match classification.level {
            ReasoningLevel::Direct | ReasoningLevel::MemoryLookup => {
                plan.add_step(
                    "respond directly from the conversation and memory already available",
                    None,
                    vec![],
                    false,
                );
            }
            ReasoningLevel::SingleTool => {
                plan.add_step(
                    format!(
                        "call a tool to handle this request (intent: {:?})",
                        classification.intent
                    ),
                    None,
                    vec![],
                    needs_confirmation(classification.intent),
                );
            }
            ReasoningLevel::MultiStepWorkflow | ReasoningLevel::LongRunningWorkflow => {
                plan.add_step(
                    "this request needs multi-step decomposition, which needs either a working \
                     llm-runtime (Phase 4) or a real Tool Framework (Phase 7) to ground concrete \
                     steps in actual capabilities -- flagging rather than fabricating a breakdown",
                    None,
                    vec![],
                    true,
                );
            }
        }
        plan
    }
}

fn priority_for(level: ReasoningLevel) -> Priority {
    match level {
        ReasoningLevel::LongRunningWorkflow => Priority::High,
        ReasoningLevel::MultiStepWorkflow => Priority::Normal,
        _ => Priority::Low,
    }
}

fn needs_confirmation(intent: Intent) -> bool {
    matches!(
        intent,
        Intent::PackageManagement | Intent::FileManagement | Intent::Automation
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use reasoning::KeywordClassifier;

    #[test]
    fn linear_plan_executes_in_order() {
        let mut plan = Plan::new("test", Priority::Normal);
        let a = plan.add_step("a", None, vec![], false);
        let b = plan.add_step("b", None, vec![a], false);
        let c = plan.add_step("c", None, vec![b], false);

        assert_eq!(plan.execution_order().unwrap(), vec![a, b, c]);
    }

    #[test]
    fn independent_steps_can_appear_in_any_relative_order_but_all_present() {
        let mut plan = Plan::new("test", Priority::Normal);
        plan.add_step("a", None, vec![], false);
        plan.add_step("b", None, vec![], false);

        let order = plan.execution_order().unwrap();
        assert_eq!(order.len(), 2);
    }

    #[test]
    fn detects_a_direct_cycle() {
        let mut plan = Plan::new("test", Priority::Normal);
        // Steps reference each other before either is added, so build
        // ids by hand: step 0 will depend on step 1 and vice versa.
        plan.add_step("a", None, vec![1], false);
        plan.add_step("b", None, vec![0], false);

        assert!(matches!(plan.execution_order(), Err(PlanError::Cycle(_))));
    }

    #[test]
    fn detects_an_unknown_dependency() {
        let mut plan = Plan::new("test", Priority::Normal);
        plan.add_step("a", None, vec![99], false);

        assert_eq!(
            plan.validate(),
            Err(PlanError::UnknownDependency {
                step: 0,
                depends_on: 99
            })
        );
    }

    #[test]
    fn mark_updates_status_and_is_complete_reflects_it() {
        let mut plan = Plan::new("test", Priority::Normal);
        let a = plan.add_step("a", None, vec![], false);
        assert!(!plan.is_complete());
        plan.mark(a, StepStatus::Completed).unwrap();
        assert!(plan.is_complete());
    }

    #[test]
    fn marking_an_unknown_step_is_an_error() {
        let mut plan = Plan::new("test", Priority::Normal);
        assert_eq!(
            plan.mark(0, StepStatus::Completed),
            Err(PlanError::UnknownStep(0))
        );
    }

    #[test]
    fn empty_plan_is_not_considered_complete() {
        assert!(!Plan::new("test", Priority::Normal).is_complete());
    }

    #[test]
    fn direct_requests_get_a_single_real_step() {
        let planner = Planner::new(Box::new(KeywordClassifier));
        let plan = planner.create_plan("What is Linux?");
        assert_eq!(plan.steps().len(), 1);
        assert!(!plan.steps()[0].requires_confirmation);
    }

    #[test]
    fn multi_step_requests_are_flagged_not_fabricated() {
        let planner = Planner::new(Box::new(KeywordClassifier));
        let plan = planner.create_plan("Find all PDFs in Downloads and compress them.");
        assert_eq!(plan.steps().len(), 1);
        assert!(plan.steps()[0].description.contains("Tool Framework"));
        assert!(plan.steps()[0].requires_confirmation);
    }

    #[test]
    fn package_management_requires_confirmation() {
        let planner = Planner::new(Box::new(KeywordClassifier));
        let plan = planner.create_plan("Install the ripgrep package");
        assert!(plan.steps()[0].requires_confirmation);
    }

    #[test]
    fn long_running_requests_get_high_priority() {
        let planner = Planner::new(Box::new(KeywordClassifier));
        let plan = planner
            .create_plan("Monitor disk usage and notify me when free space drops below 10 GB.");
        assert_eq!(plan.priority, Priority::High);
    }
}
