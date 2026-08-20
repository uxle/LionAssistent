//! Optional, opt-in real-world interop check against
//! `@modelcontextprotocol/server-filesystem`, the actual third-party
//! reference server (not anything this workspace built) used during
//! development to ground `protocol.rs` in real wire behavior -- see
//! `lib.rs`'s crate docs for the full account of what was manually
//! confirmed. This file re-runs a version of that same probe as an
//! executable test, `#[ignore]`d so it never affects the default
//! `cargo test` sweep (this workspace's other 39 crates run on
//! nothing but the pinned Rust toolchain and the system packages the
//! README documents; making this one crate's default suite depend on
//! npm/node would be a real, unwanted expansion of that).
//!
//! Run explicitly, with Node.js available, via:
//! `cargo test -p mcp --test real_reference_server_interop -- --ignored --nocapture`
//!
//! `npx` is used specifically so this doesn't depend on the ad hoc
//! `npm install` performed during development (outside the repo,
//! under `/home/claude/work/mcp-probe/`, never committed) -- it fetches
//! the exact pinned version fresh, so this test's own claim ("this
//! version, run this way, behaves like this") stays independently
//! checkable rather than resting on an environment nobody else has.

use std::process::Command;

const REFERENCE_SERVER_PACKAGE: &str = "@modelcontextprotocol/server-filesystem@2026.7.10";

fn npx_available() -> bool {
    Command::new("npx")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[test]
#[ignore = "requires Node.js/npx and network access to the real npm registry; see module docs"]
fn real_reference_server_confirms_2025_11_25_and_a_real_file_read() {
    if !npx_available() {
        eprintln!("skipping: npx not found on PATH -- this test needs Node.js installed");
        return;
    }

    let dir = std::env::temp_dir().join("mcp-real-server-interop");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("greeting.txt"),
        "hello from a real reference server test",
    )
    .unwrap();

    let client = mcp::McpClient::spawn(
        "npx",
        &[
            "--yes".to_string(),
            REFERENCE_SERVER_PACKAGE.to_string(),
            dir.to_str().unwrap().to_string(),
        ],
    )
    .expect("npx should be able to spawn the reference server");

    let init = client
        .initialize()
        .expect("a real, currently-published reference server should complete the handshake");
    assert_eq!(
        init.protocol_version,
        mcp::protocol::PROTOCOL_VERSION,
        "the real reference server should still speak 2025-11-25 when asked for it, \
         exactly as observed during development -- if this now fails, the ecosystem \
         has likely moved to 2026-07-28 and lib.rs's version-choice reasoning needs revisiting"
    );

    let tools = client.list_tools().unwrap();
    assert!(
        tools.iter().any(|t| t.name == "read_text_file"),
        "expected the real server's well-known read_text_file tool"
    );

    let result = client
        .call_tool(
            "read_text_file",
            serde_json::json!({"path": dir.join("greeting.txt").to_str().unwrap()}),
        )
        .unwrap();
    assert!(!result.is_error, "{result:?}");
    let text = match &result.content[0] {
        mcp::protocol::ContentBlock::Text { text } => text.clone(),
        other => panic!("expected Text content, got {other:?}"),
    };
    assert!(text.contains("hello from a real reference server test"));

    // The specific, non-obvious detail this crate's error handling
    // depends on: annotations exist on real tools (readOnlyHint,
    // openWorldHint, ...) but this crate's bridge intentionally never
    // reads them for a permission decision (bridge.rs docs). Confirm
    // they really are present on a real server's real response, so
    // that "we choose not to trust them" is a real choice about real
    // data, not a moot point about a field that never shows up.
    let read_tool = tools.iter().find(|t| t.name == "read_text_file").unwrap();
    assert!(
        read_tool.annotations.is_some(),
        "expected the real server to actually send annotations, confirming bridge.rs's \
         'never trust them anyway' stance is a real, exercised choice"
    );

    client.shutdown().unwrap();
    std::fs::remove_dir_all(&dir).ok();
}
