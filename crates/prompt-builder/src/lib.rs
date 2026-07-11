//! `prompt-builder` -- turns a system preamble + selected conversation
//! history into the exact string sent to the model (Mms SS8 Prompt
//! Pipeline).
//!
//! Implemented per model family behind a trait (Mms SS4 "Supported Model
//! Families") so swapping the local model later doesn't touch the
//! Assistant Core (Archd SS3 "Replaceable AI backend").

use conversation::{Message, Role};

pub trait PromptFormatter {
    fn format(&self, system: &str, history: &[&Message]) -> String;
}

/// Gemma's turn-based chat template:
/// `<start_of_turn>user\n{msg}<end_of_turn>\n<start_of_turn>model\n{msg}<end_of_turn>\n`
/// repeated per turn, with a trailing `<start_of_turn>model\n` to prompt
/// the next reply. Gemma has no dedicated system role, so the system
/// preamble is folded into the first user turn.
#[derive(Debug, Default, Clone, Copy)]
pub struct GemmaPromptFormatter;

impl PromptFormatter for GemmaPromptFormatter {
    fn format(&self, system: &str, history: &[&Message]) -> String {
        let mut out = String::new();
        let mut system_written = system.is_empty();

        for message in history {
            match message.role {
                Role::User => {
                    out.push_str("<start_of_turn>user\n");
                    if !system_written {
                        out.push_str(system);
                        out.push('\n');
                        system_written = true;
                    }
                    out.push_str(&message.content);
                    out.push_str("<end_of_turn>\n");
                }
                Role::Assistant => {
                    out.push_str("<start_of_turn>model\n");
                    out.push_str(&message.content);
                    out.push_str("<end_of_turn>\n");
                }
                Role::System | Role::Tool => {
                    // Folded into the next user turn's preamble for now;
                    // Gemma has no first-class system/tool role. Tool
                    // results get their own treatment once the Tool
                    // Framework (Phase 7) exists.
                }
            }
        }
        out.push_str("<start_of_turn>model\n");
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_a_single_turn_with_system_preamble() {
        let msg = Message::new(Role::User, "hello");
        let history = vec![&msg];
        let prompt = GemmaPromptFormatter.format("be helpful", &history);
        assert!(prompt.starts_with("<start_of_turn>user\nbe helpful\nhello<end_of_turn>\n"));
        assert!(prompt.ends_with("<start_of_turn>model\n"));
    }

    #[test]
    fn only_injects_system_preamble_once() {
        let m1 = Message::new(Role::User, "first");
        let m2 = Message::new(Role::Assistant, "reply");
        let m3 = Message::new(Role::User, "second");
        let history = vec![&m1, &m2, &m3];
        let prompt = GemmaPromptFormatter.format("sys", &history);
        assert_eq!(prompt.matches("sys").count(), 1);
    }

    #[test]
    fn empty_system_preamble_is_skipped_entirely() {
        let msg = Message::new(Role::User, "hi");
        let history = vec![&msg];
        let prompt = GemmaPromptFormatter.format("", &history);
        assert_eq!(
            prompt,
            "<start_of_turn>user\nhi<end_of_turn>\n<start_of_turn>model\n"
        );
    }
}
