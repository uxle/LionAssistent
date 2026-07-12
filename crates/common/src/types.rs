use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier used for sessions, messages, tools, memories, and any
/// other entity that needs one (DBDMS SS3 tables all key on an `id` field).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Id(pub Uuid);

impl Id {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for Id {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for Id {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(uuid::Uuid::parse_str(s)?))
    }
}

/// UTC timestamp used throughout the assistant: chat messages, logs, tool
/// executions, and audit records (DBDMS SS3, TCLIS SS9 audit logging).
pub type Timestamp = chrono::DateTime<chrono::Utc>;

pub fn now() -> Timestamp {
    chrono::Utc::now()
}

/// Permission levels shared by the Tool Framework and Permission Manager
/// (Archd SS7, Apis SS8, TCLIS SS6). Ordered from least to most privileged so
/// callers can compare levels directly, e.g. `level >= PermissionLevel::Destructive`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
pub enum PermissionLevel {
    /// Level 0 - read-only information (CPU, RAM, system info).
    ReadOnly = 0,
    /// Level 1 - safe actions (open application, notifications).
    Safe = 1,
    /// Level 2 - user data modification (create, rename, move files).
    Moderate = 2,
    /// Level 3 - destructive actions; requires confirmation.
    Destructive = 3,
    /// Level 4 - administrative actions; requires explicit approval.
    Administrator = 4,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_levels_are_ordered() {
        assert!(PermissionLevel::ReadOnly < PermissionLevel::Safe);
        assert!(PermissionLevel::Destructive < PermissionLevel::Administrator);
    }

    #[test]
    fn ids_are_unique() {
        assert_ne!(Id::new(), Id::new());
    }

    #[test]
    fn id_round_trips_through_display_and_parse() {
        let id = Id::new();
        let parsed: Id = id.to_string().parse().unwrap();
        assert_eq!(id, parsed);
    }
}
