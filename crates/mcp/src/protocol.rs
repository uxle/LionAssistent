//! JSON-RPC 2.0 envelope and Model Context Protocol message types.
//!
//! Field names and shapes are taken from the real schema
//! (`github.com/modelcontextprotocol/modelcontextprotocol`,
//! `schema/2025-11-25/schema.json`, fetched directly rather than
//! recalled from training data) and cross-checked against a real,
//! currently-published reference server
//! (`@modelcontextprotocol/server-filesystem` 2026.7.10 on npm) during
//! development -- see the crate-level docs in `lib.rs` and the
//! Phase 27 CHANGELOG entry for what was actually observed on the
//! wire, including the non-obvious bits (tool-level failures arrive
//! as a normal JSON-RPC *result* with `isError: true`, not as a
//! protocol-level `error`).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The single spec revision this crate speaks. See `lib.rs` for why
/// 2025-11-25 rather than the newer 2026-07-28 stateless revision.
pub const PROTOCOL_VERSION: &str = "2025-11-25";

// Standard JSON-RPC 2.0 error codes (base spec, not MCP-specific).
// `METHOD_NOT_FOUND` is the one this crate's own server actually
// constructs; the others are recognized on incoming error objects.
pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
pub const INTERNAL_ERROR: i64 = -32603;

/// A JSON-RPC request ID. The spec permits string or integer; this
/// crate only ever mints integer IDs itself (`client::McpClient`), but
/// must still be able to parse a string ID, since nothing requires a
/// peer to use the same convention.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestId {
    Number(i64),
    String(String),
}

impl From<i64> for RequestId {
    fn from(n: i64) -> Self {
        RequestId::Number(n)
    }
}

impl std::fmt::Display for RequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RequestId::Number(n) => write!(f, "{n}"),
            RequestId::String(s) => write!(f, "{s}"),
        }
    }
}

/// An outgoing JSON-RPC request: expects exactly one response, either
/// a [`IncomingMessage::Result`] or [`IncomingMessage::Error`].
#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: &'static str,
    pub id: RequestId,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl JsonRpcRequest {
    pub fn new(id: RequestId, method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            method: method.into(),
            params,
        }
    }

    pub fn to_value(&self) -> AppResultValue {
        serde_json::to_value(self)
    }
}

type AppResultValue = Result<Value, serde_json::Error>;

/// An outgoing JSON-RPC notification: fire-and-forget, no response.
#[derive(Debug, Clone, Serialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: &'static str,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl JsonRpcNotification {
    pub fn new(method: impl Into<String>, params: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0",
            method: method.into(),
            params,
        }
    }

    pub fn to_value(&self) -> AppResultValue {
        serde_json::to_value(self)
    }
}

/// A JSON-RPC error object: `{"code": ..., "message": ..., "data"?: ...}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcErrorObject {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// One parsed line of incoming JSON-RPC traffic.
///
/// Deliberately dispatched by hand (peek at which top-level keys are
/// present, then deserialize exactly once into the matching shape)
/// rather than as a single `#[serde(untagged)]` enum. Untagged
/// matching tries each variant's full deserialization in turn and is
/// easy to get subtly wrong when several variants share an optional
/// `id` field; checking key presence first is more explicit and
/// easier to verify by inspection.
#[derive(Debug, Clone)]
pub enum IncomingMessage {
    /// A successful response to a request this crate sent.
    Result { id: RequestId, result: Value },
    /// An error response to a request this crate sent.
    Error {
        id: RequestId,
        error: JsonRpcErrorObject,
    },
    /// A notification from the peer -- no response expected.
    Notification {
        method: String,
        params: Option<Value>,
    },
    /// A request *from* the peer (e.g. a server asking the client to
    /// sample, or list roots). Phase 27 implements no peer-initiated
    /// methods; see `client::McpClient::drain_until` for how these are
    /// handled without hanging the exchange.
    Request {
        id: RequestId,
        method: String,
        params: Option<Value>,
    },
}

/// Parses one line of text as an [`IncomingMessage`]. A thin wrapper
/// around [`classify`] for callers holding raw text; `transport::recv`
/// already returns a parsed [`Value`], so `client`/`server` call
/// [`classify`] directly rather than re-serializing back to a string
/// only to re-parse it.
pub fn parse_incoming(line: &str) -> Result<IncomingMessage, serde_json::Error> {
    let v: Value = serde_json::from_str(line)?;
    classify(v)
}

/// Determines which of the four JSON-RPC message shapes `v` is, by
/// which top-level keys are present, then deserializes exactly once
/// into the matching shape. See [`IncomingMessage`] for why this is
/// done by hand rather than via `#[serde(untagged)]`.
pub fn classify(v: Value) -> Result<IncomingMessage, serde_json::Error> {
    let has_id = v.get("id").is_some();
    let has_result = v.get("result").is_some();
    let has_error = v.get("error").is_some();
    let has_method = v.get("method").is_some();

    if has_id && has_result {
        Ok(IncomingMessage::Result {
            id: serde_json::from_value(v["id"].clone())?,
            result: v["result"].clone(),
        })
    } else if has_id && has_error {
        Ok(IncomingMessage::Error {
            id: serde_json::from_value(v["id"].clone())?,
            error: serde_json::from_value(v["error"].clone())?,
        })
    } else if has_id && has_method {
        Ok(IncomingMessage::Request {
            id: serde_json::from_value(v["id"].clone())?,
            method: v["method"].as_str().unwrap_or_default().to_string(),
            params: v.get("params").cloned(),
        })
    } else if has_method {
        Ok(IncomingMessage::Notification {
            method: v["method"].as_str().unwrap_or_default().to_string(),
            params: v.get("params").cloned(),
        })
    } else {
        use serde::de::Error as _;
        Err(serde_json::Error::custom(
            "not a recognizable JSON-RPC message: needs (id+result), (id+error), \
             (id+method), or (method)",
        ))
    }
}

/// `clientInfo` / `serverInfo`: MCP's name for "who am I".
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Implementation {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub title: Option<String>,
    pub version: String,
}

/// What this crate declares itself capable of in `initialize`.
/// Deliberately empty: Phase 27 implements no client-side features
/// (sampling, roots, elicitation) -- see `lib.rs` for scope notes.
/// Kept as a real (if minimal) type rather than `json!({})` inline so
/// a future phase adding one of these has a typed place to put it.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ClientCapabilities {}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct ToolsCapability {
    #[serde(rename = "listChanged", default)]
    pub list_changed: bool,
}

/// What the server declared itself capable of. Only the `tools`
/// capability is modeled as a typed field -- everything else the
/// server might declare (resources, prompts, logging, completions,
/// sampling passthroughs, ...) is preserved verbatim in `other` via
/// `#[serde(flatten)]` rather than dropped, but this crate does not
/// act on any of it this phase.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct ServerCapabilities {
    #[serde(default)]
    pub tools: Option<ToolsCapability>,
    #[serde(flatten)]
    pub other: serde_json::Map<String, Value>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct InitializeResult {
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
    #[serde(default)]
    pub capabilities: ServerCapabilities,
    #[serde(rename = "serverInfo")]
    pub server_info: Implementation,
    #[serde(default)]
    pub instructions: Option<String>,
}

/// A tool as the *peer* describes it (`tools/list`). Kept close to
/// the wire: `input_schema` stays a raw [`Value`] here because it is
/// arbitrary JSON Schema, which this workspace's closed
/// `tool_framework::ParamKind` cannot fully represent. Converting it
/// into something `tool_framework::InputSchema` can use is a separate,
/// deliberate step -- see `bridge::to_input_schema`.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Tool {
    pub name: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
    #[serde(rename = "outputSchema", default)]
    pub output_schema: Option<Value>,
    /// Hints only, straight from the spec's own wording: never used
    /// for a trust or permission decision here (see `bridge.rs`).
    #[serde(default)]
    pub annotations: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListToolsResult {
    #[serde(default)]
    pub tools: Vec<Tool>,
    #[serde(rename = "nextCursor", default)]
    pub next_cursor: Option<String>,
}

/// A tool-execution result. Note `is_error`: a failed tool call is
/// still a JSON-RPC *success* (a `result`, not an `error`) with this
/// flag set -- confirmed against a real server (see module docs), not
/// assumed from the schema text alone.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct CallToolResult {
    #[serde(default)]
    pub content: Vec<ContentBlock>,
    #[serde(rename = "isError", default)]
    pub is_error: bool,
    #[serde(rename = "structuredContent", default)]
    pub structured_content: Option<Value>,
}

/// One block of tool-result content. `Unknown` is the forward-compat
/// catch-all for a `type` this crate does not specifically model
/// (e.g. `resource_link`) -- honestly represented as "something came
/// back", not silently dropped or misparsed as a different variant.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image {
        data: String,
        #[serde(rename = "mimeType")]
        mime_type: String,
    },
    #[serde(rename = "audio")]
    Audio {
        data: String,
        #[serde(rename = "mimeType")]
        mime_type: String,
    },
    #[serde(rename = "resource")]
    Resource { resource: Value },
    #[serde(other)]
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn request_serializes_with_no_trailing_params_when_none() {
        let req = JsonRpcRequest::new(RequestId::Number(1), "ping", None);
        let v = req.to_value().unwrap();
        assert_eq!(v, json!({"jsonrpc": "2.0", "id": 1, "method": "ping"}));
    }

    #[test]
    fn notification_has_no_id_field_at_all() {
        let note = JsonRpcNotification::new("notifications/initialized", None);
        let v = note.to_value().unwrap();
        assert!(v.get("id").is_none());
        assert_eq!(v["method"], json!("notifications/initialized"));
    }

    #[test]
    fn parses_a_real_initialize_result_shape() {
        // Exact shape observed from @modelcontextprotocol/server-filesystem
        // 2026.7.10 during manual probing (see CHANGELOG).
        let line = r#"{"result":{"protocolVersion":"2025-11-25","capabilities":{"tools":{"listChanged":true}},"serverInfo":{"name":"secure-filesystem-server","version":"0.2.0"}},"jsonrpc":"2.0","id":1}"#;
        match parse_incoming(line).unwrap() {
            IncomingMessage::Result { id, result } => {
                assert_eq!(id, RequestId::Number(1));
                let init: InitializeResult = serde_json::from_value(result).unwrap();
                assert_eq!(init.protocol_version, "2025-11-25");
                assert_eq!(init.server_info.name, "secure-filesystem-server");
                assert!(init.capabilities.tools.unwrap().list_changed);
            }
            other => panic!("expected Result, got {other:?}"),
        }
    }

    #[test]
    fn parses_a_real_tool_call_error_result() {
        // A *tool-level* failure: still a JSON-RPC result, isError: true.
        // Exact shape observed against the real filesystem server for an
        // unknown tool name.
        let line = r#"{"result":{"content":[{"type":"text","text":"MCP error -32602: Tool bogus not found"}],"isError":true},"jsonrpc":"2.0","id":2}"#;
        match parse_incoming(line).unwrap() {
            IncomingMessage::Result { result, .. } => {
                let call: CallToolResult = serde_json::from_value(result).unwrap();
                assert!(call.is_error);
                match &call.content[0] {
                    ContentBlock::Text { text } => assert!(text.contains("not found")),
                    other => panic!("expected Text, got {other:?}"),
                }
            }
            other => panic!("expected Result, got {other:?}"),
        }
    }

    #[test]
    fn parses_a_real_protocol_level_error() {
        // Exact shape observed for an unknown top-level method.
        let line =
            r#"{"jsonrpc":"2.0","id":4,"error":{"code":-32601,"message":"Method not found"}}"#;
        match parse_incoming(line).unwrap() {
            IncomingMessage::Error { id, error } => {
                assert_eq!(id, RequestId::Number(4));
                assert_eq!(error.code, METHOD_NOT_FOUND);
            }
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[test]
    fn parses_a_notification_with_no_id() {
        let line = r#"{"jsonrpc":"2.0","method":"notifications/tools/list_changed"}"#;
        match parse_incoming(line).unwrap() {
            IncomingMessage::Notification { method, .. } => {
                assert_eq!(method, "notifications/tools/list_changed");
            }
            other => panic!("expected Notification, got {other:?}"),
        }
    }

    #[test]
    fn rejects_a_line_matching_none_of_the_four_shapes() {
        assert!(parse_incoming(r#"{"jsonrpc":"2.0"}"#).is_err());
    }

    #[test]
    fn tool_with_unmodeled_content_type_falls_back_to_unknown_not_an_error() {
        let v = json!({"type": "resource_link", "uri": "file:///x"});
        let block: ContentBlock = serde_json::from_value(v).unwrap();
        assert!(matches!(block, ContentBlock::Unknown));
    }

    #[test]
    fn server_capabilities_preserves_unmodeled_categories() {
        let v = json!({"tools": {"listChanged": true}, "resources": {"subscribe": true}});
        let caps: ServerCapabilities = serde_json::from_value(v).unwrap();
        assert!(caps.tools.unwrap().list_changed);
        assert_eq!(
            caps.other.get("resources").unwrap()["subscribe"],
            json!(true)
        );
    }

    #[test]
    fn request_id_round_trips_both_number_and_string() {
        assert_eq!(
            serde_json::from_str::<RequestId>("7").unwrap(),
            RequestId::Number(7)
        );
        assert_eq!(
            serde_json::from_str::<RequestId>("\"abc\"").unwrap(),
            RequestId::String("abc".into())
        );
    }
}
