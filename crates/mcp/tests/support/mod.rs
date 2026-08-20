//! Shared helper for `mcp`'s integration tests: connects a real
//! `McpClient` to the real, separately-compiled `mcp-stub-server`
//! binary (`src/bin/mcp-stub-server.rs`) over a genuine subprocess
//! boundary. `CARGO_BIN_EXE_mcp-stub-server` is set automatically by
//! Cargo for integration tests in this same package -- no PATH lookup,
//! no assumption about where the binary landed.
//!
//! Named `support`, not the more common `common` -- this workspace has
//! a real crate literally named `common`, and a same-named local test
//! module shadows it for every `common::` path inside the test binary
//! that declares `mod common;` (confirmed the hard way: the first
//! version of this file was named `common`, and every
//! `common::PermissionLevel` reference in the tests that used it
//! promptly failed to resolve).

use mcp::McpClient;
use std::sync::Arc;

/// Spawns the real stub server and performs the real `initialize`
/// handshake. Panics on failure -- acceptable in a test helper, since
/// a failure here means the test itself cannot proceed meaningfully.
pub fn connect_to_stub_server() -> Arc<McpClient> {
    let client = McpClient::spawn(env!("CARGO_BIN_EXE_mcp-stub-server"), &[])
        .expect("the real mcp-stub-server binary should spawn");
    client
        .initialize()
        .expect("initialize handshake should succeed against our own real server");
    Arc::new(client)
}
