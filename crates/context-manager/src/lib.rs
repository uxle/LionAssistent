//! `context-manager` -- keeps prompts within the model's context window by
//! trimming older messages when necessary (Ai_reasoning SS8 Context
//! Builder).
//!
//! True token-aware budgeting arrives with the tokenizer in Phase 4; this
//! uses a conservative characters estimate in the meantime. Semantic
//! memory retrieval (Ai_reasoning SS9) is Phase 5 and has no hook here
//! yet -- `select` only knows about the live conversation so far.

use conversation::Message;

pub struct ContextBuilder {
    pub max_chars: usize,
}

impl Default for ContextBuilder {
    fn default() -> Self {
        // ~4 chars/token is a common rough estimate. This project's
        // default context_length is 8192 tokens (see config::ModelConfig)
        // -- budget for roughly half, leaving room for the response and
        // the prompt template itself.
        Self {
            max_chars: 8192 / 2 * 4,
        }
    }
}

impl ContextBuilder {
    pub fn new(max_chars: usize) -> Self {
        Self { max_chars }
    }

    /// Returns the newest messages that fit within `max_chars`, dropping
    /// the oldest first, but always keeping at least the single newest
    /// message even if it alone exceeds the budget.
    pub fn select<'a>(&self, messages: &'a [Message]) -> Vec<&'a Message> {
        let mut selected = Vec::new();
        let mut used = 0usize;
        for message in messages.iter().rev() {
            let cost = message.content.len();
            if used + cost > self.max_chars && !selected.is_empty() {
                break;
            }
            used += cost;
            selected.push(message);
        }
        selected.reverse();
        selected
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conversation::Role;

    #[test]
    fn keeps_everything_under_budget() {
        let messages = vec![
            Message::new(Role::User, "hi"),
            Message::new(Role::Assistant, "hello"),
        ];
        let builder = ContextBuilder::new(1000);
        assert_eq!(builder.select(&messages).len(), 2);
    }

    #[test]
    fn drops_oldest_when_over_budget() {
        let messages = vec![
            Message::new(Role::User, "a".repeat(50)),
            Message::new(Role::Assistant, "b".repeat(50)),
            Message::new(Role::User, "c".repeat(50)),
        ];
        let builder = ContextBuilder::new(120);
        let selected = builder.select(&messages);
        assert!(selected.len() < messages.len());
        assert_eq!(selected.last().unwrap().content, "c".repeat(50));
    }

    #[test]
    fn always_keeps_at_least_the_newest_message() {
        let messages = vec![Message::new(Role::User, "x".repeat(9999))];
        let builder = ContextBuilder::new(10);
        assert_eq!(builder.select(&messages).len(), 1);
    }
}
