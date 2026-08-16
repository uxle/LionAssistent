//! Wires analysis, generation, and diagnostics up as real
//! [`tool_framework::Tool`]s, following `linux-services::tools`' exact
//! shape.

use crate::analysis::{analyze_project_with_limit, DEFAULT_MAX_FILES_WALKED};
use crate::diagnostics::run_cargo_diagnostics;
use crate::generation::{generate_freeform, generate_from_template};
use assistant_core::{EchoResponseGenerator, ResponseGenerator};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tool_framework::{
    InputSchema, ParamKind, ParamSchema, PermissionLevel, Tool, ToolArgs, ToolCategory, ToolOutcome,
};

fn err_outcome(e: impl std::fmt::Display) -> ToolOutcome {
    ToolOutcome {
        success: false,
        message: e.to_string(),
        data: None,
    }
}

pub struct AnalyzeProjectTool;

impl Tool for AnalyzeProjectTool {
    fn name(&self) -> &str {
        "analyze_project"
    }
    fn description(&self) -> &str {
        "Reports real facts about a code project: file count and language breakdown by \
         extension, and (if found) the parsed manifest's package name and dependency count."
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Development
    }
    /// `ReadOnly`: pure directory traversal and inert-data parsing
    /// (TOML/JSON manifest *contents*, never anything executed) -- the
    /// same risk shape as `read_file`/`list_directory`. Contrast
    /// `RunDiagnosticsTool`, which genuinely does run code and is
    /// permissioned much more cautiously as a result.
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
    fn timeout(&self) -> Duration {
        Duration::from_secs(10)
    }
    fn input_schema(&self) -> InputSchema {
        InputSchema::new(vec![
            ParamSchema::required("path", ParamKind::String),
            ParamSchema::optional("max_files", ParamKind::Integer),
        ])
    }
    fn execute(&self, args: &ToolArgs) -> ToolOutcome {
        let Some(path) = args.get("path").and_then(|v| v.as_str()) else {
            return err_outcome("missing required parameter \"path\"");
        };
        let max_files = args
            .get("max_files")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(DEFAULT_MAX_FILES_WALKED);

        match analyze_project_with_limit(Path::new(path), max_files) {
            Ok(info) => {
                let manifest_summary = info
                    .primary_manifest
                    .as_ref()
                    .map(|m| {
                        format!(
                            ", {} manifest{} ({} dependencies)",
                            m.kind,
                            m.name
                                .as_deref()
                                .map(|n| format!(" for {n}"))
                                .unwrap_or_default(),
                            m.dependency_count
                        )
                    })
                    .unwrap_or_default();
                ToolOutcome {
                    success: true,
                    message: format!(
                        "{} files across {} extension(s){manifest_summary}{}",
                        info.file_count,
                        info.language_breakdown.len(),
                        if info.truncated {
                            " (truncated at the file limit)"
                        } else {
                            ""
                        }
                    ),
                    data: Some(serde_json::json!({
                        "file_count": info.file_count,
                        "language_breakdown": info.language_breakdown,
                        "manifests_found": info.manifests_found,
                        "primary_manifest": info.primary_manifest.map(|m| serde_json::json!({
                            "kind": m.kind,
                            "name": m.name,
                            "dependency_count": m.dependency_count,
                        })),
                        "truncated": info.truncated,
                    })),
                }
            }
            Err(e) => err_outcome(e),
        }
    }
}

pub struct GenerateCodeTool {
    generator: Arc<Mutex<Box<dyn ResponseGenerator>>>,
}

impl GenerateCodeTool {
    pub fn new(generator: Arc<Mutex<Box<dyn ResponseGenerator>>>) -> Self {
        Self { generator }
    }
}

impl Default for GenerateCodeTool {
    /// `EchoResponseGenerator`, the same honest Phase 3 stub
    /// `AssistantCore` itself defaults to -- see this crate's doc
    /// comment for why that's the right default rather than a gap.
    fn default() -> Self {
        Self::new(Arc::new(Mutex::new(
            Box::new(EchoResponseGenerator) as Box<dyn ResponseGenerator>
        )))
    }
}

impl Tool for GenerateCodeTool {
    fn name(&self) -> &str {
        "generate_code"
    }
    fn description(&self) -> &str {
        "Generates code. Provide \"kind\" (\"function\", \"struct\", or \"test\") plus \"name\" \
         for real, deterministic Rust scaffolding, or \"description\" alone for free-form \
         generation through whichever backend is configured (a stub unless a real local model \
         has been wired in)."
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Development
    }
    /// `ReadOnly`: neither generation path touches the filesystem or
    /// runs anything -- pure text in, text out. The lowest-risk tool in
    /// this crate, deliberately: saving the result anywhere is a
    /// separate, already-`Moderate`-permissioned step through
    /// `write_file`, not something this tool does for the caller.
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
    fn timeout(&self) -> Duration {
        Duration::from_secs(30)
    }
    fn input_schema(&self) -> InputSchema {
        InputSchema::new(vec![
            ParamSchema::optional("kind", ParamKind::String),
            ParamSchema::optional("name", ParamKind::String),
            ParamSchema::optional("description", ParamKind::String),
            ParamSchema::optional("language", ParamKind::String),
        ])
    }
    fn execute(&self, args: &ToolArgs) -> ToolOutcome {
        let language = args
            .get("language")
            .and_then(|v| v.as_str())
            .unwrap_or("rust");
        let description = args.get("description").and_then(|v| v.as_str());

        if let Some(kind) = args.get("kind").and_then(|v| v.as_str()) {
            let Some(name) = args.get("name").and_then(|v| v.as_str()) else {
                return err_outcome(
                    "template-based generation (\"kind\" was provided) also requires \"name\"",
                );
            };
            return match generate_from_template(kind, name, language, description) {
                Ok(code) => ToolOutcome {
                    success: true,
                    message: format!("generated a real, compilable {kind} template for {name}"),
                    data: Some(serde_json::json!({ "code": code, "mode": "template" })),
                },
                Err(e) => err_outcome(e),
            };
        }

        let Some(description) = description else {
            return err_outcome(
                "either \"kind\" (template mode) or \"description\" (freeform mode) is required",
            );
        };

        let mut guard = match self.generator.lock() {
            Ok(g) => g,
            Err(e) => return err_outcome(format!("generator lock was poisoned: {e}")),
        };
        let generator: &mut dyn ResponseGenerator = &mut **guard;
        let code = generate_freeform(description, language, generator);
        ToolOutcome {
            success: true,
            message: "generated code from a free-form description".to_string(),
            data: Some(serde_json::json!({ "code": code, "mode": "freeform" })),
        }
    }
}

pub struct RunDiagnosticsTool;

impl Tool for RunDiagnosticsTool {
    fn name(&self) -> &str {
        "run_diagnostics"
    }
    fn description(&self) -> &str {
        "Runs `cargo check` on a Rust project and reports real, structured compiler diagnostics \
         (errors and warnings with file/line/column)."
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Development
    }
    /// `Destructive`, not `ReadOnly` -- see `diagnostics::
    /// run_cargo_diagnostics`'s own doc comment for the full reasoning.
    /// In short: `cargo check` compiles real code, which means running
    /// real `build.rs` scripts and proc-macros with this process's full
    /// privileges, unsandboxed. TCLIS SS6 doesn't have a category for
    /// "looks read-only but can execute arbitrary code depending on
    /// what's in the target directory" -- `Destructive` (Level 3) is
    /// this project's judgment call, reasoned the same way
    /// `screenshot::tools::CaptureScreenshotTool`'s `Moderate` was:
    /// TCLIS SS6's own Level 3 examples include "remove packages",
    /// which carries a comparable "runs code from a project's own
    /// declared dependencies" risk (install scripts), so anchoring here
    /// rather than at `Moderate` keeps similar risks at similar levels
    /// rather than underselling this one because it doesn't delete
    /// anything. `sandbox::FilesystemSandbox` isn't wired into the
    /// spawned `cargo` process in this phase -- a real, documented gap
    /// (docs/ROADMAP.md), not a silent one; the permission-confirmation
    /// gate is this tool's only safeguard today.
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Destructive
    }
    fn timeout(&self) -> Duration {
        Duration::from_secs(120)
    }
    fn input_schema(&self) -> InputSchema {
        InputSchema::new(vec![ParamSchema::required("path", ParamKind::String)])
    }
    fn execute(&self, args: &ToolArgs) -> ToolOutcome {
        let Some(path) = args.get("path").and_then(|v| v.as_str()) else {
            return err_outcome("missing required parameter \"path\"");
        };

        match run_cargo_diagnostics(Path::new(path)) {
            Ok(report) => ToolOutcome {
                success: report.success,
                message: format!(
                    "{} error(s), {} warning(s)",
                    report.error_count(),
                    report.warning_count()
                ),
                data: Some(serde_json::json!({
                    "success": report.success,
                    "diagnostics": report.diagnostics.iter().map(|d| serde_json::json!({
                        "level": d.level,
                        "file": d.file,
                        "line": d.line,
                        "column": d.column,
                        "message": d.message,
                        "code": d.code,
                    })).collect::<Vec<_>>(),
                })),
            },
            Err(e) => err_outcome(e),
        }
    }
}

pub fn register_all(
    registry: &mut tool_framework::ToolRegistry,
    generator: Arc<Mutex<Box<dyn ResponseGenerator>>>,
) -> common::AppResult<()> {
    registry.register(Arc::new(AnalyzeProjectTool))?;
    registry.register(Arc::new(GenerateCodeTool::new(generator)))?;
    registry.register(Arc::new(RunDiagnosticsTool))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tool_framework::{ResultStatus, ToolRegistry, ToolRouter};

    fn router() -> ToolRouter {
        let mut registry = ToolRegistry::new();
        register_all(
            &mut registry,
            Arc::new(Mutex::new(
                Box::new(EchoResponseGenerator) as Box<dyn ResponseGenerator>
            )),
        )
        .unwrap();
        ToolRouter::new(registry, PermissionLevel::Safe)
    }

    #[test]
    fn all_tools_are_registered() {
        let mut registry = ToolRegistry::new();
        register_all(
            &mut registry,
            Arc::new(Mutex::new(
                Box::new(EchoResponseGenerator) as Box<dyn ResponseGenerator>
            )),
        )
        .unwrap();
        let mut names = registry.list();
        names.sort();
        assert_eq!(
            names,
            vec!["analyze_project", "generate_code", "run_diagnostics"]
        );
    }

    #[tokio::test]
    async fn analyze_project_runs_without_confirmation_up_to_safe_level() {
        let router = router();
        let mut args = ToolArgs::new();
        args.insert("path".into(), serde_json::json!(env!("CARGO_MANIFEST_DIR")));
        let result = router.execute("analyze_project", args).await;
        assert_ne!(result.status, ResultStatus::PermissionDenied);
        assert_eq!(result.status, ResultStatus::Ok, "{}", result.message);
    }

    #[tokio::test]
    async fn generate_code_template_mode_end_to_end_through_the_router() {
        let router = router();
        let mut args = ToolArgs::new();
        args.insert("kind".into(), serde_json::json!("function"));
        args.insert("name".into(), serde_json::json!("my_generated_fn"));
        let result = router.execute("generate_code", args).await;
        assert_eq!(result.status, ResultStatus::Ok, "{}", result.message);
        let code = result.data.unwrap()["code"].as_str().unwrap().to_string();
        assert!(code.contains("pub fn my_generated_fn"));
    }

    #[tokio::test]
    async fn generate_code_freeform_mode_end_to_end_through_the_router() {
        let router = router();
        let mut args = ToolArgs::new();
        args.insert(
            "description".into(),
            serde_json::json!("a function that adds two numbers"),
        );
        let result = router.execute("generate_code", args).await;
        assert_eq!(result.status, ResultStatus::Ok, "{}", result.message);
        let code = result.data.unwrap()["code"].as_str().unwrap().to_string();
        assert!(code.contains("stub reply"));
    }

    #[tokio::test]
    async fn generate_code_with_neither_kind_nor_description_fails_validation_clearly() {
        let router = router();
        let result = router.execute("generate_code", ToolArgs::new()).await;
        assert!(!result.success);
    }

    #[tokio::test]
    async fn run_diagnostics_is_gated_above_safe_level_by_the_router() {
        // This crate's own `router()` test helper caps max_auto_level at
        // `Safe` -- `Destructive` is above that, so this should be
        // denied by the router's simple fallback without needing a real
        // `PermissionManager` wired up (that full pipeline is exercised
        // in tests/permission_integration.rs).
        let router = router();
        let mut args = ToolArgs::new();
        args.insert("path".into(), serde_json::json!(env!("CARGO_MANIFEST_DIR")));
        let result = router.execute("run_diagnostics", args).await;
        assert_eq!(result.status, ResultStatus::PermissionDenied);
    }
}
