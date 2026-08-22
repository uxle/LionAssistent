//! Proves `/tools/execute` genuinely goes through `ToolRouter::execute`
//! -- the same validate/permission-check/timeout pipeline any local
//! caller already goes through -- rather than granting a paired
//! remote device some separate, less-guarded path. Mirrors the exact
//! pattern `rag`'s and `mcp`'s own `permission_integration.rs` files
//! already established: an `AlwaysDeny` checker must block the call
//! end to end, over a real socket, not just in theory.

use permission_manager::{
    AlwaysApproveForTestingOnly, AlwaysDeny, ConfirmationHandler, PermissionManager, SecurityRules,
};
use remote_control::{DeviceStore, NotificationRelay, PairingManager, SharedState};
use std::sync::Arc;
use std::time::Duration;
use tool_framework::stubs::{DangerousStubTool, EchoTool};
use tool_framework::{PermissionLevel, ToolRegistry, ToolRouter};

struct TestServer {
    base: String,
    pairing: Arc<PairingManager>,
    _rt: tokio::runtime::Runtime,
}

fn start_server_with_confirmation_handler(
    handler: Box<dyn ConfirmationHandler>,
    rules: SecurityRules,
) -> TestServer {
    let port = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().port()
    };

    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(EchoTool)).unwrap();
    registry.register(Arc::new(DangerousStubTool)).unwrap();
    let permissions = PermissionManager::in_memory(rules, handler).unwrap();
    let router = ToolRouter::new(registry, PermissionLevel::ReadOnly)
        .with_permission_checker(Arc::new(permissions));

    let devices = Arc::new(DeviceStore::in_memory().unwrap());
    let pairing = Arc::new(PairingManager::new());
    let bus = event_bus::EventBus::default();

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
    for _ in 0..100 {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    TestServer {
        base: format!("http://127.0.0.1:{port}"),
        pairing,
        _rt: rt,
    }
}

fn pair_a_device(server: &TestServer) -> String {
    let code = server.pairing.begin_pairing("Test Device");
    let body: serde_json::Value = ureq::post(&format!("{}/pair/complete", server.base))
        .send_json(ureq::json!({"code": code.code, "device_label": "Test Device"}))
        .unwrap()
        .into_json()
        .unwrap();
    body["token"].as_str().unwrap().to_string()
}

#[test]
fn a_denying_permission_manager_blocks_a_remote_tool_call_end_to_end() {
    let server = start_server_with_confirmation_handler(
        Box::new(AlwaysDeny),
        SecurityRules {
            allow_administrator_actions: true, // reach confirmation, then get denied there
            ..SecurityRules::default()
        },
    );
    let token = pair_a_device(&server);

    // EchoTool is ReadOnly, which never reaches confirmation at all
    // (TCLIS SS6 Level 0) -- proving the *denying* path specifically
    // needs a tool above the router's auto-approve ceiling, hence
    // DangerousStubTool (Administrator).
    let result: serde_json::Value = ureq::post(&format!("{}/tools/execute", server.base))
        .set("Authorization", &format!("Bearer {token}"))
        .send_json(ureq::json!({"tool": "dangerous-stub", "args": {}}))
        .unwrap()
        .into_json()
        .unwrap();

    assert_eq!(result["success"], false);
    assert_eq!(result["status"], "PermissionDenied");
    assert!(
        !result["message"]
            .as_str()
            .unwrap()
            .contains("permission denial did NOT fire"),
        "the stub tool's own body must never have run"
    );
}

#[test]
fn an_approving_permission_manager_allows_the_identical_call() {
    let server = start_server_with_confirmation_handler(
        Box::new(AlwaysApproveForTestingOnly),
        SecurityRules {
            allow_administrator_actions: true,
            ..SecurityRules::default()
        },
    );
    let token = pair_a_device(&server);

    let result: serde_json::Value = ureq::post(&format!("{}/tools/execute", server.base))
        .set("Authorization", &format!("Bearer {token}"))
        .send_json(ureq::json!({"tool": "dangerous-stub", "args": {}}))
        .unwrap()
        .into_json()
        .unwrap();

    assert_eq!(result["success"], true);
    assert_eq!(result["status"], "Ok");
}

#[test]
fn read_only_tools_succeed_regardless_of_the_confirmation_handler() {
    // A ReadOnly tool never reaches the confirmation handler at all
    // (auto-approved below the router's ceiling) -- proving that even
    // an AlwaysDeny handler doesn't block what was never going to ask
    // it in the first place, over the real remote path. Default
    // SecurityRules (Administrator actions blocked) is irrelevant to
    // this specific assertion, so it's left at its default here.
    let server =
        start_server_with_confirmation_handler(Box::new(AlwaysDeny), SecurityRules::default());
    let token = pair_a_device(&server);

    let result: serde_json::Value = ureq::post(&format!("{}/tools/execute", server.base))
        .set("Authorization", &format!("Bearer {token}"))
        .send_json(ureq::json!({"tool": "echo", "args": {"text": "still works"}}))
        .unwrap()
        .into_json()
        .unwrap();

    assert_eq!(result["success"], true);
    assert!(result["message"].as_str().unwrap().contains("still works"));
}

#[test]
fn administrator_actions_are_denied_by_default_over_the_remote_path_too() {
    // The most important case this file proves: PRD SS6.6's
    // "root-level actions must be blocked by default" applies exactly
    // as much to a paired remote device as to a local caller -- even
    // with a confirmation handler that would approve anything,
    // SecurityRules::default()'s allow_administrator_actions: false
    // still blocks it before confirmation is ever reached.
    let server = start_server_with_confirmation_handler(
        Box::new(AlwaysApproveForTestingOnly),
        SecurityRules::default(),
    );
    let token = pair_a_device(&server);

    let result: serde_json::Value = ureq::post(&format!("{}/tools/execute", server.base))
        .set("Authorization", &format!("Bearer {token}"))
        .send_json(ureq::json!({"tool": "dangerous-stub", "args": {}}))
        .unwrap()
        .into_json()
        .unwrap();

    assert_eq!(result["success"], false);
    assert_eq!(result["status"], "PermissionDenied");
}
