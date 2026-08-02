//! Core LLM-protocol types used across laipe.

use serde::{Deserialize, Serialize};

/// Which provider protocol to speak.
///
/// LLM clients in the wild have settled on three wire formats. laipe
/// implements all three so apps can swap providers without code changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiFormat {
    /// OpenAI `/v1/chat/completions` (`data: {...}\n\n` SSE).
    OpenAiChat,
    /// OpenAI `/v1/responses` (`event: response.output_item.added` etc.).
    OpenAiResponses,
    /// Anthropic `/v1/messages` (`event: content_block_delta` etc.).
    Anthropic,
}

/// Per-run reasoning effort / thinking level.
///
/// Most providers map this to a header or a body field. Defaults to None.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EffortLevel {
    Low,
    Medium,
    High,
}

/// Where a chat message sits in a conversation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    System,
    #[default]
    User,
    Assistant,
    /// Tool result echo (cross-protocol)
    Tool,
}

/// Chat state machine value, mirrored 1:1 from `useStreamReducer` on the TS
/// side. See `docs/STATE_MACHINE.md` for the full set of valid transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatStatus {
    Idle,
    Streaming,
    Error,
    Cancelled,
}

/// A single chat message in any of the three protocols.
///
/// The `tool_call_id` / `tool_calls` fields are only used in the tool-calling
/// flow:
/// - `role = Tool` requires `tool_call_id` (set by the consumer when responding
///   to an assistant tool call).
/// - `role = Assistant` may carry `tool_calls` (the model's declared calls;
///   the LLM is responsible for emitting these, laipe does not synthesize them).
///
/// In the streaming agent loop, the Rust side appends the assistant message
/// (with `tool_calls`) and a `role = Tool` message for each executed tool.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
    /// OpenAI Responses: carry a tool_call_id when role=Tool
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// OpenAI Chat Completions: assistant may carry N tool_calls
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<AssistantToolCall>>,
}

/// Lightweight per-run config (endpoint, key, model, effort).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub endpoint: String,
    pub api_key: String,
    pub model: String,
    pub api_format: ApiFormat,
    #[serde(default)]
    pub effort: Option<EffortLevel>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub temperature: Option<f32>,
    /// Cross-protocol tool schema: OpenAI `tools: [{type, function: {name, ...}}]`,
    /// Anthropic `tools: [{name, description, input_schema}]`, Responses flat.
    /// If `None` or empty, laipe does NOT write a `tools` field on the wire —
    /// the upstream then has zero knowledge tools exist (user requirement).
    #[serde(default)]
    pub tools: Option<Vec<crate::ToolDefinition>>,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            endpoint: String::new(),
            api_key: String::new(),
            model: String::new(),
            api_format: ApiFormat::OpenAiChat,
            effort: None,
            max_tokens: None,
            temperature: None,
            tools: None,
        }
    }
}

/// A tool call attached to an assistant message (OpenAI Chat shape).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String, // always "function" today
    pub function: AssistantToolCallFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantToolCallFunction {
    pub name: String,
    /// The accumulating JSON arguments string. May be partial.
    pub arguments: String,
}

/// What comes back from a `pick(api_format).dispatch(...)` receiver.
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// A text delta — append-only.
    Text(String),
    /// One or more tool-call partials (may be sent in fragments; consumer
    /// should accumulate by `index` then call `complete()` once `Done`).
    ToolCalls(Vec<crate::ToolCallPartial>),
    /// Stream finished cleanly.
    Done,
    /// Mid-stream error (e.g. SSE protocol violation, socket read failure).
    /// Errors before the stream opens come back as `Err(StreamError)` from
    /// `run()` instead.
    Error {
        kind: crate::ChatErrorKind,
        message: String,
    },
}
