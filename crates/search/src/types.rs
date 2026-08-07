//! Shared result types every search provider produces, so [`crate::engine::SearchEngine`]
//! (Development_plan Phase 13 "Global Search") can merge results from
//! files, memories, and commands into one ranked list.

use std::fmt;

/// Which provider produced a [`SearchHit`] (TCLIS SS4 "Search" category).
///
/// This workspace's take on "search installed applications" is "search
/// things the assistant can actually run" -- registered `tool_framework`
/// tools and saved `workflow_engine` workflows -- rather than desktop
/// `.desktop` entries, since there's no desktop session in this sandbox
/// to enumerate those from (the same honest scope limit
/// `linux_services::applications` already documented). See
/// `crate::command_search` for the detail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SearchKind {
    File,
    Memory,
    Command,
}

impl SearchKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            SearchKind::File => "file",
            SearchKind::Memory => "memory",
            SearchKind::Command => "command",
        }
    }
}

impl fmt::Display for SearchKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One search result, comparable across providers via `score` (higher is
/// more relevant). See `crate::ranking::score_text` for how file/command
/// scores are computed, and `crate::memory_search` for why memory hits
/// use a different, honestly-documented method.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchHit {
    pub kind: SearchKind,
    pub title: String,
    pub snippet: String,
    /// `0.0..=1.0`; higher is more relevant. Comparable *within* a
    /// provider by construction; comparable *across* providers only
    /// approximately, since files/commands are scored from real text
    /// overlap while memory hits are scored from rank position (see
    /// `memory_search`) -- the best this crate can honestly offer
    /// without either a shared embedding space or exposing `memory`'s
    /// internal similarity numbers through its public API.
    pub score: f32,
    /// Opaque origin identifier: an absolute path for `File`, a
    /// `common::Id` rendered as a string for `Memory`, a tool/workflow
    /// name for `Command`. Not parsed by anything in this crate -- purely
    /// for a future caller (a UI, Archd SS4 "Memory Viewer"-style) to
    /// jump to the right place.
    pub source_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_kind_displays_as_lowercase_str() {
        assert_eq!(SearchKind::File.to_string(), "file");
        assert_eq!(SearchKind::Memory.to_string(), "memory");
        assert_eq!(SearchKind::Command.to_string(), "command");
    }
}
