//! `reasoning` -- classifies user input into an intent category and a
//! reasoning level (Ai_reasoning SS5 "Reasoning Levels", SS6 "Intent
//! Classification").
//!
//! [`KeywordClassifier`] is a real, working, dependency-free rule-based
//! classifier -- not an LLM-based one. `llm-runtime` (Phase 4) has its
//! own unresolved sandbox toolchain constraint (docs/ROADMAP.md), and a
//! keyword classifier needs no model at all, so this is genuinely useful
//! today rather than a placeholder. It's honest about being a heuristic:
//! confidence scores are relative, not calibrated probabilities. An
//! LLM-backed classifier can implement the same [`IntentClassifier`]
//! trait later (Archd SS3 "Replaceable AI backend").

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intent {
    QuestionAnswering,
    Conversation,
    FileManagement,
    ApplicationControl,
    SystemInformation,
    PackageManagement,
    Automation,
    Settings,
    WorkflowExecution,
    MemoryOperations,
    Help,
    Search,
}

/// Ai_reasoning SS5: how much reasoning a request needs, from a direct
/// answer up to a long-running reactive workflow. Ordered so callers can
/// compare levels directly (`level >= ReasoningLevel::SingleTool`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum ReasoningLevel {
    /// "What is Linux?"
    Direct = 0,
    /// "What theme do I prefer?"
    MemoryLookup = 1,
    /// "Open Firefox."
    SingleTool = 2,
    /// "Find all PDFs in Downloads and compress them."
    MultiStepWorkflow = 3,
    /// "Monitor disk usage and notify me when free space drops below 10 GB."
    LongRunningWorkflow = 4,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IntentClassification {
    pub intent: Intent,
    pub level: ReasoningLevel,
    /// Relative self-reported confidence in `[0.0, 1.0]`, not a
    /// calibrated probability -- higher when a specific keyword rule
    /// matched, lower when this fell through to a generic pattern.
    pub confidence: f32,
}

pub trait IntentClassifier: Send + Sync {
    fn classify(&self, input: &str) -> IntentClassification;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct KeywordClassifier;

impl IntentClassifier for KeywordClassifier {
    fn classify(&self, input: &str) -> IntentClassification {
        let text = input.to_lowercase();
        let level = classify_level(&text);
        let (intent, confidence) = classify_intent(&text);
        IntentClassification {
            intent,
            level,
            confidence,
        }
    }
}

const LONG_RUNNING_PATTERNS: &[&str] = &[
    "monitor",
    "watch for",
    "notify me when",
    "alert me when",
    "keep track of",
    "whenever ",
];

const MEMORY_PATTERNS: &[&str] = &[
    "do i prefer",
    "what do i like",
    "did i say",
    "do i like",
    "what's my",
    "what is my",
    "remind me what",
    "remember that i",
    "have i told you",
];

const ACTION_VERBS: &[&str] = &[
    "open ",
    "launch ",
    "close ",
    "quit ",
    "start ",
    "find ",
    "search ",
    "delete ",
    "remove ",
    "move ",
    "copy ",
    "compress ",
    "install ",
    "uninstall ",
    "update ",
    "set ",
    "change ",
    "run ",
    "execute ",
];

fn contains_any(text: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|p| text.contains(p))
}

/// A conjunction ("and"/"then") joining clauses that include at least one
/// recognized action verb is treated as a multi-step request -- modeled
/// directly on Ai_reasoning's own example ("Find all PDFs ... **and**
/// compress them").
fn joins_multiple_actions(text: &str) -> bool {
    let has_conjunction = text.contains(" and ") || text.contains(" then ");
    has_conjunction && contains_any(text, ACTION_VERBS)
}

fn classify_level(text: &str) -> ReasoningLevel {
    if contains_any(text, LONG_RUNNING_PATTERNS) {
        ReasoningLevel::LongRunningWorkflow
    } else if contains_any(text, MEMORY_PATTERNS) {
        ReasoningLevel::MemoryLookup
    } else if joins_multiple_actions(text) {
        ReasoningLevel::MultiStepWorkflow
    } else if contains_any(text, ACTION_VERBS) {
        ReasoningLevel::SingleTool
    } else {
        ReasoningLevel::Direct
    }
}

fn classify_intent(text: &str) -> (Intent, f32) {
    const RULES: &[(&[&str], Intent)] = &[
        (MEMORY_PATTERNS, Intent::MemoryOperations),
        (&["remember", "forget"], Intent::MemoryOperations),
        (LONG_RUNNING_PATTERNS, Intent::Automation),
        (
            &["run the workflow", "execute workflow", "start workflow"],
            Intent::WorkflowExecution,
        ),
        (
            &["open ", "launch ", "close ", "quit ", "restart "],
            Intent::ApplicationControl,
        ),
        (
            &[
                "install ",
                "uninstall ",
                "remove package",
                "update package",
                "apt ",
                "package",
            ],
            Intent::PackageManagement,
        ),
        (
            &[
                "cpu",
                "ram",
                "memory usage",
                "disk usage",
                "disk space",
                "battery",
                "uptime",
                "system info",
            ],
            Intent::SystemInformation,
        ),
        (
            &[
                "find file",
                "find all",
                "search for file",
                "delete file",
                "move file",
                "copy file",
                "compress",
                "pdf",
                "download",
                "folder",
                "directory",
            ],
            Intent::FileManagement,
        ),
        (
            &[
                "change setting",
                "set theme",
                "preferences",
                "configure",
                "settings",
            ],
            Intent::Settings,
        ),
        (&["search for", "look up", "look for"], Intent::Search),
        (
            &["help", "how do i", "what can you do", "what commands"],
            Intent::Help,
        ),
        (
            &["hi", "hello", "hey ", "how are you", "thanks", "thank you"],
            Intent::Conversation,
        ),
    ];

    for (keywords, intent) in RULES {
        if contains_any(text, keywords) {
            return (*intent, 0.7);
        }
    }

    let trimmed = text.trim_end_matches(['?', '.', '!']);
    if trimmed.starts_with("what is")
        || trimmed.starts_with("who is")
        || trimmed.starts_with("when is")
        || trimmed.starts_with("why ")
        || trimmed.starts_with("how does")
        || trimmed.starts_with("how do ")
    {
        return (Intent::QuestionAnswering, 0.6);
    }

    (Intent::Conversation, 0.3)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn classify(input: &str) -> IntentClassification {
        KeywordClassifier.classify(input)
    }

    // The five canonical examples from Ai_reasoning SS5, used verbatim.
    #[test]
    fn direct_question() {
        let c = classify("What is Linux?");
        assert_eq!(c.level, ReasoningLevel::Direct);
        assert_eq!(c.intent, Intent::QuestionAnswering);
    }

    #[test]
    fn memory_lookup() {
        let c = classify("What theme do I prefer?");
        assert_eq!(c.level, ReasoningLevel::MemoryLookup);
        assert_eq!(c.intent, Intent::MemoryOperations);
    }

    #[test]
    fn single_tool_call() {
        let c = classify("Open Firefox.");
        assert_eq!(c.level, ReasoningLevel::SingleTool);
        assert_eq!(c.intent, Intent::ApplicationControl);
    }

    #[test]
    fn multi_step_workflow() {
        let c = classify("Find all PDFs in Downloads and compress them.");
        assert_eq!(c.level, ReasoningLevel::MultiStepWorkflow);
        assert_eq!(c.intent, Intent::FileManagement);
    }

    #[test]
    fn long_running_workflow() {
        let c = classify("Monitor disk usage and notify me when free space drops below 10 GB.");
        assert_eq!(c.level, ReasoningLevel::LongRunningWorkflow);
        assert_eq!(c.intent, Intent::Automation);
    }

    // Additional coverage beyond the five canonical examples.
    #[test]
    fn package_management() {
        let c = classify("Install the ripgrep package");
        assert_eq!(c.intent, Intent::PackageManagement);
        assert_eq!(c.level, ReasoningLevel::SingleTool);
    }

    #[test]
    fn system_information() {
        let c = classify("How much disk space is left?");
        assert_eq!(c.intent, Intent::SystemInformation);
    }

    #[test]
    fn greeting_is_conversation_with_low_confidence() {
        let c = classify("Hey, how's it going?");
        assert_eq!(c.intent, Intent::Conversation);
    }

    #[test]
    fn empty_input_falls_back_to_direct_conversation() {
        let c = classify("");
        assert_eq!(c.level, ReasoningLevel::Direct);
        assert_eq!(c.intent, Intent::Conversation);
    }

    #[test]
    fn reasoning_levels_are_ordered() {
        assert!(ReasoningLevel::Direct < ReasoningLevel::MemoryLookup);
        assert!(ReasoningLevel::SingleTool < ReasoningLevel::MultiStepWorkflow);
        assert!(ReasoningLevel::MultiStepWorkflow < ReasoningLevel::LongRunningWorkflow);
    }

    #[test]
    fn plain_and_without_an_action_verb_is_not_multi_step() {
        // Regression guard: "and" alone shouldn't trigger multi-step --
        // only "and"/"then" combined with a recognized action verb should.
        let c = classify("I like cats and dogs");
        assert_ne!(c.level, ReasoningLevel::MultiStepWorkflow);
    }
}
