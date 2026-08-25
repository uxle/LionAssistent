//! A native D-Bus API (Development_plan Phase 30 "Native AI APIs"):
//! exposes this assistant's own `ToolRouter` to other local
//! applications over D-Bus, the standard IPC mechanism real Linux
//! desktop software already uses to talk to each other -- a more
//! "native" surface than `remote-control`'s HTTP server (Phase 28),
//! which exists for a different audience (a phone on the same LAN,
//! which has no D-Bus access at all).
//!
//! `ExecuteTool` routes through `ToolRouter::execute` -- the exact
//! same validate/permission-check/timeout pipeline any other caller in
//! this workspace already goes through (TCLIS SS1) -- so a D-Bus
//! caller is granted no more trust than any other one, only a
//! different transport to reach the identical gate, the same
//! reasoning `mcp::server` and `remote-control::server` already
//! documented for their own transports.
//!
//! Built on `dbus` + `dbus-crossroads` (verified empirically to
//! compile cleanly under this workspace's pinned Rust 1.75 toolchain
//! and to genuinely complete a client/server round trip over a real
//! session bus before being adopted -- see this crate's Cargo.toml).
//! Bridges into `ToolRouter::execute`'s `async fn` from
//! `dbus-crossroads`'s synchronous method-call callbacks via an
//! explicitly-held `tokio::runtime::Handle::block_on`, the same idiom
//! `mcp::server::serve` and `remote_control::server::serve` already
//! established for the identical constraint -- now a third time.
//!
//! Tool lists and results cross the D-Bus boundary as JSON strings
//! rather than structured D-Bus types: `ToolArgs`/`ToolResult::data`
//! are already `serde_json::Value` internally, and round-tripping
//! that through D-Bus's own (richer, but separate) type-signature
//! system would mean writing and maintaining a second serialization
//! layer for the same data. A single string parameter is simpler,
//! and every D-Bus client library already speaks JSON just as easily
//! as a Rust caller does.

use common::{AppError, AppResult};
use dbus::blocking::Connection;
use dbus::channel::MatchingReceiver;
use dbus_crossroads::Crossroads;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tool_framework::{ToolArgs, ToolRouter};

pub const BUS_NAME: &str = "dev.local_assistant.Assistant";
pub const OBJECT_PATH: &str = "/dev/local_assistant/Assistant";
pub const INTERFACE_NAME: &str = "dev.local_assistant.Assistant1";

struct ServiceState {
    router: Arc<ToolRouter>,
    runtime_handle: tokio::runtime::Handle,
}

fn list_tools_json(router: &ToolRouter) -> String {
    let names = router.registry().list();
    let tools: Vec<_> = names
        .iter()
        .filter_map(|n| router.registry().get(n))
        .map(|t| {
            json!({
                "name": t.name(),
                "description": t.description(),
                "permission_level": format!("{:?}", t.permission_level()),
            })
        })
        .collect();
    json!({ "tools": tools }).to_string()
}

fn execute_tool_json(
    router: &ToolRouter,
    handle: &tokio::runtime::Handle,
    tool_name: &str,
    args_json: &str,
) -> String {
    let args: ToolArgs = match serde_json::from_str::<serde_json::Value>(args_json) {
        Ok(serde_json::Value::Object(map)) => map.into_iter().collect(),
        Ok(serde_json::Value::Null) => ToolArgs::new(),
        Ok(_) => {
            return json!({"success": false, "message": "args must be a JSON object"}).to_string()
        }
        Err(e) => {
            return json!({"success": false, "message": format!("invalid JSON args: {e}")})
                .to_string()
        }
    };

    let result = handle.block_on(router.execute(tool_name, args));
    json!({
        "success": result.success,
        "status": format!("{:?}", result.status),
        "message": result.message,
        "data": result.data,
    })
    .to_string()
}

/// Registers [`BUS_NAME`]/[`OBJECT_PATH`]/[`INTERFACE_NAME`] on
/// `connection` and returns the configured [`Crossroads`] instance,
/// ready to serve. Split out from [`serve`] specifically so tests can
/// drive `cr.handle_message` directly against an in-memory connection
/// pair without needing a full accept-loop running on a background
/// thread for every single assertion.
pub fn register(
    connection: &Connection,
    router: Arc<ToolRouter>,
    runtime_handle: tokio::runtime::Handle,
) -> AppResult<Crossroads> {
    connection
        .request_name(BUS_NAME, false, true, false)
        .map_err(|e| AppError::Module {
            module: "os-integration::dbus_api",
            message: format!("could not claim bus name {BUS_NAME:?}: {e}"),
        })?;

    let mut cr = Crossroads::new();
    let iface_token = cr.register(INTERFACE_NAME, |b| {
        b.method(
            "ListTools",
            (),
            ("tools_json",),
            |_ctx, state: &mut ServiceState, ()| Ok((list_tools_json(&state.router),)),
        );
        b.method(
            "ExecuteTool",
            ("tool_name", "args_json"),
            ("result_json",),
            |_ctx, state: &mut ServiceState, (tool_name, args_json): (String, String)| {
                Ok((execute_tool_json(
                    &state.router,
                    &state.runtime_handle,
                    &tool_name,
                    &args_json,
                ),))
            },
        );
    });
    cr.insert(
        OBJECT_PATH,
        &[iface_token],
        ServiceState {
            router,
            runtime_handle,
        },
    );
    Ok(cr)
}

/// Serves the native API forever. Call from a dedicated thread (or
/// `tokio::task::spawn_blocking`) -- like `remote_control::serve` and
/// `mcp::server::serve_stdio`, this blocks the calling thread for the
/// life of the service.
pub fn serve(
    connection: Connection,
    router: Arc<ToolRouter>,
    runtime_handle: tokio::runtime::Handle,
) -> AppResult<()> {
    let cr = register(&connection, router, runtime_handle)?;
    let cr = std::sync::Mutex::new(cr);
    connection.start_receive(
        dbus::message::MatchRule::new_method_call(),
        Box::new(move |msg, conn| {
            cr.lock()
                .unwrap_or_else(|p| p.into_inner())
                .handle_message(msg, conn)
                .unwrap();
            true
        }),
    );
    loop {
        connection
            .process(Duration::from_secs(1))
            .map_err(|e| AppError::Module {
                module: "os-integration::dbus_api",
                message: format!("D-Bus connection processing failed: {e}"),
            })?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tool_framework::stubs::EchoTool;
    use tool_framework::ToolRegistry;

    #[test]
    fn list_tools_json_reports_a_registered_tool() {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(EchoTool)).unwrap();
        let router = ToolRouter::new(registry, tool_framework::PermissionLevel::ReadOnly);

        let json = list_tools_json(&router);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let names: Vec<&str> = parsed["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"echo"));
    }

    #[test]
    fn execute_tool_json_runs_a_real_tool_and_returns_its_result() {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(EchoTool)).unwrap();
        let router = ToolRouter::new(registry, tool_framework::PermissionLevel::ReadOnly);
        let rt = tokio::runtime::Runtime::new().unwrap();

        let json = execute_tool_json(&router, rt.handle(), "echo", r#"{"text": "hello"}"#);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["success"], true);
        assert!(parsed["message"].as_str().unwrap().contains("hello"));
    }

    #[test]
    fn execute_tool_json_rejects_malformed_json_args_cleanly() {
        let registry = ToolRegistry::new();
        let router = ToolRouter::new(registry, tool_framework::PermissionLevel::ReadOnly);
        let rt = tokio::runtime::Runtime::new().unwrap();

        let json = execute_tool_json(&router, rt.handle(), "echo", "not json");
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["success"], false);
        assert!(parsed["message"].as_str().unwrap().contains("invalid JSON"));
    }

    #[test]
    fn execute_tool_json_rejects_non_object_args_cleanly() {
        let registry = ToolRegistry::new();
        let router = ToolRouter::new(registry, tool_framework::PermissionLevel::ReadOnly);
        let rt = tokio::runtime::Runtime::new().unwrap();

        let json = execute_tool_json(&router, rt.handle(), "echo", "[1, 2, 3]");
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["success"], false);
        assert!(parsed["message"]
            .as_str()
            .unwrap()
            .contains("must be a JSON object"));
    }

    #[test]
    fn missing_args_json_null_is_treated_as_no_arguments() {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(EchoTool)).unwrap();
        let router = ToolRouter::new(registry, tool_framework::PermissionLevel::ReadOnly);
        let rt = tokio::runtime::Runtime::new().unwrap();

        // EchoTool requires "text"; calling with no args should fail
        // validation cleanly, not panic on a missing key.
        let json = execute_tool_json(&router, rt.handle(), "echo", "null");
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["success"], false);
    }
}
