//! Wires a representative subset of this crate's capabilities up as real
//! [`tool_framework::Tool`] implementations -- the first real tools
//! Phase 7's registry/router/validation pipeline has ever had to route
//! to, replacing Phase 7's `EchoTool`/`SlowTool`/`DangerousStubTool`
//! stand-ins with genuine Linux operations.
//!
//! Not every capability in this crate is wrapped here. Read-only,
//! low-risk operations (system info, listing) are; destructive or
//! desktop-dependent ones (delete, kill, power, clipboard/audio/
//! notifications) are left as direct functions for now, callable once
//! there's a concrete caller and a real Permission Manager (Phase 9) to
//! design the confirmation flow against, rather than wrapped speculatively.

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

pub struct SystemInfoTool;

impl Tool for SystemInfoTool {
    fn name(&self) -> &str {
        "system_info"
    }
    fn description(&self) -> &str {
        "Reports CPU core count, memory, uptime, and kernel version."
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::System
    }
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
    fn timeout(&self) -> Duration {
        Duration::from_secs(5)
    }
    fn input_schema(&self) -> InputSchema {
        InputSchema::default()
    }
    fn execute(&self, _args: &ToolArgs) -> ToolOutcome {
        match crate::system::system_info() {
            Ok(info) => ToolOutcome {
                success: true,
                message: format!(
                    "{} core(s), {}MB available of {}MB total, up {:.0}s, kernel {}",
                    info.cpu_cores,
                    info.memory_available_kb / 1024,
                    info.memory_total_kb / 1024,
                    info.uptime_seconds,
                    info.kernel_version
                ),
                data: Some(serde_json::json!({
                    "cpu_cores": info.cpu_cores,
                    "cpu_model": info.cpu_model,
                    "memory_total_kb": info.memory_total_kb,
                    "memory_available_kb": info.memory_available_kb,
                    "uptime_seconds": info.uptime_seconds,
                    "kernel_version": info.kernel_version,
                })),
            },
            Err(e) => err_outcome(e),
        }
    }
}

pub struct DiskUsageTool;

impl Tool for DiskUsageTool {
    fn name(&self) -> &str {
        "disk_usage"
    }
    fn description(&self) -> &str {
        "Reports total/used/available disk space for a given path."
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::System
    }
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
    fn timeout(&self) -> Duration {
        Duration::from_secs(5)
    }
    fn input_schema(&self) -> InputSchema {
        InputSchema::new(vec![ParamSchema::required("path", ParamKind::String)])
    }
    fn execute(&self, args: &ToolArgs) -> ToolOutcome {
        let Some(path) = args.get("path").and_then(|v| v.as_str()) else {
            return err_outcome("missing required parameter \"path\"");
        };
        match crate::system::disk_usage(Path::new(path)) {
            Ok(usage) => ToolOutcome {
                success: true,
                message: format!(
                    "{} available of {} total on {}",
                    format_kb(usage.available_kb),
                    format_kb(usage.total_kb),
                    usage.mounted_on
                ),
                data: Some(serde_json::json!({
                    "total_kb": usage.total_kb,
                    "used_kb": usage.used_kb,
                    "available_kb": usage.available_kb,
                    "mounted_on": usage.mounted_on,
                })),
            },
            Err(e) => err_outcome(e),
        }
    }
}

fn format_kb(kb: u64) -> String {
    if kb > 1024 * 1024 {
        format!("{:.1}GB", kb as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1}MB", kb as f64 / 1024.0)
    }
}

pub struct ListDirectoryTool;

impl Tool for ListDirectoryTool {
    fn name(&self) -> &str {
        "list_directory"
    }
    fn description(&self) -> &str {
        "Lists the files and subdirectories in a given directory."
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::FileManagement
    }
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
        match crate::files::list_directory(Path::new(path)) {
            Ok(entries) => {
                let data = entries
                    .iter()
                    .map(|e| serde_json::json!({ "name": e.name, "is_dir": e.is_dir, "size": e.size }))
                    .collect::<Vec<_>>();
                ToolOutcome {
                    success: true,
                    message: format!("{} entries in {path}", entries.len()),
                    data: Some(serde_json::json!(data)),
                }
            }
            Err(e) => err_outcome(e),
        }
    }
}

pub struct ReadFileTool;

impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }
    fn description(&self) -> &str {
        "Reads the contents of a text file."
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::FileManagement
    }
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
        match crate::files::read_file(Path::new(path)) {
            Ok(content) => ToolOutcome {
                success: true,
                message: format!("read {} bytes from {path}", content.len()),
                data: Some(serde_json::json!({ "content": content })),
            },
            Err(e) => err_outcome(e),
        }
    }
}

pub struct ListProcessesTool;

impl Tool for ListProcessesTool {
    fn name(&self) -> &str {
        "list_processes"
    }
    fn description(&self) -> &str {
        "Lists running processes with their PID, name, and state."
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::ProcessManagement
    }
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
    fn timeout(&self) -> Duration {
        Duration::from_secs(10)
    }
    fn input_schema(&self) -> InputSchema {
        InputSchema::default()
    }
    fn execute(&self, _args: &ToolArgs) -> ToolOutcome {
        match crate::processes::list_processes() {
            Ok(processes) => {
                let data = processes
                    .iter()
                    .map(|p| serde_json::json!({ "pid": p.pid, "name": p.name, "state": p.state }))
                    .collect::<Vec<_>>();
                ToolOutcome {
                    success: true,
                    message: format!("{} processes", processes.len()),
                    data: Some(serde_json::json!(data)),
                }
            }
            Err(e) => err_outcome(e),
        }
    }
}

pub struct RunTerminalCommandTool;

impl Tool for RunTerminalCommandTool {
    fn name(&self) -> &str {
        "run_terminal_command"
    }
    fn description(&self) -> &str {
        "Runs an approved, allowlisted command (see linux_services::terminal::ALLOWED_COMMANDS). Not arbitrary shell execution."
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::Terminal
    }
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Safe
    }
    fn timeout(&self) -> Duration {
        Duration::from_secs(15)
    }
    fn input_schema(&self) -> InputSchema {
        InputSchema::new(vec![
            ParamSchema::required("command", ParamKind::String),
            ParamSchema::optional("args", ParamKind::StringArray),
        ])
    }
    fn execute(&self, args: &ToolArgs) -> ToolOutcome {
        let Some(command) = args.get("command").and_then(|v| v.as_str()) else {
            return err_outcome("missing required parameter \"command\"");
        };
        let command_args: Vec<String> = args
            .get("args")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();

        match crate::terminal::execute(command, &command_args, Duration::from_secs(10)) {
            Ok(output) => ToolOutcome {
                success: output.success,
                message: if output.success {
                    output.stdout.trim().to_string()
                } else {
                    format!("exit code {:?}: {}", output.exit_code, output.stderr.trim())
                },
                data: Some(serde_json::json!({
                    "stdout": output.stdout,
                    "stderr": output.stderr,
                    "exit_code": output.exit_code,
                })),
            },
            Err(e) => err_outcome(e),
        }
    }
}

pub struct ListPackagesTool;

impl Tool for ListPackagesTool {
    fn name(&self) -> &str {
        "list_packages"
    }
    fn description(&self) -> &str {
        "Lists installed system packages."
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::PackageManagement
    }
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
    fn timeout(&self) -> Duration {
        Duration::from_secs(15)
    }
    fn input_schema(&self) -> InputSchema {
        InputSchema::default()
    }
    fn execute(&self, _args: &ToolArgs) -> ToolOutcome {
        match crate::packages::list_installed() {
            Ok(packages) => ToolOutcome {
                success: true,
                message: format!("{} packages installed", packages.len()),
                data: Some(serde_json::json!(packages
                    .iter()
                    .map(|p| serde_json::json!({ "name": p.name, "version": p.version }))
                    .collect::<Vec<_>>())),
            },
            Err(e) => err_outcome(e),
        }
    }
}

pub struct SearchPackagesTool;

impl Tool for SearchPackagesTool {
    fn name(&self) -> &str {
        "search_packages"
    }
    fn description(&self) -> &str {
        "Searches available packages by name/description."
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::PackageManagement
    }
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
    fn timeout(&self) -> Duration {
        Duration::from_secs(15)
    }
    fn input_schema(&self) -> InputSchema {
        InputSchema::new(vec![ParamSchema::required("query", ParamKind::String)])
    }
    fn execute(&self, args: &ToolArgs) -> ToolOutcome {
        let Some(query) = args.get("query").and_then(|v| v.as_str()) else {
            return err_outcome("missing required parameter \"query\"");
        };
        match crate::packages::search(query) {
            Ok(results) => ToolOutcome {
                success: true,
                message: format!("{} match(es) for {query:?}", results.len()),
                data: Some(serde_json::json!(results)),
            },
            Err(e) => err_outcome(e),
        }
    }
}

/// Registers every tool in this module, for `app` to call once at
/// startup.
pub fn register_all(registry: &mut tool_framework::ToolRegistry) -> common::AppResult<()> {
    use std::sync::Arc;
    registry.register(Arc::new(SystemInfoTool))?;
    registry.register(Arc::new(DiskUsageTool))?;
    registry.register(Arc::new(ListDirectoryTool))?;
    registry.register(Arc::new(ReadFileTool))?;
    registry.register(Arc::new(ListProcessesTool))?;
    registry.register(Arc::new(RunTerminalCommandTool))?;
    registry.register(Arc::new(ListPackagesTool))?;
    registry.register(Arc::new(SearchPackagesTool))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tool_framework::{ToolRegistry, ToolRouter};

    fn router() -> ToolRouter {
        let mut registry = ToolRegistry::new();
        register_all(&mut registry).unwrap();
        ToolRouter::new(registry, PermissionLevel::Safe)
    }

    #[tokio::test]
    async fn system_info_tool_runs_through_the_real_pipeline() {
        let result = router().execute("system_info", ToolArgs::new()).await;
        assert!(result.success, "{}", result.message);
    }

    #[tokio::test]
    async fn list_directory_tool_lists_a_real_directory() {
        let mut args = ToolArgs::new();
        args.insert("path".into(), serde_json::json!("/tmp"));
        let result = router().execute("list_directory", args).await;
        assert!(result.success, "{}", result.message);
    }

    #[tokio::test]
    async fn list_directory_tool_reports_a_missing_directory_as_data_not_a_crash() {
        let mut args = ToolArgs::new();
        args.insert(
            "path".into(),
            serde_json::json!("/definitely/not/a/real/path"),
        );
        let result = router().execute("list_directory", args).await;
        assert!(!result.success);
    }

    #[tokio::test]
    async fn run_terminal_command_tool_only_allows_the_allowlist() {
        let mut args = ToolArgs::new();
        args.insert("command".into(), serde_json::json!("rm"));
        args.insert("args".into(), serde_json::json!(["-rf", "/"]));
        let result = router().execute("run_terminal_command", args).await;
        assert!(!result.success);
    }

    #[tokio::test]
    async fn run_terminal_command_tool_runs_an_allowed_command() {
        let mut args = ToolArgs::new();
        args.insert("command".into(), serde_json::json!("echo"));
        args.insert("args".into(), serde_json::json!(["hello"]));
        let result = router().execute("run_terminal_command", args).await;
        assert!(result.success);
        assert_eq!(result.message, "hello");
    }

    #[tokio::test]
    async fn all_tools_are_registered() {
        let router = router();
        assert_eq!(
            router.registry().list(),
            vec![
                "disk_usage",
                "list_directory",
                "list_packages",
                "list_processes",
                "read_file",
                "run_terminal_command",
                "search_packages",
                "system_info",
            ]
        );
    }
}
