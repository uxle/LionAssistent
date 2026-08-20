//! "External Tool Integration" (Development_plan Phase 27): turns
//! tools discovered from a connected [`McpClient`] into ordinary
//! `tool_framework::Tool` registrations, and turns this workspace's
//! own tools into MCP-shaped JSON Schema for `server.rs` to advertise.
//! Once registered, an MCP-sourced tool is indistinguishable to
//! `ToolRouter` from a built-in one: same validation, same permission
//! check, same timeout enforcement, same audit trail. TCLIS SS11
//! ("Plugin Tool Support... follow the same validation and permission
//! pipeline as built-in tools") already established this principle for
//! plugin-contributed tools; an MCP server is just another source of
//! tools this assistant did not write itself, so the same principle
//! applies without needing new reasoning.
//!
//! # Why this doesn't register tools *as an LLM-callable tool call*
//!
//! It would be natural to expect a `connect_mcp_server` tool the model
//! itself can invoke. Deliberately not built this phase: registering a
//! newly discovered tool needs `&mut ToolRegistry`, but
//! `tool_framework::Tool::execute` only ever gets `&self` with no path
//! back to the live registry it is being dispatched from (by design --
//! `tool-framework`'s own docs note tools and the router that dispatches
//! them are kept structurally separate). Giving a tool that access would
//! mean either a new registry-mutation channel threaded through the
//! router, or wrapping the whole `ToolRegistry` in a `Mutex` so
//! something outside the dispatch path could reach in -- either is a
//! real design change to `tool-framework` itself, not a Phase 27
//! concern, and guessing at one now risks the same mistake Phase 7
//! deliberately avoided by not wiring itself into `assistant-core`
//! before a real caller existed. What Phase 27 delivers instead: a
//! real, fully working [`McpClient`], and [`register_mcp_tools`] for
//! whoever already holds `&mut ToolRegistry` at construction time --
//! `app`'s startup sequence, most likely, alongside
//! `linux_services::tools::register_all` and every other phase's own
//! `register_all` -- to call once connected servers are known.
//!
//! # Permission level
//!
//! Every MCP-sourced tool registers at [`PermissionLevel::Moderate`],
//! regardless of what the remote tool's own `annotations` claim. The
//! real schema is explicit that annotations are hints a client "should
//! never make tool use decisions" based on when the server isn't
//! trusted -- confirmed directly from the fetched 2025-11-25 schema,
//! not inferred. This crate has no way to establish that trust for an
//! arbitrary configured server, so every proxied tool gets the same
//! floor: never auto-approved, always something a real
//! `PermissionChecker` must confirm.

use crate::client::McpClient;
use crate::protocol::{ContentBlock, Tool as McpTool};
use common::{AppResult, PermissionLevel};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tool_framework::{InputSchema, ParamKind, ParamSchema, ToolArgs, ToolCategory, ToolOutcome};

/// Generous but bounded: a well-behaved local MCP server answers in
/// milliseconds, but this crate cannot audit an arbitrary external
/// tool's own implementation, so the ceiling errs toward "long enough
/// that a legitimately slower external tool still finishes" rather
/// than "tight enough to assume good behavior" -- `ToolRouter::execute`
/// enforces this for real (Phase 7), the same as every other tool.
const MCP_TOOL_TIMEOUT: Duration = Duration::from_secs(30);

/// Converts a workspace-native [`InputSchema`] into the JSON Schema
/// object shape MCP's `Tool.inputSchema` expects (real shape confirmed
/// from the fetched schema and from a real server's `tools/list`
/// response). Lossless in this direction: every [`ParamKind`] has an
/// exact JSON Schema equivalent, including [`ParamKind::Any`], which
/// becomes `{}` -- the empty schema, which in JSON Schema legitimately
/// means "any value accepted", an honest representation rather than a
/// guess at a narrower type.
pub fn to_json_schema(schema: &InputSchema) -> Value {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();
    for param in &schema.params {
        properties.insert(param.name.clone(), param_kind_to_json_schema(param.kind));
        if param.required {
            required.push(Value::String(param.name.clone()));
        }
    }
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
    })
}

fn param_kind_to_json_schema(kind: ParamKind) -> Value {
    match kind {
        ParamKind::String => json!({"type": "string"}),
        ParamKind::Integer => json!({"type": "integer"}),
        ParamKind::Float => json!({"type": "number"}),
        ParamKind::Bool => json!({"type": "boolean"}),
        ParamKind::StringArray => json!({"type": "array", "items": {"type": "string"}}),
        ParamKind::Any => json!({}),
    }
}

/// Converts an external tool's arbitrary JSON Schema `inputSchema`
/// into a best-effort [`InputSchema`]. The reverse direction is
/// necessarily lossy -- JSON Schema can express nested objects,
/// `enum`/`oneOf`/`anyOf`, typed arrays, and far more than
/// `ParamKind`'s five closed variants -- so every declared top-level
/// property is preserved (as [`ParamKind::Any`] when nothing more
/// specific applies), rather than silently dropping properties this
/// crate cannot fully type. Preserving the property is what matters
/// for `InputSchema::validate` to accept it at all: an *omitted*
/// property would be rejected outright as
/// [`tool_framework::ValidationError::UnexpectedParam`] the moment a
/// caller passed it, which would be strictly worse than accepting it
/// untyped and letting the remote server -- which does know its own
/// real schema -- validate it on the far side.
///
/// Known, deliberate limitation: a schema using `additionalProperties`
/// to accept properties it does not explicitly list is not honored --
/// only declared `properties` entries become params. Uncommon for
/// tool-call schemas in practice (every real tool observed during
/// development declared its properties explicitly), and honoring it
/// would mean accepting literally any key, which is a different and
/// much weaker guarantee than "every accepted key was declared
/// somewhere".
pub fn to_input_schema(json_schema: &Value) -> InputSchema {
    let required: HashSet<&str> = json_schema
        .get("required")
        .and_then(Value::as_array)
        .map(|arr| arr.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();

    let mut params = Vec::new();
    if let Some(properties) = json_schema.get("properties").and_then(Value::as_object) {
        for (name, prop_schema) in properties {
            let kind = infer_param_kind(prop_schema);
            params.push(if required.contains(name.as_str()) {
                ParamSchema::required(name.clone(), kind)
            } else {
                ParamSchema::optional(name.clone(), kind)
            });
        }
    }
    InputSchema::new(params)
}

fn infer_param_kind(prop_schema: &Value) -> ParamKind {
    match prop_schema.get("type").and_then(Value::as_str) {
        Some("string") => ParamKind::String,
        Some("integer") => ParamKind::Integer,
        Some("number") => ParamKind::Float,
        Some("boolean") => ParamKind::Bool,
        Some("array") => {
            let items_are_strings = prop_schema
                .get("items")
                .and_then(|items| items.get("type"))
                .and_then(Value::as_str)
                == Some("string");
            if items_are_strings {
                ParamKind::StringArray
            } else {
                ParamKind::Any
            }
        }
        // "object", a missing/array-valued "type" (e.g. ["string","null"]),
        // enum-only schemas, oneOf/anyOf/allOf composition -- every
        // shape this crate does not specifically model. Any is the
        // honest fallback: still declared and thus acceptable, never
        // silently mistyped.
        _ => ParamKind::Any,
    }
}

/// A `tool_framework::Tool` that proxies every call to one tool on a
/// connected [`McpClient`]. See the module docs for the permission
/// level and timeout reasoning.
pub struct McpToolProxy {
    client: Arc<McpClient>,
    name: String,
    description: String,
    input_schema: InputSchema,
}

impl McpToolProxy {
    pub fn new(client: Arc<McpClient>, remote_tool: &McpTool) -> Self {
        let description = remote_tool.description.clone().unwrap_or_else(|| {
            format!(
                "(the MCP server providing {:?} declared no description)",
                remote_tool.name
            )
        });
        Self {
            client,
            name: remote_tool.name.clone(),
            description,
            input_schema: to_input_schema(&remote_tool.input_schema),
        }
    }
}

impl tool_framework::Tool for McpToolProxy {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn category(&self) -> ToolCategory {
        ToolCategory::Mcp
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Moderate
    }

    fn timeout(&self) -> Duration {
        MCP_TOOL_TIMEOUT
    }

    fn input_schema(&self) -> InputSchema {
        self.input_schema.clone()
    }

    fn execute(&self, args: &ToolArgs) -> ToolOutcome {
        let arguments = Value::Object(args.iter().map(|(k, v)| (k.clone(), v.clone())).collect());

        match self.client.call_tool(&self.name, arguments) {
            Ok(result) => {
                let text = result
                    .content
                    .iter()
                    .filter_map(|block| match block {
                        ContentBlock::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                let message = if text.is_empty() {
                    format!(
                        "{} {} with no text content in the response",
                        self.name,
                        if result.is_error {
                            "reported a failure"
                        } else {
                            "completed"
                        }
                    )
                } else {
                    text
                };
                ToolOutcome {
                    success: !result.is_error,
                    message,
                    data: result.structured_content,
                }
            }
            Err(e) => ToolOutcome {
                success: false,
                message: format!("calling {:?} via MCP failed: {e}", self.name),
                data: None,
            },
        }
    }
}

/// Discovers every tool `client` offers and registers a proxy for
/// each into `registry` -- the "External Tool Integration" bridge.
/// Returns how many were registered.
///
/// Stops at the first registration failure (almost always a name
/// collision with an already-registered tool -- `ToolRegistry::register`
/// refuses to silently shadow one) and leaves every tool registered
/// before that point in place; this function does not roll back
/// partial registration on error. Callers that need all-or-nothing
/// semantics should register into a fresh `ToolRegistry` and merge it
/// in only on success.
pub fn register_mcp_tools(
    registry: &mut tool_framework::ToolRegistry,
    client: &Arc<McpClient>,
) -> AppResult<usize> {
    let tools = client.list_tools()?;
    let mut registered = 0usize;
    for remote_tool in &tools {
        registry.register(Arc::new(McpToolProxy::new(Arc::clone(client), remote_tool)))?;
        registered += 1;
    }
    Ok(registered)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_json_schema_covers_every_param_kind() {
        let schema = InputSchema::new(vec![
            ParamSchema::required("s", ParamKind::String),
            ParamSchema::required("i", ParamKind::Integer),
            ParamSchema::optional("f", ParamKind::Float),
            ParamSchema::optional("b", ParamKind::Bool),
            ParamSchema::optional("arr", ParamKind::StringArray),
            ParamSchema::optional("any", ParamKind::Any),
        ]);
        let v = to_json_schema(&schema);
        assert_eq!(v["type"], json!("object"));
        assert_eq!(v["properties"]["s"], json!({"type": "string"}));
        assert_eq!(v["properties"]["i"], json!({"type": "integer"}));
        assert_eq!(v["properties"]["f"], json!({"type": "number"}));
        assert_eq!(v["properties"]["b"], json!({"type": "boolean"}));
        assert_eq!(
            v["properties"]["arr"],
            json!({"type": "array", "items": {"type": "string"}})
        );
        assert_eq!(v["properties"]["any"], json!({}));
        let required = v["required"].as_array().unwrap();
        assert_eq!(required.len(), 2);
        assert!(required.contains(&json!("s")));
        assert!(required.contains(&json!("i")));
    }

    #[test]
    fn to_json_schema_of_an_empty_schema_still_has_a_required_array() {
        let v = to_json_schema(&InputSchema::default());
        assert_eq!(v["required"], json!([]));
        assert_eq!(v["properties"], json!({}));
    }

    #[test]
    fn to_input_schema_maps_every_simple_json_schema_type() {
        let json_schema = json!({
            "type": "object",
            "properties": {
                "path": {"type": "string"},
                "count": {"type": "integer"},
                "ratio": {"type": "number"},
                "flag": {"type": "boolean"},
                "tags": {"type": "array", "items": {"type": "string"}},
            },
            "required": ["path", "count"],
        });
        let schema = to_input_schema(&json_schema);
        assert_eq!(schema.params.len(), 5);

        let find = |name: &str| schema.params.iter().find(|p| p.name == name).unwrap();
        assert_eq!(find("path").kind, ParamKind::String);
        assert!(find("path").required);
        assert_eq!(find("count").kind, ParamKind::Integer);
        assert!(find("count").required);
        assert_eq!(find("ratio").kind, ParamKind::Float);
        assert!(!find("ratio").required);
        assert_eq!(find("flag").kind, ParamKind::Bool);
        assert_eq!(find("tags").kind, ParamKind::StringArray);
    }

    #[test]
    fn to_input_schema_falls_back_to_any_for_shapes_it_cannot_type() {
        let json_schema = json!({
            "type": "object",
            "properties": {
                "nested": {"type": "object", "properties": {"x": {"type": "string"}}},
                "mixed_array": {"type": "array", "items": {"type": "number"}},
                "nullable": {"type": ["string", "null"]},
                "choice": {"enum": ["a", "b", "c"]},
            },
            "required": [],
        });
        let schema = to_input_schema(&json_schema);
        let find = |name: &str| schema.params.iter().find(|p| p.name == name).unwrap();
        assert_eq!(find("nested").kind, ParamKind::Any);
        assert_eq!(find("mixed_array").kind, ParamKind::Any);
        assert_eq!(find("nullable").kind, ParamKind::Any);
        assert_eq!(find("choice").kind, ParamKind::Any);
    }

    #[test]
    fn to_input_schema_of_a_schema_with_no_properties_is_empty_not_an_error() {
        let schema = to_input_schema(&json!({"type": "object"}));
        assert!(schema.params.is_empty());
    }

    #[test]
    fn every_declared_property_is_accepted_by_validate_round_trip() {
        // The property that motivated keeping every declared property
        // (as Any when untypeable) instead of dropping it: it must
        // still be *accepted*, or InputSchema::validate would reject a
        // perfectly legitimate remote-tool argument as unexpected.
        let json_schema = json!({
            "type": "object",
            "properties": {"nested": {"type": "object"}},
            "required": ["nested"],
        });
        let schema = to_input_schema(&json_schema);
        let mut args = ToolArgs::new();
        args.insert("nested".into(), json!({"anything": ["goes", 1, true]}));
        assert!(schema.validate(&args).is_ok());
    }
}
