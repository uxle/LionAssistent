//! End-to-end proof over a genuine subprocess boundary: a real
//! `McpClient` spawns the real, separately-compiled `mcp-stub-server`
//! binary, performs the actual lifecycle handshake, and calls its
//! tools -- real OS pipes, real process, real line-delimited JSON-RPC
//! both directions, not an in-memory stand-in. `server.rs` and
//! `client.rs` each have their own fast in-process unit tests for
//! edge cases; this file is the one place proving the two actually
//! talk to each other for real.

mod support;

use support::connect_to_stub_server;
use tool_framework::ToolRegistry;

#[test]
fn initialize_handshake_reports_the_real_server_identity() {
    let client = connect_to_stub_server();
    let info = client
        .server_info()
        .unwrap()
        .expect("server_info should be set after a successful initialize");
    assert_eq!(info.name, "local-linux-ai-assistant");
    assert_eq!(
        client.negotiated_protocol_version().unwrap().as_deref(),
        Some(mcp::protocol::PROTOCOL_VERSION)
    );
    client.shutdown().unwrap();
}

#[test]
fn list_tools_sees_every_stub_tool_the_real_server_registered() {
    let client = connect_to_stub_server();
    let tools = client.list_tools().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&"echo"));
    assert!(names.contains(&"slow-stub"));
    assert!(names.contains(&"dangerous-stub"));
    client.shutdown().unwrap();
}

#[test]
fn call_tool_against_the_real_server_returns_the_real_echoed_text() {
    let client = connect_to_stub_server();
    let result = client
        .call_tool(
            "echo",
            serde_json::json!({"text": "hello over a real pipe"}),
        )
        .unwrap();
    assert!(!result.is_error, "{result:?}");
    let text = match &result.content[0] {
        mcp::protocol::ContentBlock::Text { text } => text.clone(),
        other => panic!("expected Text content, got {other:?}"),
    };
    assert_eq!(text, "echo: hello over a real pipe");
    client.shutdown().unwrap();
}

#[test]
fn calling_with_a_missing_required_argument_comes_back_is_error_not_a_panic_or_hang() {
    let client = connect_to_stub_server();
    let result = client.call_tool("echo", serde_json::json!({})).unwrap();
    assert!(result.is_error);
    client.shutdown().unwrap();
}

#[test]
fn a_genuinely_slow_tool_is_still_timed_out_by_the_real_servers_own_router() {
    // slow-stub sleeps 500ms behind a 50ms declared timeout
    // (tool-framework/src/stubs.rs). The real server's own ToolRouter
    // enforces this exactly as it would for a local call; the MCP
    // round trip changes nothing about that guarantee.
    let client = connect_to_stub_server();
    let started = std::time::Instant::now();
    let result = client
        .call_tool("slow-stub", serde_json::json!({}))
        .unwrap();
    assert!(result.is_error);
    match &result.content[0] {
        mcp::protocol::ContentBlock::Text { text } => {
            assert!(text.contains("did not complete within"), "{text}")
        }
        other => panic!("expected Text content, got {other:?}"),
    }
    assert!(
        started.elapsed() < std::time::Duration::from_millis(500),
        "should return promptly once the router's own timeout fires, not wait out the tool's full sleep"
    );
    client.shutdown().unwrap();
}

#[test]
fn register_mcp_tools_bridges_every_discovered_tool_into_a_real_registry() {
    let client = connect_to_stub_server();
    let mut registry = ToolRegistry::new();
    let count = mcp::register_mcp_tools(&mut registry, &client).unwrap();
    assert_eq!(count, 3);
    assert_eq!(registry.list(), vec!["dangerous-stub", "echo", "slow-stub"]);
    // Every bridged tool got tagged ToolCategory::Mcp and
    // PermissionLevel::Moderate regardless of what it was natively --
    // dangerous-stub is Administrator locally, but proxied it is
    // Moderate like every other MCP-sourced tool (bridge.rs docs).
    let proxied = registry.get("dangerous-stub").unwrap();
    assert_eq!(
        proxied.permission_level(),
        common::PermissionLevel::Moderate
    );
    assert_eq!(proxied.category(), tool_framework::ToolCategory::Mcp);
    client.shutdown().unwrap();
}

#[test]
fn a_bridged_tool_executes_through_the_full_round_trip_from_a_plain_tool_call() {
    let client = connect_to_stub_server();
    let mut registry = ToolRegistry::new();
    mcp::register_mcp_tools(&mut registry, &client).unwrap();

    let tool = registry.get("echo").expect("echo should be bridged");
    let mut args = tool_framework::ToolArgs::new();
    args.insert("text".into(), serde_json::json!("bridged call"));
    let outcome = tool.execute(&args);

    assert!(outcome.success, "{}", outcome.message);
    assert_eq!(outcome.message, "echo: bridged call");
    assert_eq!(
        outcome.data.unwrap()["text"],
        serde_json::json!("bridged call")
    );
    client.shutdown().unwrap();
}
