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

// =============================================================================
// Test connection
// =============================================================================

/// Parameters for `test_provider` — non-streaming ping that validates an
/// endpoint + api_key + model combo end-to-end.
///
/// Consumers (Tauri commands, server-side health checks) call this with
/// the user's saved config. The laipe-streaming impl handles auth header
/// construction, body serialization, and 3-protocol response parsing —
/// callers just hand over the 4 fields and read the result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestProviderParams {
    /// Base URL — the protocol-specific path is appended internally.
    pub endpoint: String,
    /// API key. Empty string is allowed (some local-only endpoints skip auth).
    pub api_key: String,
    /// Wire protocol. Drives auth header, body shape, and response parser.
    pub api_format: ApiFormat,
    /// Model id — sent in the request body.
    pub model: String,
}

/// Result of `test_provider`.
///
/// `ok = true` means the request returned 2xx **and** we successfully
/// extracted a non-empty text fragment from the response body (so we know
/// the auth + model id both worked, not just that the server is up).
///
/// `ok = false` carries the failure reason:
/// - `error` — human-readable summary (HTTP status + body preview, or a
///   network/HTTP error message).
/// - `status` — HTTP status code when we got one back.
/// - `response` — first content text if the body parsed but extraction
///   yielded nothing (rare; usually a sign the protocol is "right" but
///   the model is misconfigured).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestProviderResult {
    pub ok: bool,
    /// HTTP status code, if we got a response back at all.
    pub status: Option<u16>,
    /// Human-readable error message. Present when `ok = false`.
    pub error: Option<String>,
    /// First content text fragment from the response (truncated to 200 chars
    /// server-side). Surfaced in the UI so users can see "yep the model
    /// actually answered something".
    pub response: Option<String>,
    /// Echoed input — useful for UI to show "tested X with Y".
    pub endpoint: String,
    pub model: String,
    pub api_format: ApiFormat,
}
