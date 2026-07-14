//! Asking for confirmation (TDS SS2 "Permission Manager":
//! `request_confirmation`). A real interactive dialog needs a working
//! UI to show it in -- Phase 2's Slint UI has its own unresolved
//! toolchain constraint (docs/ROADMAP.md) -- so this stays a swappable
//! trait (Archd SS3 "Replaceable AI backend", same pattern as
//! `ResponseGenerator`/`Embedder`/`IntentClassifier`) with two honest,
//! working stand-ins rather than a fabricated "dialog" that doesn't
//! show anything to anyone.

use common::PermissionLevel;

#[derive(Debug, Clone)]
pub struct ConfirmationPrompt {
    pub tool_name: String,
    pub permission_level: PermissionLevel,
    pub description: String,
}

pub trait ConfirmationHandler: Send + Sync {
    fn confirm(&self, prompt: &ConfirmationPrompt) -> bool;
}

/// The safe default when nothing else is configured: PRD SS6.6 says the
/// assistant "must refuse unsafe or ambiguous destructive actions unless
/// explicitly confirmed", and an unconfigured confirmation channel is
/// exactly that ambiguous case.
#[derive(Debug, Default, Clone, Copy)]
pub struct AlwaysDeny;

impl ConfirmationHandler for AlwaysDeny {
    fn confirm(&self, _prompt: &ConfirmationPrompt) -> bool {
        false
    }
}

/// Approves everything. For scripted/testing contexts only -- **never**
/// a sensible default, since auto-approving defeats the entire point of
/// asking. Intentionally verbose to name, so it can't be reached for by
/// accident.
#[derive(Debug, Default, Clone, Copy)]
pub struct AlwaysApproveForTestingOnly;

impl ConfirmationHandler for AlwaysApproveForTestingOnly {
    fn confirm(&self, _prompt: &ConfirmationPrompt) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prompt() -> ConfirmationPrompt {
        ConfirmationPrompt {
            tool_name: "delete_file".to_string(),
            permission_level: PermissionLevel::Destructive,
            description: "delete a file".to_string(),
        }
    }

    #[test]
    fn always_deny_denies() {
        assert!(!AlwaysDeny.confirm(&prompt()));
    }

    #[test]
    fn always_approve_approves() {
        assert!(AlwaysApproveForTestingOnly.confirm(&prompt()));
    }
}
