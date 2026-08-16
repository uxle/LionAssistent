//! Wires text extraction up as a real [`tool_framework::Tool`], following
//! `linux-services::tools`' exact shape.

use crate::{extract_text, DEFAULT_LANGUAGE};
use std::path::Path;
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

pub struct ExtractTextFromImageTool;

impl Tool for ExtractTextFromImageTool {
    fn name(&self) -> &str {
        "extract_text_from_image"
    }
    fn description(&self) -> &str {
        "Extracts any text visible in an image file via OCR."
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Vision
    }
    /// `ReadOnly`, matching `read_file`'s precedent exactly: like
    /// `read_file`, this takes a caller-named path to something that
    /// already exists rather than deciding for itself what to expose
    /// (contrast `screenshot`'s `capture_screenshot`, which is
    /// `Moderate` for exactly that reason -- see its doc comment). This
    /// tool can only ever read *less* than `read_file` already can (text
    /// recognized in one named image, not arbitrary file bytes), so
    /// holding it to a stricter standard than `read_file` itself would
    /// be an inconsistency worth avoiding, not extra caution.
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
    fn timeout(&self) -> Duration {
        Duration::from_secs(20)
    }
    fn input_schema(&self) -> InputSchema {
        InputSchema::new(vec![
            ParamSchema::required("path", ParamKind::String),
            ParamSchema::optional("language", ParamKind::String),
        ])
    }
    fn execute(&self, args: &ToolArgs) -> ToolOutcome {
        let Some(path) = args.get("path").and_then(|v| v.as_str()) else {
            return err_outcome("missing required parameter \"path\"");
        };
        let language = args
            .get("language")
            .and_then(|v| v.as_str())
            .unwrap_or(DEFAULT_LANGUAGE);

        match extract_text(Path::new(path), language) {
            Ok(text) => ToolOutcome {
                success: true,
                message: if text.is_empty() {
                    format!("no text recognized in {path}")
                } else {
                    format!(
                        "recognized {} character(s) of text in {path}",
                        text.chars().count()
                    )
                },
                data: Some(serde_json::json!({ "text": text })),
            },
            Err(e) => err_outcome(e),
        }
    }
}

pub fn register_all(registry: &mut tool_framework::ToolRegistry) -> common::AppResult<()> {
    use std::sync::Arc;
    registry.register(Arc::new(ExtractTextFromImageTool))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tool_framework::{ResultStatus, ToolRegistry, ToolRouter};

    fn router() -> ToolRouter {
        let mut registry = ToolRegistry::new();
        register_all(&mut registry).unwrap();
        ToolRouter::new(registry, PermissionLevel::Safe)
    }

    #[test]
    fn all_tools_are_registered() {
        let mut registry = ToolRegistry::new();
        register_all(&mut registry).unwrap();
        assert_eq!(registry.list(), vec!["extract_text_from_image"]);
    }

    #[tokio::test]
    async fn read_only_tool_runs_without_confirmation_up_to_safe_level() {
        // Safe (1) is still >= ReadOnly (0), so this should be allowed
        // through the router's simple max_auto_level fallback without a
        // PermissionChecker configured at all -- confirming the level
        // set on the tool is actually ReadOnly, not something stricter.
        let router = router();
        let mut args = ToolArgs::new();
        args.insert(
            "path".into(),
            serde_json::json!("/nonexistent/for-permission-check-only.png"),
        );
        let result = router.execute("extract_text_from_image", args).await;
        assert_ne!(result.status, ResultStatus::PermissionDenied);
    }

    #[tokio::test]
    async fn missing_required_path_fails_validation_before_permission_or_execution() {
        let router = router();
        let result = router
            .execute("extract_text_from_image", ToolArgs::new())
            .await;
        assert_eq!(result.status, ResultStatus::ValidationFailed);
    }
}
