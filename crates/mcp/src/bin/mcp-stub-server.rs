//! A tiny, real MCP server binary that serves `tool_framework::stubs`'
//! existing test tools (`EchoTool`, `SlowTool`, `DangerousStubTool`)
//! over stdio. Exists so `tests/client_server_roundtrip.rs` can spawn
//! a genuine, separately-compiled peer process and drive
//! `McpClient` against it over real OS pipes -- a true subprocess
//! boundary, not an in-memory stand-in -- without pulling in any
//! non-Rust dependency (no npm/pip package) just to run `cargo test`.
//! `Cargo` builds this automatically alongside the `mcp` library and
//! exposes it to integration tests as `CARGO_BIN_EXE_mcp-stub-server`.
//!
//! Manual, real-world interop against an actual third-party server
//! (`@modelcontextprotocol/server-filesystem`) was also performed
//! during development -- see `lib.rs` and `protocol.rs` module docs
//! for what that confirmed -- but is not automated here, since doing
//! so would make `cargo test` depend on npm/node being present, which
//! nothing else in this workspace's default test suite requires.

fn main() {
    let mut registry = tool_framework::ToolRegistry::new();
    registry
        .register(std::sync::Arc::new(tool_framework::stubs::EchoTool))
        .expect("EchoTool registers cleanly");
    registry
        .register(std::sync::Arc::new(tool_framework::stubs::SlowTool))
        .expect("SlowTool registers cleanly");
    registry
        .register(std::sync::Arc::new(
            tool_framework::stubs::DangerousStubTool,
        ))
        .expect("DangerousStubTool registers cleanly");

    // Administrator so every stub tool (including DangerousStubTool)
    // is actually reachable through this test server -- this binary
    // exists purely to exercise the wire protocol end to end, not to
    // demonstrate permission gating (that's covered directly in
    // `tests/permission_integration.rs` against `McpToolProxy` without
    // a subprocess in the loop at all).
    let router = tool_framework::ToolRouter::new(registry, common::PermissionLevel::Administrator);

    if let Err(e) = mcp::server::serve_stdio(&router) {
        eprintln!("mcp-stub-server exiting on error: {e}");
        std::process::exit(1);
    }
}
