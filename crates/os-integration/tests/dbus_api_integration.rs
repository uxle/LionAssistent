//! Drives `os_integration::dbus_api` against a real D-Bus session
//! bus: a real `dbus-daemon --session` process, a real server thread
//! running the actual `register`/`handle_message` wiring, and a real,
//! separate client connection -- the same "prove the real transport
//! boundary, not just the logic behind it" standard `mcp`'s and
//! `remote-control`'s own real-socket tests already set, applied here
//! to a D-Bus session bus instead of a subprocess pipe or a TCP
//! socket.
//!
//! Confirmed by hand, before writing this file, that `dbus-daemon`
//! can be started directly with an explicit `--address=unix:path=...`
//! (bypassing `dbus-launch`, which isn't installed in this sandbox and
//! which wants an X11 `$DISPLAY` for its own autolaunch fallback
//! regardless) and that a real client/server round trip over it
//! genuinely works with the `dbus` + `dbus-crossroads` crates.
//!
//! Every connection here uses `Connection::new_address(&bus.address)`,
//! never `Connection::new_session()` -- found the empirical way, not
//! anticipated: an early version of this file used `new_session()`
//! (which reads the ambient `$DBUS_SESSION_BUS_ADDRESS`) with each
//! test spinning up its own independent `dbus-daemon`, and every test
//! after the first failed, all trying to reach the *first* test's
//! socket regardless of which one actually set the env var most
//! recently -- even fully serialized (`--test-threads=1`), which rules
//! out a Rust-level race. That points at `libdbus` itself caching
//! session-bus connection state process-wide on the first
//! `dbus_bus_get`-style lookup, not a bug in this file's own
//! synchronization. `new_address` connects to an explicit address
//! directly, with no ambient or cached state involved at all, which
//! is what actually fixed it.

use dbus::blocking::Connection;
use permission_manager::{
    AlwaysApproveForTestingOnly, AlwaysDeny, ConfirmationHandler, PermissionManager, SecurityRules,
};
use std::process::{Child, Command};
use std::sync::Arc;
use std::time::Duration;
use tool_framework::stubs::{DangerousStubTool, EchoTool};
use tool_framework::{PermissionLevel, ToolRegistry, ToolRouter};

/// A real `dbus-daemon --session` on its own private Unix socket,
/// killed when dropped.
struct SessionBus {
    address: String,
    daemon: Child,
}

impl SessionBus {
    fn start(socket_name: &str) -> Self {
        let socket_path = std::env::temp_dir().join(socket_name);
        let _ = std::fs::remove_file(&socket_path);
        let address = format!("unix:path={}", socket_path.display());

        let daemon = Command::new("dbus-daemon")
            .args(["--session", "--nofork", &format!("--address={address}")])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("dbus-daemon must be installed -- see README's Setup section");
        std::thread::sleep(Duration::from_millis(300));

        Self { address, daemon }
    }
}

impl Drop for SessionBus {
    fn drop(&mut self) {
        let _ = self.daemon.kill();
        let _ = self.daemon.wait();
    }
}

fn router_with(handler: Box<dyn ConfirmationHandler>, rules: SecurityRules) -> ToolRouter {
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(EchoTool)).unwrap();
    registry.register(Arc::new(DangerousStubTool)).unwrap();
    let permissions = PermissionManager::in_memory(rules, handler).unwrap();
    ToolRouter::new(registry, PermissionLevel::ReadOnly)
        .with_permission_checker(Arc::new(permissions))
}

/// Starts the real server on a background thread against `bus.address`
/// and returns once it's confirmed accepting connections (a client
/// call to `org.freedesktop.DBus.ListNames` succeeding).
fn start_server(bus: &SessionBus, router: ToolRouter) {
    let address = bus.address.clone();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let handle = rt.handle().clone();
    std::thread::spawn(move || {
        // Keep the runtime alive for the life of the server thread --
        // ExecuteTool's block_on depends on it.
        let _rt = rt;
        let conn = Connection::new_address(&address).expect("server connect failed");
        os_integration::dbus_api::serve(conn, Arc::new(router), handle).expect("serve failed");
    });

    // Poll until the server has actually claimed its well-known bus
    // name, not just until the thread has started.
    for _ in 0..100 {
        std::thread::sleep(Duration::from_millis(20));
        if let Ok(conn) = Connection::new_address(&bus.address) {
            let proxy = conn.with_proxy(
                "org.freedesktop.DBus",
                "/org/freedesktop/DBus",
                Duration::from_secs(1),
            );
            if let Ok((names,)) = proxy.method_call::<(Vec<String>,), _, _, _>(
                "org.freedesktop.DBus",
                "ListNames",
                (),
            ) {
                if names
                    .iter()
                    .any(|n| n == os_integration::dbus_api::BUS_NAME)
                {
                    return;
                }
            }
        }
    }
    panic!(
        "server never claimed {} on the session bus",
        os_integration::dbus_api::BUS_NAME
    );
}

fn client_proxy(
    bus: &SessionBus,
    timeout: Duration,
) -> dbus::blocking::Proxy<'static, &'static Connection> {
    let conn = Connection::new_address(&bus.address).expect("client connect failed");
    // Leak the connection for the test's lifetime -- simpler than
    // threading a lifetime through every call site in a short-lived
    // test process, and each test's dbus-daemon is killed by its own
    // SessionBus's Drop regardless.
    let conn: &'static Connection = Box::leak(Box::new(conn));
    conn.with_proxy(
        os_integration::dbus_api::BUS_NAME,
        os_integration::dbus_api::OBJECT_PATH,
        timeout,
    )
}

#[test]
fn list_tools_over_a_real_session_bus_reports_a_real_registered_tool() {
    let bus = SessionBus::start("os-integration-test-1.sock");
    start_server(
        &bus,
        router_with(
            Box::new(AlwaysApproveForTestingOnly),
            SecurityRules::default(),
        ),
    );

    let proxy = client_proxy(&bus, Duration::from_secs(3));
    let (tools_json,): (String,) = proxy
        .method_call(os_integration::dbus_api::INTERFACE_NAME, "ListTools", ())
        .unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&tools_json).unwrap();
    let names: Vec<&str> = parsed["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"echo"));
}

#[test]
fn execute_tool_over_a_real_session_bus_runs_a_real_tool() {
    let bus = SessionBus::start("os-integration-test-2.sock");
    start_server(
        &bus,
        router_with(
            Box::new(AlwaysApproveForTestingOnly),
            SecurityRules::default(),
        ),
    );

    let proxy = client_proxy(&bus, Duration::from_secs(3));
    let (result_json,): (String,) = proxy
        .method_call(
            os_integration::dbus_api::INTERFACE_NAME,
            "ExecuteTool",
            ("echo", r#"{"text": "hello over d-bus"}"#),
        )
        .unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&result_json).unwrap();
    assert_eq!(parsed["success"], true);
    assert!(parsed["message"]
        .as_str()
        .unwrap()
        .contains("hello over d-bus"));
}

#[test]
fn a_denying_permission_manager_blocks_a_dbus_call_end_to_end() {
    let bus = SessionBus::start("os-integration-test-3.sock");
    start_server(
        &bus,
        router_with(
            Box::new(AlwaysDeny),
            SecurityRules {
                allow_administrator_actions: true,
                ..SecurityRules::default()
            },
        ),
    );

    let proxy = client_proxy(&bus, Duration::from_secs(3));
    let (result_json,): (String,) = proxy
        .method_call(
            os_integration::dbus_api::INTERFACE_NAME,
            "ExecuteTool",
            ("dangerous-stub", "{}"),
        )
        .unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&result_json).unwrap();
    assert_eq!(parsed["success"], false);
    assert_eq!(parsed["status"], "PermissionDenied");
}

#[test]
fn administrator_actions_are_denied_by_default_over_dbus_too() {
    // Mirrors remote-control's own equivalent test: PRD SS6.6's
    // "root-level actions blocked by default" applies over this
    // transport exactly as it does over HTTP and MCP, not just in
    // theory.
    let bus = SessionBus::start("os-integration-test-4.sock");
    start_server(
        &bus,
        router_with(
            Box::new(AlwaysApproveForTestingOnly),
            SecurityRules::default(),
        ),
    );

    let proxy = client_proxy(&bus, Duration::from_secs(3));
    let (result_json,): (String,) = proxy
        .method_call(
            os_integration::dbus_api::INTERFACE_NAME,
            "ExecuteTool",
            ("dangerous-stub", "{}"),
        )
        .unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&result_json).unwrap();
    assert_eq!(parsed["success"], false);
    assert_eq!(parsed["status"], "PermissionDenied");
}

#[test]
fn nonexistent_tool_name_is_a_clean_result_not_a_dbus_error() {
    let bus = SessionBus::start("os-integration-test-5.sock");
    start_server(
        &bus,
        router_with(
            Box::new(AlwaysApproveForTestingOnly),
            SecurityRules::default(),
        ),
    );

    let proxy = client_proxy(&bus, Duration::from_secs(3));
    let (result_json,): (String,) = proxy
        .method_call(
            os_integration::dbus_api::INTERFACE_NAME,
            "ExecuteTool",
            ("does-not-exist", "{}"),
        )
        .unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&result_json).unwrap();
    assert_eq!(parsed["success"], false);
    assert_eq!(parsed["status"], "ToolUnavailable");
}
