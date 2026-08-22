//! Drives the real, compiled `remote_control::server::serve` over an
//! actual TCP socket on `127.0.0.1` with a real HTTP client (`ureq`),
//! not an in-process mock of either side -- the same "prove the real
//! transport boundary, not just the logic behind it" standard
//! `mcp/tests/client_server_roundtrip.rs` set for Phase 27, applied
//! here to a network socket instead of a subprocess pipe.

use event_bus::{Event, EventBus};
use permission_manager::{AlwaysApproveForTestingOnly, PermissionManager, SecurityRules};
use remote_control::{DeviceStore, NotificationRelay, PairingManager, SharedState};
use std::io::BufRead;
use std::io::BufReader;
use std::sync::Arc;
use std::time::Duration;
use tool_framework::stubs::EchoTool;
use tool_framework::{PermissionLevel, ToolRegistry, ToolRouter};

struct TestServer {
    base: String,
    pairing: Arc<PairingManager>,
    devices: Arc<DeviceStore>,
    bus: EventBus,
    // Kept alive for the life of the test: dropping the Runtime would
    // stop the notification relay's background task and the server's
    // ability to bridge into ToolRouter::execute.
    _rt: tokio::runtime::Runtime,
}

/// Starts the real server on an OS-assigned free port, with a
/// `PermissionManager` that auto-approves everything -- deliberately
/// permissive so these tests isolate *transport and pairing*
/// correctness from permission logic, which
/// `tests/permission_integration.rs` covers on its own.
fn start_test_server() -> TestServer {
    let port = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().port()
    };

    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(EchoTool)).unwrap();
    let router =
        ToolRouter::new(registry, PermissionLevel::ReadOnly).with_permission_checker(Arc::new(
            PermissionManager::in_memory(
                SecurityRules::default(),
                Box::new(AlwaysApproveForTestingOnly),
            )
            .unwrap(),
        ));

    let devices = Arc::new(DeviceStore::in_memory().unwrap());
    let pairing = Arc::new(PairingManager::new());
    let bus = EventBus::default();

    let rt = tokio::runtime::Runtime::new().unwrap();
    let handle = rt.handle().clone();
    let notifications = NotificationRelay::spawn(&bus, &handle);

    let state = SharedState::new(
        Arc::clone(&devices),
        Arc::clone(&pairing),
        router,
        notifications,
        handle,
    );

    std::thread::spawn(move || {
        remote_control::serve("127.0.0.1", port, state, 4).unwrap();
    });
    wait_for_port("127.0.0.1", port);

    TestServer {
        base: format!("http://127.0.0.1:{port}"),
        pairing,
        devices,
        bus,
        _rt: rt,
    }
}

fn wait_for_port(host: &str, port: u16) {
    for _ in 0..100 {
        if std::net::TcpStream::connect((host, port)).is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("server on {host}:{port} never started accepting connections");
}

/// Completes pairing for real: starts a pairing window on the
/// server's own `PairingManager` (standing in for a local UI having
/// already displayed the code) and submits it through the real HTTP
/// endpoint, returning the `(device_id, token)` a genuine paired
/// device would receive.
fn pair_a_real_device(server: &TestServer, label: &str) -> (String, String) {
    let code = server.pairing.begin_pairing(label);
    let body: serde_json::Value = ureq::post(&format!("{}/pair/complete", server.base))
        .send_json(ureq::json!({"code": code.code, "device_label": label}))
        .expect("pairing with the real, just-generated code must succeed")
        .into_json()
        .unwrap();
    (
        body["device_id"]
            .as_str()
            .expect("response must include a device_id")
            .to_string(),
        body["token"]
            .as_str()
            .expect("response must include a token")
            .to_string(),
    )
}

#[test]
fn full_pairing_then_authenticated_status_and_tool_call_over_a_real_socket() {
    let server = start_test_server();
    let (_device_id, token) = pair_a_real_device(&server, "Lion's Phone");

    let status: serde_json::Value = ureq::get(&format!("{}/status", server.base))
        .set("Authorization", &format!("Bearer {token}"))
        .call()
        .unwrap()
        .into_json()
        .unwrap();
    assert_eq!(status["paired_as"], "Lion's Phone");

    let tools: serde_json::Value = ureq::get(&format!("{}/tools", server.base))
        .set("Authorization", &format!("Bearer {token}"))
        .call()
        .unwrap()
        .into_json()
        .unwrap();
    let names: Vec<&str> = tools["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"echo"));

    let result: serde_json::Value = ureq::post(&format!("{}/tools/execute", server.base))
        .set("Authorization", &format!("Bearer {token}"))
        .send_json(ureq::json!({"tool": "echo", "args": {"text": "hello from a real socket"}}))
        .unwrap()
        .into_json()
        .unwrap();
    assert_eq!(result["success"], true);
    assert!(result["message"]
        .as_str()
        .unwrap()
        .contains("hello from a real socket"));
}

#[test]
fn a_pairing_code_is_single_use_over_the_real_http_endpoint_too() {
    let server = start_test_server();
    let code = server.pairing.begin_pairing("Device");
    let first = ureq::post(&format!("{}/pair/complete", server.base))
        .send_json(ureq::json!({"code": code.code, "device_label": "Device"}));
    assert!(
        first.is_ok(),
        "first submission of a valid code must succeed"
    );

    let second = ureq::post(&format!("{}/pair/complete", server.base))
        .send_json(ureq::json!({"code": code.code, "device_label": "Device"}));
    match second {
        Err(ureq::Error::Status(401, _)) => {}
        other => panic!("expected the already-used code to be rejected, got {other:?}"),
    }
}

#[test]
fn wrong_pairing_code_is_rejected_over_the_real_http_endpoint() {
    let server = start_test_server();
    let _code = server.pairing.begin_pairing("Device"); // start a real window, then guess wrong
    let resp = ureq::post(&format!("{}/pair/complete", server.base))
        .send_json(ureq::json!({"code": "000001", "device_label": "Attacker"}));
    match resp {
        Err(ureq::Error::Status(401, r)) => {
            let body: serde_json::Value = r.into_json().unwrap();
            assert!(body["error"]
                .as_str()
                .unwrap()
                .contains("attempts remaining"));
        }
        other => panic!("expected 401 with attempts-remaining detail, got {other:?}"),
    }
}

#[test]
fn revoked_device_is_rejected_on_its_very_next_request() {
    let server = start_test_server();
    let (device_id, token) = pair_a_real_device(&server, "Soon Revoked");

    assert!(
        ureq::get(&format!("{}/status", server.base))
            .set("Authorization", &format!("Bearer {token}"))
            .call()
            .is_ok(),
        "must work before revocation"
    );

    assert!(server.devices.revoke(&device_id).unwrap());

    let err = ureq::get(&format!("{}/status", server.base))
        .set("Authorization", &format!("Bearer {token}"))
        .call()
        .unwrap_err();
    match err {
        ureq::Error::Status(401, _) => {}
        other => panic!("expected a revoked token to be rejected with 401, got {other:?}"),
    }
}

#[test]
fn unauthenticated_requests_to_protected_routes_are_rejected() {
    let server = start_test_server();

    for path in ["/status", "/tools"] {
        let err = ureq::get(&format!("{}{path}", server.base))
            .call()
            .unwrap_err();
        match err {
            ureq::Error::Status(401, _) => {}
            other => panic!("{path}: expected 401, got {other:?}"),
        }
    }

    let err = ureq::post(&format!("{}/tools/execute", server.base))
        .send_json(ureq::json!({"tool": "echo", "args": {"text": "hi"}}))
        .unwrap_err();
    match err {
        ureq::Error::Status(401, _) => {}
        other => panic!("expected 401, got {other:?}"),
    }
}

#[test]
fn a_bogus_bearer_token_is_rejected_the_same_as_no_token() {
    let server = start_test_server();
    let err = ureq::get(&format!("{}/status", server.base))
        .set("Authorization", "Bearer not-a-real-token")
        .call()
        .unwrap_err();
    match err {
        ureq::Error::Status(401, _) => {}
        other => panic!("expected 401, got {other:?}"),
    }
}

#[test]
fn unknown_route_returns_404_not_a_panic_or_hang() {
    let server = start_test_server();
    let err = ureq::get(&format!("{}/this/route/does/not/exist", server.base))
        .call()
        .unwrap_err();
    match err {
        ureq::Error::Status(404, _) => {}
        other => panic!("expected 404, got {other:?}"),
    }
}

#[test]
fn malformed_json_body_is_a_clean_400_not_a_crash() {
    let server = start_test_server();
    let err = ureq::post(&format!("{}/pair/complete", server.base))
        .set("Content-Type", "application/json")
        .send_string("{not valid json")
        .unwrap_err();
    match err {
        ureq::Error::Status(400, _) => {}
        other => panic!("expected 400, got {other:?}"),
    }
}

#[test]
fn nonexistent_tool_name_is_a_clean_result_not_a_500() {
    let server = start_test_server();
    let (_id, token) = pair_a_real_device(&server, "Device");
    let resp = ureq::post(&format!("{}/tools/execute", server.base))
        .set("Authorization", &format!("Bearer {token}"))
        .send_json(ureq::json!({"tool": "this-tool-does-not-exist", "args": {}}));
    let result: serde_json::Value = resp.unwrap().into_json().unwrap();
    assert_eq!(result["success"], false);
    assert_eq!(result["status"], "ToolUnavailable");
}

#[test]
fn static_pwa_shell_is_served_without_authentication() {
    let server = start_test_server();
    let html = ureq::get(&server.base)
        .call()
        .unwrap()
        .into_string()
        .unwrap();
    assert!(html.contains("app.js"));

    let js = ureq::get(&format!("{}/app.js", server.base))
        .call()
        .unwrap();
    assert_eq!(js.content_type(), "application/javascript");
}

#[test]
fn sse_stream_delivers_a_real_event_bus_publish_over_the_real_socket() {
    let server = start_test_server();
    let (_id, token) = pair_a_real_device(&server, "Listener");

    let resp = ureq::get(&format!("{}/events?token={token}", server.base))
        .call()
        .unwrap();
    assert_eq!(resp.content_type(), "text/event-stream");
    let mut reader = BufReader::new(resp.into_reader());

    // Publish only after the connection is confirmed open, avoiding a
    // race where the event fires before the SSE subscriber exists.
    server.bus.publish(Event::ToolExecutionFinished {
        tool: "echo".to_string(),
        success: true,
    });

    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line).unwrap();
        assert!(n > 0, "stream closed before delivering an event");
        if let Some(data) = line.strip_prefix("data: ") {
            let parsed: serde_json::Value = serde_json::from_str(data.trim()).unwrap();
            assert_eq!(parsed["type"], "tool_execution_finished");
            assert_eq!(parsed["tool"], "echo");
            break;
        }
    }
}

#[test]
fn server_survives_many_rapid_unauthenticated_requests_without_deadlocking() {
    let server = start_test_server();
    for _ in 0..25 {
        let _ = ureq::get(&format!("{}/status", server.base)).call();
    }
    // If any handler deadlocked (e.g. a Mutex held across a blocking
    // call it shouldn't be), this would hang instead of returning --
    // ureq's default timeout bounds the test either way.
    let result = ureq::get(&format!("{}/status", server.base)).call();
    assert!(matches!(result, Err(ureq::Error::Status(401, _))));
}
