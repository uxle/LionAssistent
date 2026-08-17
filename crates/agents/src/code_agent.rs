//! Wraps `coding_agent`'s real analysis/diagnostics/template-generation
//! functions (Development_plan Phase 24) behind the
//! [`crate::coordinator::Agent`] interface. Named `code_agent`, not
//! `coding_agent`, specifically so `mod code_agent` here doesn't shadow
//! the `coding-agent` crate dependency's own name in this crate's scope.
//!
//! # Scope: template generation only, not freeform
//!
//! `coding_agent::generation` has two paths -- real deterministic
//! templates, and freeform generation through an
//! `assistant_core::ResponseGenerator`. Only the first is wrapped here.
//! Freeform generation needs a live `Box<dyn ResponseGenerator>`
//! instance managed somewhere, and `coding_agent::tools::
//! GenerateCodeTool::new(generator)` already has a clear place for
//! that -- whoever constructs the `ToolRegistry` supplies it. Giving
//! `CodeAgent` its own second copy of that same responsibility would
//! duplicate a wiring decision that already has a good home, not add
//! real capability. Freeform generation stays reachable through
//! `coding_agent::tools::GenerateCodeTool` / `ToolRouter` directly.
//!
//! # Payload contract
//!
//! Request `{"action": "analyze", "path", "max_files"}` (`max_files`
//! optional) -> reply mirrors `coding_agent::analysis::ProjectInfo`'s
//! fields directly.
//!
//! Request `{"action": "diagnostics", "path"}` -> reply mirrors
//! `coding_agent::diagnostics::DiagnosticReport`'s fields directly.
//!
//! Request `{"action": "generate_template", "kind", "name", "language",
//! "description"}` (`language` optional, defaults `"rust"`;
//! `description` optional) -> reply `{"code": "..."}`.

use crate::coordinator::Agent;
use crate::message::AgentMessage;
use crate::module_err;
use coding_agent::analysis::{analyze_project_with_limit, DEFAULT_MAX_FILES_WALKED};
use coding_agent::diagnostics::run_cargo_diagnostics;
use coding_agent::generation::generate_from_template;
use common::AppResult;
use std::path::Path;

pub struct CodeAgent;

fn get_str<'a>(payload: &'a serde_json::Value, field: &str) -> AppResult<&'a str> {
    payload
        .get(field)
        .and_then(|v| v.as_str())
        .ok_or_else(|| module_err(format!("payload missing required string field {field:?}")))
}

impl Agent for CodeAgent {
    fn name(&self) -> &str {
        "code"
    }

    /// `run_diagnostics` genuinely runs `cargo check` (see
    /// `coding_agent::diagnostics`'s own doc comment on why that's
    /// slower than it looks) -- 120 seconds, matching
    /// `coding_agent::tools::RunDiagnosticsTool`'s own timeout exactly,
    /// rather than this trait's 30-second default.
    fn timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(120)
    }

    fn handle(&self, message: &AgentMessage) -> AppResult<AgentMessage> {
        let action = get_str(&message.payload, "action")?;

        match action {
            "analyze" => {
                let path = get_str(&message.payload, "path")?;
                let max_files = message
                    .payload
                    .get("max_files")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize)
                    .unwrap_or(DEFAULT_MAX_FILES_WALKED);

                let info = analyze_project_with_limit(Path::new(path), max_files)?;
                Ok(message.reply(serde_json::json!({
                    "file_count": info.file_count,
                    "language_breakdown": info.language_breakdown,
                    "manifests_found": info.manifests_found,
                    "primary_manifest": info.primary_manifest.map(|m| serde_json::json!({
                        "kind": m.kind, "name": m.name, "dependency_count": m.dependency_count,
                    })),
                    "truncated": info.truncated,
                })))
            }
            "diagnostics" => {
                let path = get_str(&message.payload, "path")?;
                let report = run_cargo_diagnostics(Path::new(path))?;
                Ok(message.reply(serde_json::json!({
                    "success": report.success,
                    "error_count": report.error_count(),
                    "warning_count": report.warning_count(),
                    "diagnostics": report.diagnostics.iter().map(|d| serde_json::json!({
                        "level": d.level, "file": d.file, "line": d.line,
                        "column": d.column, "message": d.message, "code": d.code,
                    })).collect::<Vec<_>>(),
                })))
            }
            "generate_template" => {
                let kind = get_str(&message.payload, "kind")?;
                let name = get_str(&message.payload, "name")?;
                let language = message
                    .payload
                    .get("language")
                    .and_then(|v| v.as_str())
                    .unwrap_or("rust");
                let description = message.payload.get("description").and_then(|v| v.as_str());

                let code = generate_from_template(kind, name, language, description)?;
                Ok(message.reply(serde_json::json!({ "code": code })))
            }
            other => Err(module_err(format!(
                "unknown action {other:?} -- expected \"analyze\", \"diagnostics\", or \
                 \"generate_template\""
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::Priority;

    #[test]
    fn analyzes_this_very_workspaces_cache_crate_through_a_message() {
        let agent = CodeAgent;
        let cache_crate = Path::new(env!("CARGO_MANIFEST_DIR")).join("../cache");
        let message = AgentMessage::new(
            "test",
            "code",
            serde_json::json!({"action": "analyze", "path": cache_crate.to_str().unwrap()}),
            Priority::Normal,
        );
        let reply = agent.handle(&message).unwrap();

        assert!(reply.payload["file_count"].as_u64().unwrap() > 0);
        assert_eq!(reply.payload["primary_manifest"]["kind"], "cargo");
        assert_eq!(reply.payload["primary_manifest"]["name"], "cache");
    }

    #[test]
    fn runs_real_diagnostics_against_a_deliberately_broken_scratch_crate() {
        let agent = CodeAgent;
        let dir = std::env::temp_dir().join("code-agent-test-diagnostics");
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"code-agent-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("src/lib.rs"),
            "pub fn broken() -> i32 {\n    \"not an integer\"\n}\n",
        )
        .unwrap();

        let message = AgentMessage::new(
            "test",
            "code",
            serde_json::json!({"action": "diagnostics", "path": dir.to_str().unwrap()}),
            Priority::Normal,
        );
        let reply = agent.handle(&message).unwrap();

        assert_eq!(reply.payload["success"], false);
        assert_eq!(reply.payload["error_count"], 1);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn generates_a_real_compilable_template_through_a_message() {
        let agent = CodeAgent;
        let message = AgentMessage::new(
            "test",
            "code",
            serde_json::json!({"action": "generate_template", "kind": "function", "name": "hello"}),
            Priority::Normal,
        );
        let reply = agent.handle(&message).unwrap();
        assert!(reply.payload["code"]
            .as_str()
            .unwrap()
            .contains("pub fn hello"));
    }

    #[test]
    fn unknown_action_is_a_clean_error() {
        let agent = CodeAgent;
        let message = AgentMessage::new(
            "test",
            "code",
            serde_json::json!({"action": "levitate"}),
            Priority::Normal,
        );
        assert!(agent.handle(&message).is_err());
    }
}
