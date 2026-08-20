//! `mcp` -- a Model Context Protocol client, server, and external-tool
//! bridge (Development_plan Phase 27 "Model Context Protocol (MCP)";
//! Apis SS19 and Archd SS20 both name MCP explicitly as a future
//! extensibility point).
//!
//! # Why 2025-11-25, not 2026-07-28
//!
//! Two spec revisions exist as of this phase's implementation (August
//! 2026). 2026-07-28 -- the actual current/latest revision, finalized
//! three weeks before this phase was written -- replaces the
//! stateful `initialize`/`initialized` handshake and session-ID model
//! entirely with self-contained, stateless requests (every request
//! carries its own protocol version and capabilities in `_meta`
//! rather than negotiating them once). It is real and it is where the
//! protocol is headed. It is also, by the spec's own release notes,
//! barely adopted yet: SDK support across the official languages was
//! still in beta at release, and new clients speaking 2026-07-28 are
//! explicitly designed to fall back to the 2025-11-25 handshake for
//! any server that doesn't speak the new version -- which, per those
//! same release notes, is expected to be most of the deployed
//! ecosystem for some time.
//!
//! This was not left as an assumption: `@modelcontextprotocol/server-filesystem`
//! version `2026.7.10` -- a real, currently-published reference
//! server, installed from the real npm registry during development,
//! three weeks *after* 2026-07-28 finalized -- was spawned and probed
//! directly. Requesting `protocolVersion: "2025-11-25"` in a genuine
//! `initialize` call got back exactly `"2025-11-25"`, with a real
//! `tools/list` and `tools/call` round trip against a real file
//! confirming the wire shapes this crate implements. That is the
//! version the actually-installable server ecosystem speaks today,
//! confirmed empirically rather than assumed from documentation, so
//! that is what this crate targets. `protocol.rs`'s module docs record
//! the exact shapes observed, including two details that were *not*
//! obvious from the schema alone: a failed tool call arrives as a
//! normal JSON-RPC result with `isError: true`, not a protocol-level
//! error, and only a genuinely unrecognized method produces one.
//!
//! # What this phase implements
//!
//! - [`client::McpClient`]: spawns an MCP server as a subprocess,
//!   performs the real lifecycle handshake, lists and calls tools.
//! - [`server`]: exposes this assistant's own `ToolRouter` as an MCP
//!   server over stdio, tool calls routed through the exact same
//!   permission-checked pipeline as any other caller.
//! - [`bridge`]: turns tools discovered via a connected `McpClient`
//!   into real `tool_framework::Tool` registrations ("External Tool
//!   Integration"), and this workspace's own tools into the JSON
//!   Schema `server` advertises.
//!
//! # What this phase deliberately does not implement
//!
//! - **Resources, Prompts, Sampling, Roots, Elicitation.** Tools are
//!   the primitive this assistant's own architecture already has a
//!   concept of (`tool_framework::Tool`); the others have no existing
//!   analog here to bridge into, and inventing one speculatively
//!   risks the same mistake Phase 7 avoided by not wiring itself into
//!   `assistant-core` before a real caller existed.
//! - **Tasks.** Marked experimental in 2025-11-25 itself, and
//!   substantially reworked into an optional extension for
//!   2026-07-28 -- building on it now would mean building on the
//!   least stable, shortest-lived part of either spec revision.
//! - **Streamable HTTP transport.** stdio only. A local-first desktop
//!   assistant spawning a local server subprocess is the primary real
//!   use case (PRD SS3 rules out cloud dependency for the core
//!   assistant), and every reference server tested against during
//!   development runs over stdio. `transport::LineTransport` is
//!   generic over any `BufRead`/`Write` pair, so adding an HTTP
//!   transport later does not require changing the framing layer.
//! - **A model-callable "connect to a new MCP server" tool.** See
//!   `bridge`'s module docs for the specific structural reason
//!   (`Tool::execute` has no path back to the live `ToolRegistry` it's
//!   dispatched from) and what this phase provides instead.
//! - **Concurrent/pipelined requests.** Both `client` and `server`
//!   handle one request at a time; see `client`'s module docs for why
//!   that matches how local MCP servers are actually used in practice.

pub mod bridge;
pub mod client;
pub mod protocol;
pub mod server;
pub mod transport;

pub use bridge::{register_mcp_tools, McpToolProxy};
pub use client::McpClient;
