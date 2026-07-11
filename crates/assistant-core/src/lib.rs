//! `assistant-core` -- the brain of the application (Archd Layer 2). Ties
//! together session management, context building, and prompt formatting,
//! then hands the finished prompt to a [`ResponseGenerator`].
//!
//! There is no real LLM behind [`ResponseGenerator`] yet -- that's Phase
//! 4's `llm-runtime`. [`EchoResponseGenerator`] proves the pipeline
//! actually runs (a real prompt gets built from real conversation state)
//! without pretending to be a real reply. Phase 4 implements the same
//! trait against Gemma/llama.cpp; nothing here needs to change when it
//! does (Archd SS3 "Replaceable AI backend").

use common::{AppError, AppResult, Id};
use context_manager::ContextBuilder;
use conversation::{Message, Role};
use prompt_builder::{GemmaPromptFormatter, PromptFormatter};
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

pub struct AssistantCore {
    sessions: SessionManager,
    formatter: GemmaPromptFormatter,
    context_builder: ContextBuilder,
    generator: Box<dyn ResponseGenerator>,
    system_prompt: String,
}

impl AssistantCore {
    pub fn new(generator: Box<dyn ResponseGenerator>) -> Self {
        Self {
            sessions: SessionManager::new(),
            formatter: GemmaPromptFormatter,
            context_builder: ContextBuilder::default(),
            generator,
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

    /// Processes one user message end to end: append it, build context,
    /// build the prompt, generate a reply, append the reply, return the
    /// reply's text (TDS SS2 `process_request`; Apis SS3
    /// `process_message`).
    pub fn process_message(
        &mut self,
        session_id: Id,
        user_input: impl Into<String>,
    ) -> AppResult<String> {
        self.sessions
            .append_message(session_id, Message::new(Role::User, user_input.into()))?;

        let session = self
            .sessions
            .get(session_id)
            .ok_or_else(|| AppError::Module {
                module: "assistant-core",
                message: format!("unknown session {session_id}"),
            })?;

        let selected = self.context_builder.select(session.conversation.messages());
        let prompt = self.formatter.format(&self.system_prompt, &selected);
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
}
