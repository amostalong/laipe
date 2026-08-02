//! Tool / function-calling schema shared with the TS layer.

use serde::{Deserialize, Serialize};

/// OpenAI Chat Completions tool definition format (the wire format laipe
/// keeps internally regardless of which protocol is on the wire).
///
/// On Anthropic, the Rust `build_anthropic_request_body` flattens this to
/// `[{name, description, input_schema: parameters}]`. On Responses, it
/// flattens to `[{type, name, description, parameters}]`. See
/// `docs/TOOL_CALLING.md` for the full table.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub kind: ToolType,
    pub function: ToolFunction,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolType {
    #[default]
    Function,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ToolFunction {
    pub name: String,
    pub description: String,
    /// JSON Schema describing the function's parameters.
    pub parameters: serde_json::Value,
}

/// A tool call as it appears in the assistant's emitted message. The TS
/// state machine accumulates `partial.arguments` (a JSON string) until
/// `Done` is received, then JSON.parse()s and dispatches.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallInfo {
    pub id: String,
    pub name: String,
    /// Final, JSON-decoded arguments.
    pub arguments: serde_json::Value,
}

/// A single tool-call partial streamed from upstream. The consumer
/// accumulates by `index` until the stream completes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallPartial {
    /// Streaming order (0-based). May be sent in fragments per call.
    pub index: u32,
    /// May be None on the very first delta (the id arrives first).
    pub id: Option<String>,
    /// Function name, may be None on first delta.
    pub name: Option<String>,
    /// JSON arguments string (raw, partial). Empty on first delta.
    pub arguments_delta: String,
}

/// Player's reply to a tool call — sent back to the LLM as a
/// `role: "tool"` (OpenAI) / `role: "user"` content block
/// `{type: "tool_result", ...}` (Anthropic) message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_call_id: String,
    pub content: String,
}
