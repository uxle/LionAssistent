//! `permission-manager` -- the real Permission Manager (Development_plan
//! Phase 9; TDS SS2 "Permission Manager": check authorization, ask for
//! confirmation, record audit events, all as one flow in practice).
//!
//! Replaces `tool-framework`'s `max_auto_level` threshold (Phase 7's
//! honest stand-in) with the full pipeline Archd SS7 and Apis SS8
//! describe: remembered decisions first, then security policy (PRD
//! SS6.6), asking for confirmation when the policy calls for it, and
//! always recording what happened (DBDMS SS3 "Permissions"/"Logs"
//! tables).

pub mod confirmation;
pub mod rules;

pub use confirmation::{
    AlwaysApproveForTestingOnly, AlwaysDeny, ConfirmationHandler, ConfirmationPrompt,
};
pub use rules::{PermissionDecision, SecurityRules};

use common::{now, AppError, AppResult, Id, PermissionLevel, Timestamp};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

impl Severity {
    fn as_str(self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Warning => "warning",
            Severity::Error => "error",
        }
    }
}

#[derive(Debug, Clone)]
pub struct AuditEntry {
    pub module: String,
    pub severity: String,
    pub message: String,
    pub timestamp: Timestamp,
}

fn create_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS permission_decisions (
            tool       TEXT PRIMARY KEY,
            decision   INTEGER NOT NULL,
            expires_at TEXT
        );

        CREATE TABLE IF NOT EXISTS audit_log (
            id        TEXT PRIMARY KEY,
            module    TEXT NOT NULL,
            severity  TEXT NOT NULL,
            message   TEXT NOT NULL,
            timestamp TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_audit_timestamp ON audit_log(timestamp);
        ",
    )
}

fn db_err(e: rusqlite::Error) -> AppError {
    AppError::Module {
        module: "permission-manager",
        message: e.to_string(),
    }
}

fn format_timestamp(t: Timestamp) -> String {
    t.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
}

fn parse_timestamp(s: &str) -> AppResult<Timestamp> {
    s.parse().map_err(|e: chrono::ParseError| AppError::Module {
        module: "permission-manager",
        message: format!("invalid timestamp {s:?}: {e}"),
    })
}

pub struct PermissionManager {
    conn: Mutex<Connection>,
    rules: SecurityRules,
    confirmation_handler: Box<dyn ConfirmationHandler>,
}

impl PermissionManager {
    pub fn open(
        path: impl AsRef<Path>,
        rules: SecurityRules,
        confirmation_handler: Box<dyn ConfirmationHandler>,
    ) -> AppResult<Self> {
        let conn = Connection::open(path).map_err(db_err)?;
        Self::from_connection(conn, rules, confirmation_handler)
    }

    /// In-memory store: no file, gone when dropped. Useful for tests and
    /// for running with auditing turned off.
    pub fn in_memory(
        rules: SecurityRules,
        confirmation_handler: Box<dyn ConfirmationHandler>,
    ) -> AppResult<Self> {
        let conn = Connection::open_in_memory().map_err(db_err)?;
        Self::from_connection(conn, rules, confirmation_handler)
    }

    fn from_connection(
        conn: Connection,
        rules: SecurityRules,
        confirmation_handler: Box<dyn ConfirmationHandler>,
    ) -> AppResult<Self> {
        create_schema(&conn).map_err(db_err)?;
        Ok(Self {
            conn: Mutex::new(conn),
            rules,
            confirmation_handler,
        })
    }

    fn log(&self, module: &str, severity: Severity, message: &str) -> AppResult<()> {
        let conn = self
            .conn
            .lock()
            .expect("permission-manager db mutex poisoned");
        conn.execute(
            "INSERT INTO audit_log (id, module, severity, message, timestamp) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![Id::new().to_string(), module, severity.as_str(), message, format_timestamp(now())],
        )
        .map_err(db_err)?;
        Ok(())
    }

    /// The full decision process (TDS's `check_permission` +
    /// `request_confirmation` + `log_permission`, since they always
    /// happen together in practice): a remembered decision short-circuits
    /// everything else; otherwise security policy decides, possibly
    /// asking for confirmation. Always logged either way.
    pub fn check_and_log(
        &self,
        tool_name: &str,
        level: PermissionLevel,
        description: &str,
    ) -> AppResult<bool> {
        if let Some(remembered) = self.recall_decision(tool_name)? {
            self.log(
                "permission-manager",
                Severity::Info,
                &format!("{tool_name}: using remembered decision ({remembered})"),
            )?;
            return Ok(remembered);
        }

        let decision = self.rules.decide(level);
        let approved = match decision {
            PermissionDecision::Allow => true,
            PermissionDecision::Deny => false,
            PermissionDecision::RequiresConfirmation => {
                let prompt = ConfirmationPrompt {
                    tool_name: tool_name.to_string(),
                    permission_level: level,
                    description: description.to_string(),
                };
                self.confirmation_handler.confirm(&prompt)
            }
        };

        self.log(
            "permission-manager",
            Severity::Info,
            &format!(
                "{tool_name}: {decision:?} at level {level:?} -> {}",
                if approved { "approved" } else { "denied" }
            ),
        )?;

        Ok(approved)
    }

    /// Remembers a decision so future calls for `tool_name` skip
    /// straight to it (DBDMS SS3 "Permissions" table's `expires` field).
    /// `ttl: None` means "remember until explicitly revoked".
    pub fn remember_decision(
        &self,
        tool_name: &str,
        decision: bool,
        ttl: Option<Duration>,
    ) -> AppResult<()> {
        let expires_at = ttl
            .and_then(|d| chrono::Duration::from_std(d).ok())
            .map(|d| format_timestamp(now() + d));
        let conn = self
            .conn
            .lock()
            .expect("permission-manager db mutex poisoned");
        conn.execute(
            "INSERT INTO permission_decisions (tool, decision, expires_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(tool) DO UPDATE SET decision = excluded.decision, expires_at = excluded.expires_at",
            params![tool_name, decision as i64, expires_at],
        )
        .map_err(db_err)?;
        Ok(())
    }

    /// Forgets any remembered decision for `tool_name`. Returns whether
    /// there was one to forget.
    pub fn revoke(&self, tool_name: &str) -> AppResult<bool> {
        let conn = self
            .conn
            .lock()
            .expect("permission-manager db mutex poisoned");
        let changed = conn
            .execute(
                "DELETE FROM permission_decisions WHERE tool = ?1",
                params![tool_name],
            )
            .map_err(db_err)?;
        Ok(changed > 0)
    }

    fn recall_decision(&self, tool_name: &str) -> AppResult<Option<bool>> {
        let row: Option<(i64, Option<String>)> = {
            let conn = self
                .conn
                .lock()
                .expect("permission-manager db mutex poisoned");
            conn.query_row(
                "SELECT decision, expires_at FROM permission_decisions WHERE tool = ?1",
                params![tool_name],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(db_err)?
        };

        let Some((decision, expires_at)) = row else {
            return Ok(None);
        };

        if let Some(expires_at) = expires_at {
            if now() >= parse_timestamp(&expires_at)? {
                return Ok(None); // expired -- treat as never remembered
            }
        }

        Ok(Some(decision != 0))
    }

    /// Most recent audit entries, newest first.
    pub fn audit_log(&self, limit: usize) -> AppResult<Vec<AuditEntry>> {
        let conn = self
            .conn
            .lock()
            .expect("permission-manager db mutex poisoned");
        let mut stmt = conn
            .prepare("SELECT module, severity, message, timestamp FROM audit_log ORDER BY timestamp DESC LIMIT ?1")
            .map_err(db_err)?;
        let rows: Vec<(String, String, String, String)> = stmt
            .query_map(params![limit as i64], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .map_err(db_err)?
            .collect::<Result<_, _>>()
            .map_err(db_err)?;
        drop(stmt);
        drop(conn);

        rows.into_iter()
            .map(|(module, severity, message, timestamp)| {
                Ok(AuditEntry {
                    module,
                    severity,
                    message,
                    timestamp: parse_timestamp(&timestamp)?,
                })
            })
            .collect()
    }
}

impl tool_framework::PermissionChecker for PermissionManager {
    fn authorize(&self, tool_name: &str, level: PermissionLevel, description: &str) -> bool {
        // Fail closed: an internal error (a DB problem, say) denies
        // rather than propagating -- a permission system silently
        // defaulting to *allow* on its own internal failure would be
        // exactly backwards.
        self.check_and_log(tool_name, level, description).unwrap_or_else(|e| {
            tracing::error!(error = %e, tool = tool_name, "permission check failed internally; denying");
            false
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manager_with(handler: Box<dyn ConfirmationHandler>) -> PermissionManager {
        PermissionManager::in_memory(SecurityRules::default(), handler).unwrap()
    }

    #[test]
    fn read_only_tools_are_approved_without_asking() {
        let manager = manager_with(Box::new(AlwaysDeny)); // would deny if asked
        assert!(manager
            .check_and_log("system_info", PermissionLevel::ReadOnly, "read system info")
            .unwrap());
    }

    #[test]
    fn destructive_tools_go_through_the_confirmation_handler() {
        let approve = manager_with(Box::new(AlwaysApproveForTestingOnly));
        assert!(approve
            .check_and_log("delete_file", PermissionLevel::Destructive, "delete a file")
            .unwrap());

        let deny = manager_with(Box::new(AlwaysDeny));
        assert!(!deny
            .check_and_log("delete_file", PermissionLevel::Destructive, "delete a file")
            .unwrap());
    }

    #[test]
    fn administrator_is_denied_before_ever_reaching_confirmation() {
        // Even though the handler would approve anything, Administrator
        // is blocked by policy before confirmation is ever asked.
        let manager = manager_with(Box::new(AlwaysApproveForTestingOnly));
        assert!(!manager
            .check_and_log("shutdown", PermissionLevel::Administrator, "shut down")
            .unwrap());
    }

    #[test]
    fn remembered_decisions_skip_the_confirmation_handler() {
        let manager = manager_with(Box::new(AlwaysDeny));
        manager
            .remember_decision("delete_file", true, None)
            .unwrap();
        // AlwaysDeny would refuse if actually asked -- the remembered
        // "true" must be what wins here.
        assert!(manager
            .check_and_log("delete_file", PermissionLevel::Destructive, "delete a file")
            .unwrap());
    }

    #[test]
    fn revoke_removes_a_remembered_decision() {
        let manager = manager_with(Box::new(AlwaysDeny));
        manager
            .remember_decision("delete_file", true, None)
            .unwrap();
        assert!(manager.revoke("delete_file").unwrap());
        // No longer remembered -- falls through to AlwaysDeny.
        assert!(!manager
            .check_and_log("delete_file", PermissionLevel::Destructive, "delete a file")
            .unwrap());
    }

    #[test]
    fn revoking_a_never_remembered_tool_returns_false() {
        let manager = manager_with(Box::new(AlwaysDeny));
        assert!(!manager.revoke("never-remembered").unwrap());
    }

    #[test]
    fn an_expired_remembered_decision_is_not_used() {
        let manager = manager_with(Box::new(AlwaysDeny));
        manager
            .remember_decision("delete_file", true, Some(Duration::from_millis(1)))
            .unwrap();
        std::thread::sleep(Duration::from_millis(20));
        // Expired -- falls through to AlwaysDeny, not the remembered "true".
        assert!(!manager
            .check_and_log("delete_file", PermissionLevel::Destructive, "delete a file")
            .unwrap());
    }

    #[test]
    fn every_check_is_recorded_in_the_audit_log() {
        let manager = manager_with(Box::new(AlwaysApproveForTestingOnly));
        manager
            .check_and_log("system_info", PermissionLevel::ReadOnly, "read system info")
            .unwrap();
        manager
            .check_and_log("delete_file", PermissionLevel::Destructive, "delete a file")
            .unwrap();

        let entries = manager.audit_log(10).unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|e| e.message.contains("system_info")));
        assert!(entries.iter().any(|e| e.message.contains("delete_file")));
    }

    #[test]
    fn permission_checker_trait_impl_fails_closed_semantics_match_check_and_log() {
        use tool_framework::PermissionChecker;
        let manager = manager_with(Box::new(AlwaysApproveForTestingOnly));
        assert!(manager.authorize("system_info", PermissionLevel::ReadOnly, "read system info"));
        assert!(!manager.authorize("shutdown", PermissionLevel::Administrator, "shut down"));
    }
}
