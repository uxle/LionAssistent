//! `assistant-core` -- the brain of the application (Archd Layer 2). Ties
//! together session management, context building, prompt formatting, and
//! (as of Phase 6) intent/reasoning-level classification, then hands the
//! finished prompt to a [`ResponseGenerator`].
//!
//! There is no real LLM behind [`ResponseGenerator`] yet -- that's Phase
//! 4's `llm-runtime`. [`EchoResponseGenerator`] proves the pipeline
//! actually runs (a real prompt gets built from real conversation state)
//! without pretending to be a real reply. Phase 4 implements the same
//! trait against Gemma/llama.cpp; nothing here needs to change when it
//! does (Archd SS3 "Replaceable AI backend").
//!
//! Phase 6 adds classification (`reasoning`) and planning (`planner`),
//! wired into `process_message` so the detected intent and reasoning
//! level genuinely reach the prompt -- but `process_message` itself
//! stays linear (append -> context -> prompt -> generate -> append)
//! rather than branching on the plan, since there's no Tool Framework
//! (Phase 7) yet for a plan to actually drive.

use common::{AppError, AppResult, Id};
use context_manager::ContextBuilder;
use conversation::{Message, Role};
use planner::Planner;
use prompt_builder::{GemmaPromptFormatter, PromptFormatter};
use reasoning::KeywordClassifier;
use session_manager::{Session, SessionManager};

/// The thing that turns a finished prompt into a reply. Phase 4
/// implements this against a real local model; `EchoResponseGenerator`
/// below is a stand-in until then. `&mut self` because a real backend
/// carries mutable state between calls (KV cache).
pub trait ResponseGenerator: Send + Sync {
    fn generate(&mut self, prompt: &str) -> String;
}

/// Stub backend: proves the pipeline ran (built a real prompt of a real
/// length from real conversation state) without pretending to be an
/// actual model reply. Replaced by `llm-runtime` in Phase 4.
#[derive(Debug, Default)]
pub struct EchoResponseGenerator;

impl ResponseGenerator for EchoResponseGenerator {
    fn generate(&mut self, prompt: &str) -> String {
        format!(
            "[stub reply -- llm-runtime lands in Phase 4] pipeline OK: built a {}-character prompt from the conversation so far.",
            prompt.chars().count()
        )
    }
}

/// Defers constructing a real `ResponseGenerator` until the first
/// `generate()` call, then keeps it (Development_plan Phase 18 "Lazy
/// Loading"/"Startup Optimization"). The motivating case is `app`
/// (Phase 5 on): loading `llm-runtime::CandleGemmaBackend` reads model
/// weights off disk, which can be slow enough to matter for how quickly
/// the window first appears -- most sessions send their first message
/// well after startup anyway, so there's no reason that work has to
/// finish before the UI is even up. Wrapping the *same* backend
/// construction this crate always did in `LazyResponseGenerator::new`
/// changes nothing about *what* gets built, only *when*.
///
/// The factory runs at most once, on whichever call to `generate()`
/// happens first, and its result is reused after that -- proven by this
/// module's own `factory_runs_exactly_once_across_multiple_generate_calls`
/// test against a factory that counts its own invocations, not just
/// asserted in a doc comment.
pub struct LazyResponseGenerator {
    factory: Option<Box<dyn FnOnce() -> Box<dyn ResponseGenerator> + Send + Sync>>,
    generator: Option<Box<dyn ResponseGenerator>>,
}

impl LazyResponseGenerator {
    pub fn new(
        factory: impl FnOnce() -> Box<dyn ResponseGenerator> + Send + Sync + 'static,
    ) -> Self {
        Self {
            factory: Some(Box::new(factory)),
            generator: None,
        }
    }

    /// Whether the factory has actually run yet -- for tests, and for
    /// any future diagnostics/startup report that wants to say whether
    /// the real backend is loaded or generation just hasn't happened
    /// yet.
    pub fn is_initialized(&self) -> bool {
        self.generator.is_some()
    }
}

impl ResponseGenerator for LazyResponseGenerator {
    fn generate(&mut self, prompt: &str) -> String {
        if self.generator.is_none() {
            let factory = self
                .factory
                .take()
                .expect("factory is only ever taken once, exactly here");
            self.generator = Some(factory());
        }
        self.generator
            .as_mut()
            .expect("just initialized above if it wasn't already")
            .generate(prompt)
    }
}

pub struct AssistantCore {
    sessions: SessionManager,
    formatter: GemmaPromptFormatter,
    context_builder: ContextBuilder,
    generator: Box<dyn ResponseGenerator>,
    planner: Planner,
    system_prompt: String,
}

impl AssistantCore {
    pub fn new(generator: Box<dyn ResponseGenerator>) -> Self {
        Self {
            sessions: SessionManager::new(),
            formatter: GemmaPromptFormatter,
            context_builder: ContextBuilder::default(),
            generator,
            planner: Planner::new(Box::new(KeywordClassifier)),
            system_prompt:
                "You are a helpful local AI assistant running on the user's Linux desktop."
                    .to_string(),
        }
    }

    pub fn create_session(&mut self, model: impl Into<String>) -> Id {
        self.sessions.create_session(model)
    }

    pub fn session(&self, id: Id) -> Option<&Session> {
        self.sessions.get(id)
    }

    /// Processes one user message end to end: classify it, append it,
    /// build context, build the prompt (informed by the classification),
    /// generate a reply, append the reply, return the reply's text (TDS
    /// SS2 `process_request`; Apis SS3 `process_message`).
    pub fn process_message(
        &mut self,
        session_id: Id,
        user_input: impl Into<String>,
    ) -> AppResult<String> {
        let user_input = user_input.into();
        let classification = self.planner.classify(&user_input);
        tracing::debug!(
            intent = ?classification.intent,
            level = ?classification.level,
            confidence = classification.confidence,
            "classified user request"
        );

        self.sessions
            .append_message(session_id, Message::new(Role::User, user_input))?;

        let session = self
            .sessions
            .get(session_id)
            .ok_or_else(|| AppError::Module {
                module: "assistant-core",
                message: format!("unknown session {session_id}"),
            })?;

        let selected = self.context_builder.select(session.conversation.messages());
        let system_prompt = format!(
            "{}\n\n(Detected intent: {:?}, reasoning level: {:?}.)",
            self.system_prompt, classification.intent, classification.level
        );
        let prompt = self.formatter.format(&system_prompt, &selected);
        let reply = self.generator.generate(&prompt);

        self.sessions
            .append_message(session_id, Message::new(Role::Assistant, reply.clone()))?;

        Ok(reply)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FixedReply(&'static str);
    impl ResponseGenerator for FixedReply {
        fn generate(&mut self, _prompt: &str) -> String {
            self.0.to_string()
        }
    }

    #[test]
    fn processes_a_message_end_to_end() {
        let mut core = AssistantCore::new(Box::new(FixedReply("hi there")));
        let session_id = core.create_session("gemma-3-1b");

        let reply = core.process_message(session_id, "hello").unwrap();
        assert_eq!(reply, "hi there");

        let session = core.session(session_id).unwrap();
        assert_eq!(session.conversation.messages().len(), 2);
        assert_eq!(session.conversation.messages()[0].role, Role::User);
        assert_eq!(session.conversation.messages()[1].role, Role::Assistant);
    }

    #[test]
    fn unknown_session_errors_instead_of_panicking() {
        let mut core = AssistantCore::new(Box::new(EchoResponseGenerator));
        let fake_id = Id::new();
        assert!(core.process_message(fake_id, "hello").is_err());
    }

    #[test]
    fn stub_generator_reports_pipeline_ran() {
        let mut core = AssistantCore::new(Box::new(EchoResponseGenerator));
        let session_id = core.create_session("gemma-3-1b");
        let reply = core.process_message(session_id, "hello").unwrap();
        assert!(reply.contains("Phase 4"));
    }

    #[test]
    fn multi_turn_conversation_accumulates() {
        let mut core = AssistantCore::new(Box::new(FixedReply("ok")));
        let session_id = core.create_session("gemma-3-1b");
        core.process_message(session_id, "first").unwrap();
        core.process_message(session_id, "second").unwrap();
        assert_eq!(
            core.session(session_id)
                .unwrap()
                .conversation
                .messages()
                .len(),
            4
        );
    }

    #[test]
    fn lazy_generator_does_not_run_its_factory_until_first_generate_call() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let ran = Arc::new(AtomicBool::new(false));
        let ran_clone = ran.clone();
        let lazy = LazyResponseGenerator::new(move || {
            ran_clone.store(true, Ordering::SeqCst);
            Box::new(EchoResponseGenerator) as Box<dyn ResponseGenerator>
        });

        assert!(!lazy.is_initialized());
        assert!(
            !ran.load(Ordering::SeqCst),
            "constructing LazyResponseGenerator must not run the factory"
        );
    }

    #[test]
    fn factory_runs_exactly_once_across_multiple_generate_calls() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let calls = Arc::new(AtomicUsize::new(0));
        let calls_clone = calls.clone();
        let mut lazy = LazyResponseGenerator::new(move || {
            calls_clone.fetch_add(1, Ordering::SeqCst);
            Box::new(EchoResponseGenerator) as Box<dyn ResponseGenerator>
        });

        lazy.generate("first");
        lazy.generate("second");
        lazy.generate("third");

        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(lazy.is_initialized());
    }

    #[test]
    fn lazy_generator_delegates_to_the_constructed_backend() {
        let mut lazy = LazyResponseGenerator::new(|| Box::new(EchoResponseGenerator));
        let reply = lazy.generate("hello world");
        assert!(reply.contains("pipeline OK"));
    }

    #[test]
    fn lazy_generator_works_as_a_drop_in_assistant_core_backend() {
        let mut core = AssistantCore::new(Box::new(LazyResponseGenerator::new(|| {
            Box::new(EchoResponseGenerator)
        })));
        let session_id = core.create_session("gemma-3-1b");

        let reply = core.process_message(session_id, "hello").unwrap();

        assert!(reply.contains("pipeline OK"));
        assert_eq!(
            core.session(session_id)
                .unwrap()
                .conversation
                .messages()
                .len(),
            2
        );
    }

    #[test]
    fn classification_reaches_the_generated_prompt() {
        use std::sync::{Arc, Mutex};

        struct CapturingReply(Arc<Mutex<String>>);
        impl ResponseGenerator for CapturingReply {
            fn generate(&mut self, prompt: &str) -> String {
                *self.0.lock().unwrap() = prompt.to_string();
                "ok".to_string()
            }
        }

        let captured = Arc::new(Mutex::new(String::new()));
        let mut core = AssistantCore::new(Box::new(CapturingReply(captured.clone())));
        let session_id = core.create_session("gemma-3-1b");

        core.process_message(session_id, "Open Firefox.").unwrap();

        let prompt = captured.lock().unwrap();
        assert!(prompt.contains("ApplicationControl"));
        assert!(prompt.contains("SingleTool"));
    }
}
