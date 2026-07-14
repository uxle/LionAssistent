//! Security policy (PRD SS6.6; Ai_reasoning SS17). Pure decision logic,
//! no I/O -- what to actually *do* with the decision (log it, ask the
//! user, remember it) lives in `lib.rs`.

use common::PermissionLevel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionDecision {
    Allow,
    Deny,
    RequiresConfirmation,
}

/// Configurable policy. Defaults are deliberately conservative (PRD
/// SS6.6: "Root-level actions must be blocked by default").
#[derive(Debug, Clone, Copy)]
pub struct SecurityRules {
    /// Administrator-level actions are refused outright unless this is
    /// explicitly turned on -- turning it on doesn't skip confirmation
    /// for them, it just allows them to *reach* the confirmation step
    /// instead of being blocked before that.
    pub allow_administrator_actions: bool,
    /// Tools at or below this level run with no confirmation at all.
    pub auto_approve_at_or_below: PermissionLevel,
}

impl Default for SecurityRules {
    fn default() -> Self {
        Self {
            allow_administrator_actions: false,
            auto_approve_at_or_below: PermissionLevel::Safe,
        }
    }
}

impl SecurityRules {
    pub fn decide(&self, level: PermissionLevel) -> PermissionDecision {
        if level == PermissionLevel::Administrator && !self.allow_administrator_actions {
            return PermissionDecision::Deny;
        }
        if level <= self.auto_approve_at_or_below {
            return PermissionDecision::Allow;
        }
        PermissionDecision::RequiresConfirmation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_only_is_allowed_by_default() {
        assert_eq!(
            SecurityRules::default().decide(PermissionLevel::ReadOnly),
            PermissionDecision::Allow
        );
    }

    #[test]
    fn destructive_requires_confirmation_by_default() {
        assert_eq!(
            SecurityRules::default().decide(PermissionLevel::Destructive),
            PermissionDecision::RequiresConfirmation
        );
    }

    #[test]
    fn administrator_is_denied_by_default() {
        assert_eq!(
            SecurityRules::default().decide(PermissionLevel::Administrator),
            PermissionDecision::Deny
        );
    }

    #[test]
    fn administrator_can_be_explicitly_enabled_but_still_needs_confirmation() {
        let rules = SecurityRules {
            allow_administrator_actions: true,
            ..SecurityRules::default()
        };
        assert_eq!(
            rules.decide(PermissionLevel::Administrator),
            PermissionDecision::RequiresConfirmation
        );
    }

    #[test]
    fn auto_approve_threshold_is_configurable() {
        let rules = SecurityRules {
            auto_approve_at_or_below: PermissionLevel::Moderate,
            ..SecurityRules::default()
        };
        assert_eq!(
            rules.decide(PermissionLevel::Moderate),
            PermissionDecision::Allow
        );
        assert_eq!(
            rules.decide(PermissionLevel::Destructive),
            PermissionDecision::RequiresConfirmation
        );
    }
}
