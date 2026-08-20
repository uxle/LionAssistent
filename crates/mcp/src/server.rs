//! Exposes this assistant's own `tool_framework::ToolRouter` as a
//! Model Context Protocol server (MCP "Server Features: Tools"), so an
//! external MCP client (Claude Desktop, another instance of this
//! assistant, or anything else that speaks 2025-11-25) can list and
//! call this assistant's real, permission-gated tools.
//!
//! `tools/call` is routed through `ToolRouter::execute` -- the exact
//! same validate/permission-check/timeout pipeline a locally-issued
//! tool call goes through (TCLIS SS1: "the LLM never performs
//! operating system actions directly, only through this kind of
//! validated, audited pipeline"). An external MCP client asking this
//! assistant to run a tool is not a special case: it is just another
//! caller of that same pipeline, permission-checked exactly the same
//! way. There is no separate, less-guarded path here.
//!
//! Deliberately serves one request at a time, synchronously reading
//! the next line only after fully responding to the last -- see
//! `client.rs`'s module docs for why that scope limit is reasonable
//! for how local MCP servers are actually used, and `lib.rs` for the
//! full list of what Phase 27 does not implement (Tasks, resources,
//! prompts, sampling, roots).

use crate::bridge::to_json_schema;
use crate::protocol::*;
use crate::transport::LineTransport;
use common::{AppError, AppResult};
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use tool_framework::{ToolArgs, ToolRouter};

const SERVER_NAME: &str = "local-linux-ai-assistant";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

fn id_to_json(id: &RequestId) -> Value {
    match id {
        RequestId::Number(n) => json!(n),
        RequestId::String(s) => json!(s),
    }
}

fn result_response(id: &RequestId, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id_to_json(id), "result": result})
}

fn error_response(id: &RequestId, code: i64, message: impl Into<String>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id_to_json(id),
        "error": {"code": code, "message": message.into()},
    })
}

/// Runs the request/response loop over `transport` until the peer
/// closes its output (clean EOF, returned as `Ok(())`) or a transport
/// error occurs. Blocks the calling thread -- see the module docs on
/// why that is a deliberate, documented choice here, not an oversight.
pub fn serve<R: BufRead, W: Write>(
    transport: &mut LineTransport<R, W>,
    router: &ToolRouter,
    handle: &tokio::runtime::Handle,
) -> AppResult<()> {
    let mut initialized = false;
    loop {
        let Some(value) = transport.recv()? else {
            return Ok(());
        };
        let raw_id = value.get("id").cloned();

        match classify(value) {
            Ok(IncomingMessage::Request { id, method, params }) => {
                let response =
                    handle_request(&id, &method, params, router, handle, &mut initialized);
                transport.send(&response)?;
            }
            Ok(IncomingMessage::Notification { .. }) => {
                // `notifications/initialized` and any notification this
                // server doesn't specifically act on: JSON-RPC 2.0
                // notifications never get a response either way, so
                // there's nothing further to do for any of them.
            }
            Ok(IncomingMessage::Result { .. }) | Ok(IncomingMessage::Error { .. }) => {
                // A response to a request we never sent (this crate's
                // server implements no server-initiated requests this
                // phase). Nothing to correlate it with; drop it.
            }
            Err(_) => {
                // Valid JSON, but not a shape `classify` recognizes.
                // Reply only if there was an id to reply *to* --
                // otherwise there's no sensible request to attach an
                // error response to.
                if let Some(id_val) = raw_id {
                    if let Ok(id) = serde_json::from_value::<RequestId>(id_val) {
                        transport.send(&error_response(
                            &id,
                            INVALID_REQUEST,
                            "message did not match any recognized JSON-RPC shape",
                        ))?;
                    }
                }
            }
        }
    }
}

fn handle_request(
    id: &RequestId,
    method: &str,
    params: Option<Value>,
    router: &ToolRouter,
    handle: &tokio::runtime::Handle,
    initialized: &mut bool,
) -> Value {
    match method {
        "initialize" => {
            *initialized = true;
            result_response(
                id,
                json!({
                    "protocolVersion": PROTOCOL_VERSION,
                    "capabilities": {"tools": {"listChanged": false}},
                    "serverInfo": {"name": SERVER_NAME, "version": SERVER_VERSION},
                }),
            )
        }
        "ping" => result_response(id, json!({})),
        _ if !*initialized => error_response(
            id,
            INVALID_REQUEST,
            format!("{method} called before initialize completed"),
        ),
        "tools/list" => {
            let registry = router.registry();
            let tools: Vec<Value> = registry
                .list()
                .into_iter()
                .filter_map(|name| registry.get(name))
                .map(|tool| {
                    json!({
                        "name": tool.name(),
                        "description": tool.description(),
                        "inputSchema": to_json_schema(&tool.input_schema()),
                    })
                })
                .collect();
            result_response(id, json!({"tools": tools}))
        }
        "tools/call" => handle_tools_call(id, params, router, handle),
        _ => error_response(id, METHOD_NOT_FOUND, format!("unknown method {method:?}")),
    }
}

fn handle_tools_call(
    id: &RequestId,
    params: Option<Value>,
    router: &ToolRouter,
    handle: &tokio::runtime::Handle,
) -> Value {
    let Some(params) = params else {
        return error_response(id, INVALID_PARAMS, "tools/call requires params");
    };
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        return error_response(
            id,
            INVALID_PARAMS,
            "tools/call params missing string \"name\"",
        );
    };
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let Some(args_obj) = arguments.as_object() else {
        return error_response(id, INVALID_PARAMS, "\"arguments\" must be a JSON object");
    };
    let args: ToolArgs = args_obj
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    // The one genuinely async call in this whole loop. Safe to
    // `block_on` here specifically because `serve`'s caller runs it
    // from a plain OS thread, never from inside an already-executing
    // tokio task -- see `serve_stdio` and the module docs.
    let result = handle.block_on(router.execute(name, args));

    // Built as a `Map` rather than via `json!` so `structuredContent`
    // can be omitted entirely when there's no data, instead of
    // serializing as an explicit `null` -- matches the real shape
    // observed from a genuine reference server for calls with no
    // structured output (see `protocol.rs` module docs).
    let mut result_obj = serde_json::Map::new();
    result_obj.insert(
        "content".into(),
        json!([{"type": "text", "text": result.message}]),
    );
    result_obj.insert("isError".into(), json!(!result.success));
    if let Some(data) = result.data {
        result_obj.insert("structuredContent".into(), data);
    }
    result_response(id, Value::Object(result_obj))
}

/// Convenience entry point for a real binary: serves over this
/// process's actual stdin/stdout, on a fresh single-threaded Tokio
/// runtime built just for this. `StdinLock`/`StdoutLock` are held for
/// the whole call, so nothing else in the process should be reading
/// stdin or writing stdout concurrently -- true of any stdio MCP
/// server, and exactly how the real reference server this crate was
/// tested against behaves too (it logs to stderr for that reason).
pub fn serve_stdio(router: &ToolRouter) -> AppResult<()> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut transport = LineTransport::new(stdin.lock(), stdout.lock());

    let rt = tokio::runtime::Runtime::new().map_err(|e| AppError::Module {
        module: "mcp",
        message: format!("failed to start a tokio runtime for the mcp server: {e}"),
    })?;
    serve(&mut transport, router, rt.handle())
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::PermissionLevel;
    use std::io::Cursor;
    use std::sync::Arc;
    use tool_framework::{ToolRegistry, ToolRouter};

    fn router_with_echo() -> ToolRouter {
        let mut registry = ToolRegistry::new();
        registry
            .register(Arc::new(tool_framework::stubs::EchoTool))
            .unwrap();
        ToolRouter::new(registry, PermissionLevel::ReadOnly)
    }

    /// Drives `serve` over an in-memory transport built from a fixed
    /// input script, and returns everything it wrote. Runs `serve` on
    /// its own thread with its own runtime -- `serve` blocks the
    /// calling thread by design (see module docs), so a `#[test]`
    /// (not `#[tokio::test]`) drives it directly, exactly as the real
    /// `serve_stdio` binary entry point does.
    fn run_serve_to_completion(input: &str, router: &ToolRouter) -> Vec<Value> {
        let mut transport = LineTransport::new(Cursor::new(input.as_bytes().to_vec()), Vec::new());
        let rt = tokio::runtime::Runtime::new().unwrap();
        serve(&mut transport, router, rt.handle()).expect("serve should end cleanly at EOF");
        String::from_utf8(transport.into_writer())
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    #[test]
    fn initialize_then_tools_list_then_tools_call_end_to_end() {
        let router = router_with_echo();
        let input = concat!(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}"#,
            "\n",
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"echo","arguments":{"text":"hi"}}}"#,
            "\n",
        );
        let responses = run_serve_to_completion(input, &router);

        assert_eq!(
            responses.len(),
            3,
            "3 requests in, 3 responses out (the notification gets none)"
        );
        assert_eq!(
            responses[0]["result"]["protocolVersion"],
            json!("2025-11-25")
        );
        assert_eq!(responses[1]["result"]["tools"][0]["name"], json!("echo"));
        assert_eq!(
            responses[2]["result"]["content"][0]["text"],
            json!("echo: hi")
        );
        assert_eq!(responses[2]["result"]["isError"], json!(false));
    }

    #[test]
    fn tools_call_before_initialize_is_rejected() {
        let router = router_with_echo();
        let input = "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}\n";
        let responses = run_serve_to_completion(input, &router);
        assert_eq!(responses[0]["error"]["code"], json!(INVALID_REQUEST));
    }

    #[test]
    fn unknown_method_returns_method_not_found() {
        let router = router_with_echo();
        let input = concat!(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":2,"method":"nonexistent/method"}"#,
            "\n",
        );
        let responses = run_serve_to_completion(input, &router);
        assert_eq!(responses[1]["error"]["code"], json!(METHOD_NOT_FOUND));
    }

    #[test]
    fn a_failing_tool_call_is_a_result_with_is_error_true_not_a_protocol_error() {
        // Matches the real reference server's behavior for a bad tool
        // call, confirmed by manual probing (protocol.rs module docs):
        // tool-level problems are results, not JSON-RPC errors.
        let router = router_with_echo();
        let input = concat!(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"echo","arguments":{}}}"#,
            "\n",
        );
        let responses = run_serve_to_completion(input, &router);
        assert!(responses[1].get("error").is_none());
        assert_eq!(responses[1]["result"]["isError"], json!(true));
    }

    #[test]
    fn calling_an_unregistered_tool_is_also_is_error_true() {
        let router = router_with_echo();
        let input = concat!(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"nonexistent","arguments":{}}}"#,
            "\n",
        );
        let responses = run_serve_to_completion(input, &router);
        assert_eq!(responses[1]["result"]["isError"], json!(true));
    }

    #[test]
    fn structured_content_is_omitted_entirely_when_the_tool_returns_no_data() {
        let mut registry = ToolRegistry::new();
        registry
            .register(Arc::new(tool_framework::stubs::SlowTool))
            .unwrap(); // SlowTool times out and returns data: None either way
        let router = ToolRouter::new(registry, PermissionLevel::ReadOnly);
        let input = concat!(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"slow-stub","arguments":{}}}"#,
            "\n",
        );
        let responses = run_serve_to_completion(input, &router);
        assert!(responses[1]["result"].get("structuredContent").is_none());
    }

    #[test]
    fn ping_works_before_initialize() {
        let router = router_with_echo();
        let input = "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n";
        let responses = run_serve_to_completion(input, &router);
        assert_eq!(responses[0]["result"], json!({}));
    }

    #[test]
    fn clean_eof_with_no_input_returns_ok() {
        let router = router_with_echo();
        let responses = run_serve_to_completion("", &router);
        assert!(responses.is_empty());
    }
}
