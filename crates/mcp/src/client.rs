//! A blocking Model Context Protocol client: spawns a peer as a
//! subprocess, performs the `initialize`/`notifications/initialized`
//! lifecycle, and offers `list_tools`/`call_tool` (MCP "Lifecycle",
//! "Server Features: Tools").
//!
//! Synchronous by design, matching `tool_framework::Tool::execute`'s
//! own signature exactly (no `async fn`) -- the same reasoning
//! `network::HttpClient` already used in this workspace (Phase 14): a
//! tool's `execute()` has no way to `.await` anything, so whatever it
//! calls into must itself block. Every public method takes `&self`
//! (not `&mut self`), with the mutable conversation state behind one
//! `Mutex`, so an `Arc<McpClient>` can be shared into a
//! `tool_framework::Tool` impl (`bridge::McpToolProxy`) the same way
//! `rag`'s tools share an `Arc<KnowledgeBase>`. That single mutex also
//! structurally enforces this crate's one deliberate scope limit: at
//! most one request in flight at a time per client, matching how
//! every real MCP server this crate has been tested against is
//! actually used (a single local session, not concurrent pipelined
//! calls).

use crate::protocol::*;
use crate::transport::LineTransport;
use common::{AppError, AppResult};
use serde_json::{json, Value};
use std::io::{BufReader, BufWriter};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const CLIENT_NAME: &str = "local-linux-ai-assistant";
const CLIENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Recent stderr lines kept for diagnostics if the peer misbehaves or
/// exits unexpectedly. Bounded so a chatty peer cannot grow this
/// process's memory without bound.
const STDERR_TAIL_LINES: usize = 50;

/// How long [`McpClient::shutdown`] waits for the child to exit on its
/// own (after closing its stdin) before killing it.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(2);

type StdioTransport = LineTransport<BufReader<ChildStdout>, BufWriter<ChildStdin>>;

struct Connection {
    transport: StdioTransport,
    next_id: i64,
    server_info: Option<Implementation>,
    server_capabilities: Option<ServerCapabilities>,
    negotiated_protocol_version: Option<String>,
}

/// A connection to one MCP server, reached by spawning it as a child
/// process and speaking the stdio transport.
pub struct McpClient {
    child: Mutex<Child>,
    /// `None` after [`Self::shutdown`] has run; every method that
    /// needs the connection goes through [`Self::with_conn`], which
    /// turns that into a clear error instead of a panic.
    conn: Mutex<Option<Connection>>,
    stderr_tail: Arc<Mutex<Vec<String>>>,
}

impl McpClient {
    /// Spawns `command` (with `args`) as a child process and connects
    /// to it over its stdio pipes. Does not perform the `initialize`
    /// handshake yet -- call [`Self::initialize`] next, mirroring the
    /// real two-step lifecycle (transport connection, then
    /// initialize).
    pub fn spawn(command: &str, args: &[String]) -> AppResult<Self> {
        let mut child = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stdin = child.stdin.take().ok_or_else(|| AppError::Module {
            module: "mcp",
            message: "spawned child had no stdin pipe".into(),
        })?;
        let stdout = child.stdout.take().ok_or_else(|| AppError::Module {
            module: "mcp",
            message: "spawned child had no stdout pipe".into(),
        })?;
        let stderr = child.stderr.take().ok_or_else(|| AppError::Module {
            module: "mcp",
            message: "spawned child had no stderr pipe".into(),
        })?;

        let stderr_tail = Arc::new(Mutex::new(Vec::new()));
        spawn_stderr_drain(stderr, Arc::clone(&stderr_tail));

        let transport = LineTransport::new(BufReader::new(stdout), BufWriter::new(stdin));

        Ok(Self {
            child: Mutex::new(child),
            conn: Mutex::new(Some(Connection {
                transport,
                next_id: 1,
                server_info: None,
                server_capabilities: None,
                negotiated_protocol_version: None,
            })),
            stderr_tail,
        })
    }

    fn with_conn<T>(&self, f: impl FnOnce(&mut Connection) -> AppResult<T>) -> AppResult<T> {
        let mut guard = self.conn.lock().map_err(|_| AppError::Module {
            module: "mcp",
            message: "mcp client state poisoned by an earlier panic".into(),
        })?;
        match guard.as_mut() {
            Some(conn) => f(conn),
            None => Err(AppError::Module {
                module: "mcp",
                message: "mcp client has already been shut down".into(),
            }),
        }
    }

    /// Sends a request and blocks for its matching response.
    ///
    /// Tolerates (and discards) any notification or peer-initiated
    /// request that arrives before the matching response -- both are
    /// legal at any time per the spec. A peer-initiated *request*
    /// gets a polite "not supported" reply so it isn't left hanging;
    /// Phase 27 implements no client-side features (sampling, roots)
    /// that a server could be asking for.
    fn request(&self, method: &str, params: Option<Value>) -> AppResult<Value> {
        self.with_conn(|conn| {
            let id = RequestId::Number(conn.next_id);
            conn.next_id += 1;
            let req = JsonRpcRequest::new(id.clone(), method, params);
            let req_value = req
                .to_value()
                .map_err(|e| AppError::Serialization(e.to_string()))?;
            conn.transport.send(&req_value)?;

            loop {
                let Some(value) = conn.transport.recv()? else {
                    return Err(AppError::Module {
                        module: "mcp",
                        message: format!(
                            "peer closed its output before responding to {method} \
                             (recent stderr: {:?})",
                            self.recent_stderr()
                        ),
                    });
                };
                let parsed = classify(value).map_err(|e| {
                    AppError::Serialization(format!("malformed message from peer: {e}"))
                })?;
                match parsed {
                    IncomingMessage::Result { id: rid, result } if rid == id => return Ok(result),
                    IncomingMessage::Error { id: rid, error } if rid == id => {
                        return Err(AppError::Module {
                            module: "mcp",
                            message: format!(
                                "{method} failed: {} (code {})",
                                error.message, error.code
                            ),
                        })
                    }
                    // A response to a different id than the one we're
                    // waiting for cannot happen given the Mutex above
                    // serializes every call to at most one in flight
                    // -- but a misbehaving peer could still send one
                    // unprompted, so skip it rather than misreport it
                    // as our answer.
                    IncomingMessage::Result { .. } | IncomingMessage::Error { .. } => continue,
                    IncomingMessage::Notification { .. } => continue,
                    IncomingMessage::Request { id: rid, .. } => {
                        let reply = json!({
                            "jsonrpc": "2.0",
                            "id": match rid {
                                RequestId::Number(n) => json!(n),
                                RequestId::String(s) => json!(s),
                            },
                            "error": {
                                "code": METHOD_NOT_FOUND,
                                "message": "client implements no server-initiated methods",
                            }
                        });
                        conn.transport.send(&reply)?;
                        continue;
                    }
                }
            }
        })
    }

    fn send_notification(&self, method: &str, params: Option<Value>) -> AppResult<()> {
        let note = JsonRpcNotification::new(method, params);
        let value = note
            .to_value()
            .map_err(|e| AppError::Serialization(e.to_string()))?;
        self.with_conn(|conn| conn.transport.send(&value))
    }

    /// Performs the `initialize` request followed by the
    /// `notifications/initialized` notification (MCP "Lifecycle").
    /// Must be called exactly once, before [`Self::list_tools`] or
    /// [`Self::call_tool`].
    pub fn initialize(&self) -> AppResult<InitializeResult> {
        let params = json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": ClientCapabilities::default(),
            "clientInfo": {"name": CLIENT_NAME, "version": CLIENT_VERSION},
        });
        let result = self.request("initialize", Some(params))?;
        let init: InitializeResult = serde_json::from_value(result)
            .map_err(|e| AppError::Serialization(format!("malformed initialize result: {e}")))?;

        self.send_notification("notifications/initialized", None)?;

        self.with_conn(|conn| {
            conn.server_info = Some(init.server_info.clone());
            conn.server_capabilities = Some(init.capabilities.clone());
            conn.negotiated_protocol_version = Some(init.protocol_version.clone());
            Ok(())
        })?;

        Ok(init)
    }

    /// Lists every tool the server offers, following `nextCursor`
    /// pagination to completion rather than returning only the first
    /// page (MCP "Pagination").
    pub fn list_tools(&self) -> AppResult<Vec<Tool>> {
        let mut all = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let params = cursor.as_ref().map(|c| json!({"cursor": c}));
            let result = self.request("tools/list", params)?;
            let page: ListToolsResult = serde_json::from_value(result).map_err(|e| {
                AppError::Serialization(format!("malformed tools/list result: {e}"))
            })?;
            all.extend(page.tools);
            match page.next_cursor {
                Some(next) => cursor = Some(next),
                None => break,
            }
        }
        Ok(all)
    }

    /// Calls one tool by name. Note that a *tool-level* failure (bad
    /// arguments, the tool's own logic erroring) is not an `Err` here
    /// -- it comes back as `Ok(CallToolResult { is_error: true, .. })`,
    /// confirmed against a real server (see `protocol` module docs).
    /// Only a protocol-level problem (transport failure, malformed
    /// JSON-RPC, an `error` object) becomes an `Err`. Callers that
    /// need a single success/failure signal should check
    /// `result.is_error` themselves -- see `bridge::McpToolProxy` for
    /// the mapping this crate uses when re-exposing a call as an
    /// ordinary `tool_framework::ToolOutcome`.
    pub fn call_tool(&self, name: &str, arguments: Value) -> AppResult<CallToolResult> {
        let params = json!({"name": name, "arguments": arguments});
        let result = self.request("tools/call", Some(params))?;
        serde_json::from_value(result)
            .map_err(|e| AppError::Serialization(format!("malformed tools/call result: {e}")))
    }

    pub fn server_info(&self) -> AppResult<Option<Implementation>> {
        self.with_conn(|conn| Ok(conn.server_info.clone()))
    }

    pub fn negotiated_protocol_version(&self) -> AppResult<Option<String>> {
        self.with_conn(|conn| Ok(conn.negotiated_protocol_version.clone()))
    }

    /// Recent lines the peer wrote to its own stderr, most useful when
    /// something has gone wrong (a call failed, the peer exited
    /// unexpectedly). Never part of the protocol itself -- purely a
    /// debugging aid, since real servers do log real diagnostics there
    /// (confirmed during manual probing).
    pub fn recent_stderr(&self) -> Vec<String> {
        self.stderr_tail
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    /// Closes the connection: drops the transport (which flushes and
    /// closes both pipe ends, sending stdin EOF -- the signal most
    /// real MCP servers treat as "the client is done" and exit on,
    /// confirmed during manual probing), waits briefly for the child
    /// to exit on its own, and kills it if it doesn't. Idempotent:
    /// calling this twice is not an error.
    pub fn shutdown(&self) -> AppResult<()> {
        {
            let mut guard = self.conn.lock().map_err(|_| AppError::Module {
                module: "mcp",
                message: "mcp client state poisoned by an earlier panic".into(),
            })?;
            *guard = None; // drops the old Connection (and its pipes) here
        }

        let mut child = self.child.lock().map_err(|_| AppError::Module {
            module: "mcp",
            message: "mcp client state poisoned by an earlier panic".into(),
        })?;

        let deadline = Instant::now() + SHUTDOWN_GRACE;
        loop {
            if child.try_wait()?.is_some() {
                return Ok(());
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for McpClient {
    /// Best-effort safety net for a client dropped without an
    /// explicit `shutdown()` call: never leave an orphaned child
    /// process behind. Not graceful (no stdin-EOF grace period) --
    /// `Drop` cannot return an error or block indefinitely, so this
    /// simply kills and reaps.
    fn drop(&mut self) {
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn spawn_stderr_drain(stderr: std::process::ChildStderr, tail: Arc<Mutex<Vec<String>>>) {
    std::thread::spawn(move || {
        use std::io::BufRead;
        let reader = std::io::BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            let Ok(mut guard) = tail.lock() else { return };
            guard.push(line);
            let excess = guard.len().saturating_sub(STDERR_TAIL_LINES);
            if excess > 0 {
                guard.drain(0..excess);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// No real MCP server needed for this one: `/bin/cat` echoes
    /// stdin to stdout, which is exactly a (protocol-incorrect but
    /// transport-correct) peer for exercising spawn/pipe wiring and
    /// clean shutdown without depending on anything MCP-specific.
    #[test]
    fn spawn_and_shutdown_a_real_child_process_cleanly() {
        let client = McpClient::spawn("cat", &[]).expect("cat should spawn");
        client.shutdown().expect("shutdown should succeed");
        // Idempotent.
        client
            .shutdown()
            .expect("second shutdown should also succeed");
    }

    #[test]
    fn spawning_a_nonexistent_command_is_a_clean_error_not_a_panic() {
        let result = McpClient::spawn("this-command-does-not-exist-anywhere", &[]);
        assert!(result.is_err());
    }

    #[test]
    fn methods_after_shutdown_return_a_clear_error() {
        let client = McpClient::spawn("cat", &[]).unwrap();
        client.shutdown().unwrap();
        let err = client.list_tools().unwrap_err();
        assert!(err.to_string().contains("already been shut down"));
    }

    #[test]
    fn drop_without_explicit_shutdown_does_not_leak_the_child() {
        let client = McpClient::spawn("cat", &[]).unwrap();
        let pid = {
            let child = client.child.lock().unwrap();
            child.id()
        };
        drop(client);
        // Give the OS a moment to actually reap it, then confirm the
        // pid is no longer a live process (kill -0 fails with ESRCH).
        std::thread::sleep(Duration::from_millis(200));
        let alive = std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        assert!(
            !alive,
            "child pid {pid} should not still be running after Drop"
        );
    }
}
