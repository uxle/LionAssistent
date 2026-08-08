//! What a telemetry event is allowed to contain, by construction.
//!
//! **Design rule, not a suggestion:** a telemetry event must never carry
//! user content, file paths, tool arguments, or free-form error text --
//! PRD SS3's "no cloud dependency for the core assistant" and Archd
//! SS3's "secure by default" read here as "private by default": nothing
//! leaves the device unless the user explicitly opts in (see
//! [`crate::TelemetryClient`]), and even then, only *this* -- a closed,
//! small vocabulary of event kinds, plus one optional, length-capped
//! label meant for coarse context like a tool's category, never its
//! arguments.
//!
//! [`TelemetryEventKind`] being an enum (not a raw `String`) is the
//! actual enforcement mechanism here, not just documentation: a caller
//! physically cannot construct an event with an arbitrary name.

use common::{now, Timestamp};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TelemetryEventKind {
    SessionStarted,
    SessionEnded,
    ToolExecuted,
    ToolFailed,
    UpdateCheckPerformed,
    CrashReported,
}

/// A label's maximum length -- generous enough for something like a
/// `ToolCategory`'s name, far too short to usefully hold a file path,
/// error message, or any real user content. A backstop, not a
/// substitute for choosing a safe label in the first place; see this
/// module's doc comment.
const MAX_LABEL_LEN: usize = 64;

#[derive(Debug, Clone, Serialize)]
pub struct TelemetryEvent {
    pub kind: TelemetryEventKind,
    pub timestamp: Timestamp,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

impl TelemetryEvent {
    pub fn new(kind: TelemetryEventKind) -> Self {
        Self {
            kind,
            timestamp: now(),
            label: None,
        }
    }

    /// Attaches a coarse label, truncated to [`MAX_LABEL_LEN`] characters
    /// if needed. Intended for closed, small-cardinality context (a
    /// `ToolCategory`'s name, say) -- see this module's doc comment for
    /// what must never go here.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        let mut label = label.into();
        if label.len() > MAX_LABEL_LEN {
            label.truncate(MAX_LABEL_LEN);
        }
        self.label = Some(label);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_event_has_no_label_by_default() {
        let event = TelemetryEvent::new(TelemetryEventKind::SessionStarted);
        assert!(event.label.is_none());
    }

    #[test]
    fn with_label_attaches_a_short_label_unchanged() {
        let event =
            TelemetryEvent::new(TelemetryEventKind::ToolExecuted).with_label("FileManagement");
        assert_eq!(event.label.as_deref(), Some("FileManagement"));
    }

    #[test]
    fn with_label_truncates_anything_longer_than_the_cap() {
        let long = "x".repeat(500);
        let event = TelemetryEvent::new(TelemetryEventKind::ToolExecuted).with_label(long);
        assert_eq!(event.label.unwrap().len(), MAX_LABEL_LEN);
    }

    #[test]
    fn event_kind_serializes_as_snake_case() {
        let value = serde_json::to_value(TelemetryEventKind::UpdateCheckPerformed).unwrap();
        assert_eq!(value, "update_check_performed");
    }
}
