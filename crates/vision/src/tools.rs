//! Wires image analysis up as a real [`tool_framework::Tool`], following
//! `linux-services::tools`' exact shape.

use crate::{describe_image, MetadataOnlyVisionDescriber};
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

pub struct DescribeImageTool;

impl Tool for DescribeImageTool {
    fn name(&self) -> &str {
        "describe_image"
    }
    fn description(&self) -> &str {
        "Reports real image metadata (dimensions, format, color) for a file, plus a description \
         from whichever vision backend is configured (a metadata-only stub unless a local \
         vision-language model has been wired in)."
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Vision
    }
    /// `ReadOnly`, matching `read_file` and `extract_text_from_image`'s
    /// precedent -- see `ocr::tools::ExtractTextFromImageTool`'s doc
    /// comment for the reasoning, which applies identically here: a
    /// caller-named existing file, not a tool that decides for itself
    /// what to expose.
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
    fn timeout(&self) -> Duration {
        Duration::from_secs(10)
    }
    fn input_schema(&self) -> InputSchema {
        InputSchema::new(vec![ParamSchema::required("path", ParamKind::String)])
    }
    fn execute(&self, args: &ToolArgs) -> ToolOutcome {
        let Some(path) = args.get("path").and_then(|v| v.as_str()) else {
            return err_outcome("missing required parameter \"path\"");
        };

        match describe_image(Path::new(path), &MetadataOnlyVisionDescriber) {
            Ok((info, description)) => ToolOutcome {
                success: true,
                message: description,
                data: Some(serde_json::json!({
                    "width": info.width,
                    "height": info.height,
                    "format": info.format,
                    "file_size_bytes": info.file_size_bytes,
                    "mean_red": info.mean_red,
                    "mean_green": info.mean_green,
                    "mean_blue": info.mean_blue,
                })),
            },
            Err(e) => err_outcome(e),
        }
    }
}

pub fn register_all(registry: &mut tool_framework::ToolRegistry) -> common::AppResult<()> {
    use std::sync::Arc;
    registry.register(Arc::new(DescribeImageTool))?;
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
        assert_eq!(registry.list(), vec!["describe_image"]);
    }

    #[tokio::test]
    async fn a_real_image_produces_a_real_successful_result() {
        let dir = std::env::temp_dir().join("vision-tool-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("solid.png");
        let img = image::RgbImage::from_fn(4, 4, |_, _| image::Rgb([10, 20, 30]));
        image::DynamicImage::ImageRgb8(img).save(&path).unwrap();

        let router = router();
        let mut args = ToolArgs::new();
        args.insert("path".into(), serde_json::json!(path.to_str().unwrap()));
        let result = router.execute("describe_image", args).await;

        assert_eq!(result.status, ResultStatus::Ok, "{}", result.message);
        let data = result.data.unwrap();
        assert_eq!(data["width"], 4);
        assert_eq!(data["height"], 4);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn missing_path_fails_validation() {
        let router = router();
        let result = router.execute("describe_image", ToolArgs::new()).await;
        assert_eq!(result.status, ResultStatus::ValidationFailed);
    }
}
