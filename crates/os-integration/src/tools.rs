//! `tool_framework::Tool` implementations wrapping [`crate::windows`]
//! and [`crate::systemd`], plus [`register_all`] -- the same
//! registration convention `linux_services::tools`, `search::tools`,
//! and every other tool-providing crate in this workspace already
//! follows.
//!
//! ## Permission levels, reasoned explicitly
//!
//! - `list_windows`/`list_user_services`/`user_service_status`:
//!   `ReadOnly` -- pure queries, TCLIS SS6 Level 0.
//! - `activate_window`/`move_window`/`resize_window`: `Safe` --
//!   TCLIS SS6 Level 2's own examples name "move" for *files*, but a
//!   window's on-screen position is transient UI state, not user
//!   data: trivially reversible, no data-loss risk, closer in spirit
//!   to "open application" (Level 1) than to moving a file a user
//!   actually cares about. A deliberate judgment call, stated here
//!   rather than left implicit.
//! - `close_window`: `Moderate` -- unlike a pure position change, a
//!   close request can lose unsaved work if the target application
//!   doesn't handle it gracefully (see `windows`'s own docs on why
//!   this is a polite `_NET_CLOSE_WINDOW`, not a forceful kill, but
//!   even a polite request can't guarantee every application behaves
//!   well). Requiring confirmation here, rather than auto-approving,
//!   matches the same reasoning `rag`'s and `coding-agent`'s own
//!   Moderate-level tools used for "reversible in principle, but not
//!   something to do silently."
//! - `start_user_service`/`stop_user_service`: `Moderate` -- starting
//!   or stopping a background service is a real behavioral change,
//!   generally recoverable (services can be restarted), but not
//!   something to auto-approve either.

use crate::{systemd, windows};
use common::AppResult;
use std::time::Duration;
use tool_framework::{
    InputSchema, ParamKind, ParamSchema, PermissionLevel, Tool, ToolArgs, ToolCategory,
    ToolOutcome, ToolRegistry,
};

fn err_outcome(e: common::AppError) -> ToolOutcome {
    ToolOutcome {
        success: false,
        message: e.to_string(),
        data: None,
    }
}

fn required_str<'a>(args: &'a ToolArgs, key: &str) -> Result<&'a str, ToolOutcome> {
    args.get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolOutcome {
            success: false,
            message: format!("missing required argument {key:?}"),
            data: None,
        })
}

pub struct ListWindowsTool;

impl Tool for ListWindowsTool {
    fn name(&self) -> &str {
        "list_windows"
    }
    fn description(&self) -> &str {
        "Lists every open desktop window (id, desktop number, title)."
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::ApplicationManagement
    }
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }
    fn timeout(&self) -> Duration {
        Duration::from_secs(5)
    }
    fn input_schema(&self) -> InputSchema {
        InputSchema::new(vec![])
    }
    fn execute(&self, _args: &ToolArgs) -> ToolOutcome {
        match windows::list_windows() {
            Ok(list) => {
                let data = serde_json::json!(list
                    .iter()
                    .map(
                        |w| serde_json::json!({"id": w.id, "desktop": w.desktop, "title": w.title})
                    )
                    .collect::<Vec<_>>());
                ToolOutcome {
                    success: true,
                    message: format!("{} window(s) found", list.len()),
                    data: Some(data),
                }
            }
            Err(e) => err_outcome(e),
        }
    }
}

pub struct ActivateWindowTool;

impl Tool for ActivateWindowTool {
    fn name(&self) -> &str {
        "activate_window"
    }
    fn description(&self) -> &str {
        "Brings a window to the front and gives it input focus, by window id (see list_windows)."
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::ApplicationManagement
    }
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Safe
    }
    fn timeout(&self) -> Duration {
        Duration::from_secs(5)
    }
    fn input_schema(&self) -> InputSchema {
        InputSchema::new(vec![ParamSchema::required("id", ParamKind::String)])
    }
    fn execute(&self, args: &ToolArgs) -> ToolOutcome {
        let id = match required_str(args, "id") {
            Ok(id) => id,
            Err(outcome) => return outcome,
        };
        match windows::activate_window(id) {
            Ok(()) => ToolOutcome {
                success: true,
                message: format!("activated window {id}"),
                data: None,
            },
            Err(e) => err_outcome(e),
        }
    }
}

pub struct MoveWindowTool;

impl Tool for MoveWindowTool {
    fn name(&self) -> &str {
        "move_window"
    }
    fn description(&self) -> &str {
        "Moves a window so its top-left corner is at (x, y) in screen coordinates."
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::ApplicationManagement
    }
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Safe
    }
    fn timeout(&self) -> Duration {
        Duration::from_secs(5)
    }
    fn input_schema(&self) -> InputSchema {
        InputSchema::new(vec![
            ParamSchema::required("id", ParamKind::String),
            ParamSchema::required("x", ParamKind::Integer),
            ParamSchema::required("y", ParamKind::Integer),
        ])
    }
    fn execute(&self, args: &ToolArgs) -> ToolOutcome {
        let id = match required_str(args, "id") {
            Ok(id) => id,
            Err(outcome) => return outcome,
        };
        let x = args.get("x").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let y = args.get("y").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        match windows::move_window(id, x, y) {
            Ok(()) => ToolOutcome {
                success: true,
                message: format!("moved window {id} to ({x}, {y})"),
                data: None,
            },
            Err(e) => err_outcome(e),
        }
    }
}

pub struct ResizeWindowTool;

impl Tool for ResizeWindowTool {
    fn name(&self) -> &str {
        "resize_window"
    }
    fn description(&self) -> &str {
        "Resizes a window to the given width and height, in pixels."
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::ApplicationManagement
    }
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Safe
    }
    fn timeout(&self) -> Duration {
        Duration::from_secs(5)
    }
    fn input_schema(&self) -> InputSchema {
        InputSchema::new(vec![
            ParamSchema::required("id", ParamKind::String),
            ParamSchema::required("width", ParamKind::Integer),
            ParamSchema::required("height", ParamKind::Integer),
        ])
    }
    fn execute(&self, args: &ToolArgs) -> ToolOutcome {
        let id = match required_str(args, "id") {
            Ok(id) => id,
            Err(outcome) => return outcome,
        };
        let width = args.get("width").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let height = args.get("height").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        match windows::resize_window(id, width, height) {
            Ok(()) => ToolOutcome {
                success: true,
                message: format!("resized window {id} to {width}x{height}"),
                data: None,
            },
            Err(e) => err_outcome(e),
        }
    }
}

pub struct CloseWindowTool;

impl Tool for CloseWindowTool {
    fn name(&self) -> &str {
        "close_window"
    }
    fn description(&self) -> &str {
        "Politely requests that a window close (the same request its own titlebar close button sends), by window id."
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::ApplicationManagement
    }
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Moderate
    }
    fn timeout(&self) -> Duration {
        Duration::from_secs(5)
    }
    fn input_schema(&self) -> InputSchema {
        InputSchema::new(vec![ParamSchema::required("id", ParamKind::String)])
    }
    fn execute(&self, args: &ToolArgs) -> ToolOutcome {
        let id = match required_str(args, "id") {
            Ok(id) => id,
            Err(outcome) => return outcome,
        };
        match windows::close_window(id) {
            Ok(()) => ToolOutcome {
                success: true,
                message: format!("requested close of window {id}"),
                data: None,
            },
            Err(e) => err_outcome(e),
        }
    }
}

pub struct ListUserServicesTool;

impl Tool for ListUserServicesTool {
    fn name(&self) -> &str {
        "list_user_services"
    }
    fn description(&self) -> &str {
        "Lists systemd user service units and their current state."
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
        InputSchema::new(vec![])
    }
    fn execute(&self, _args: &ToolArgs) -> ToolOutcome {
        match systemd::list_user_services() {
            Ok(list) => {
                let data = serde_json::json!(list
                    .iter()
                    .map(|s| serde_json::json!({
                        "unit": s.unit, "active_state": s.active_state, "sub_state": s.sub_state,
                    }))
                    .collect::<Vec<_>>());
                ToolOutcome {
                    success: true,
                    message: format!("{} unit(s) found", list.len()),
                    data: Some(data),
                }
            }
            Err(e) => err_outcome(e),
        }
    }
}

pub struct UserServiceStatusTool;

impl Tool for UserServiceStatusTool {
    fn name(&self) -> &str {
        "user_service_status"
    }
    fn description(&self) -> &str {
        "Reports one systemd user service unit's current ActiveState/SubState."
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
        InputSchema::new(vec![ParamSchema::required("unit", ParamKind::String)])
    }
    fn execute(&self, args: &ToolArgs) -> ToolOutcome {
        let unit = match required_str(args, "unit") {
            Ok(unit) => unit,
            Err(outcome) => return outcome,
        };
        match systemd::user_service_status(unit) {
            Ok(s) => ToolOutcome {
                success: true,
                message: format!("{} is {} ({})", s.unit, s.active_state, s.sub_state),
                data: Some(
                    serde_json::json!({"active_state": s.active_state, "sub_state": s.sub_state}),
                ),
            },
            Err(e) => err_outcome(e),
        }
    }
}

pub struct StartUserServiceTool;

impl Tool for StartUserServiceTool {
    fn name(&self) -> &str {
        "start_user_service"
    }
    fn description(&self) -> &str {
        "Starts a systemd user service unit."
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::System
    }
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Moderate
    }
    fn timeout(&self) -> Duration {
        Duration::from_secs(15)
    }
    fn input_schema(&self) -> InputSchema {
        InputSchema::new(vec![ParamSchema::required("unit", ParamKind::String)])
    }
    fn execute(&self, args: &ToolArgs) -> ToolOutcome {
        let unit = match required_str(args, "unit") {
            Ok(unit) => unit,
            Err(outcome) => return outcome,
        };
        match systemd::start_user_service(unit) {
            Ok(()) => ToolOutcome {
                success: true,
                message: format!("started {unit}"),
                data: None,
            },
            Err(e) => err_outcome(e),
        }
    }
}

pub struct StopUserServiceTool;

impl Tool for StopUserServiceTool {
    fn name(&self) -> &str {
        "stop_user_service"
    }
    fn description(&self) -> &str {
        "Stops a systemd user service unit."
    }
    fn category(&self) -> ToolCategory {
        ToolCategory::System
    }
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Moderate
    }
    fn timeout(&self) -> Duration {
        Duration::from_secs(15)
    }
    fn input_schema(&self) -> InputSchema {
        InputSchema::new(vec![ParamSchema::required("unit", ParamKind::String)])
    }
    fn execute(&self, args: &ToolArgs) -> ToolOutcome {
        let unit = match required_str(args, "unit") {
            Ok(unit) => unit,
            Err(outcome) => return outcome,
        };
        match systemd::stop_user_service(unit) {
            Ok(()) => ToolOutcome {
                success: true,
                message: format!("stopped {unit}"),
                data: None,
            },
            Err(e) => err_outcome(e),
        }
    }
}

/// Registers every tool in this module into `registry` -- the same
/// fixed-list convention `linux_services::tools::register_all` and
/// every other built-in tool-providing crate in this workspace already
/// follows (unlike `mcp::bridge::register_all`'s dynamically-discovered
/// count, this list is known completely at compile time).
pub fn register_all(registry: &mut ToolRegistry) -> AppResult<()> {
    registry.register(std::sync::Arc::new(ListWindowsTool))?;
    registry.register(std::sync::Arc::new(ActivateWindowTool))?;
    registry.register(std::sync::Arc::new(MoveWindowTool))?;
    registry.register(std::sync::Arc::new(ResizeWindowTool))?;
    registry.register(std::sync::Arc::new(CloseWindowTool))?;
    registry.register(std::sync::Arc::new(ListUserServicesTool))?;
    registry.register(std::sync::Arc::new(UserServiceStatusTool))?;
    registry.register(std::sync::Arc::new(StartUserServiceTool))?;
    registry.register(std::sync::Arc::new(StopUserServiceTool))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_all_registers_exactly_nine_tools_with_no_name_collisions() {
        let mut registry = ToolRegistry::new();
        register_all(&mut registry).unwrap();
        assert_eq!(registry.list().len(), 9);
    }

    #[test]
    fn missing_required_argument_is_a_clean_failure_not_a_panic() {
        let tool = ActivateWindowTool;
        let outcome = tool.execute(&ToolArgs::new());
        assert!(!outcome.success);
        assert!(outcome.message.contains("missing required argument"));
    }

    #[test]
    fn permission_levels_match_this_module_s_own_documented_reasoning() {
        assert_eq!(
            ListWindowsTool.permission_level(),
            PermissionLevel::ReadOnly
        );
        assert_eq!(ActivateWindowTool.permission_level(), PermissionLevel::Safe);
        assert_eq!(MoveWindowTool.permission_level(), PermissionLevel::Safe);
        assert_eq!(ResizeWindowTool.permission_level(), PermissionLevel::Safe);
        assert_eq!(
            CloseWindowTool.permission_level(),
            PermissionLevel::Moderate
        );
        assert_eq!(
            ListUserServicesTool.permission_level(),
            PermissionLevel::ReadOnly
        );
        assert_eq!(
            UserServiceStatusTool.permission_level(),
            PermissionLevel::ReadOnly
        );
        assert_eq!(
            StartUserServiceTool.permission_level(),
            PermissionLevel::Moderate
        );
        assert_eq!(
            StopUserServiceTool.permission_level(),
            PermissionLevel::Moderate
        );
    }

    #[test]
    fn move_window_without_a_real_display_is_a_clean_failure() {
        // No Xvfb running in a plain unit-test process: proves the
        // error path end to end without needing the real X stack this
        // crate's own integration tests set up separately.
        let tool = MoveWindowTool;
        let mut args = ToolArgs::new();
        args.insert("id".into(), serde_json::json!("0x0"));
        args.insert("x".into(), serde_json::json!(10));
        args.insert("y".into(), serde_json::json!(10));
        let outcome = tool.execute(&args);
        assert!(!outcome.success);
    }
}
