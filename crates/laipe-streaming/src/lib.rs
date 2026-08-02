//! laipe-streaming — 3-protocol SSE streaming implementations
//!
//! This crate implements the `StreamEvent` producer side for the three LLM
//! protocols laipe supports today:
//!
//! - **OpenAI Chat Completions** (`openai_chat`) — `stream_chat` API
//! - **OpenAI Responses** (`openai_responses`) — `responses` API with
//!   server-sent `output_item.added` / `function_call_arguments.delta` events
//! - **Anthropic Messages** (`anthropic`) — `messages` API with `content_block_*`
//!   / `message_delta` events
//!
//! All three yield a `tokio::sync::mpsc::Receiver<StreamEvent>` whose items
//! are either text chunks or tool-call partials. Consumers (typically a Tauri
//! command or web SSE handler) translate these into the wire format their
//! frontends want.
//!
//! ## Performance
//!
//! The four anti-stutter countermeasures from the [Locus battle-test] carry
//! over unchanged:
//!
//! 1. `tokio::task::spawn_blocking` isolates SSE byte → JSON parse
//! 2. `mpsc::channel(64)` decouples parse from emit
//! 3. 16ms rAF + 256-char batch on the emit side
//! 4. Identity-stable downstream state (consumer reuses the same buffer)
//!
//! [Locus battle-test]: https://github.com/amostalong/Locus
//!
//! See `docs/STREAMING.md` for the protocol details and
//! `docs/TOOL_CALLING.md` for cross-protocol tool schema translation.

#![doc(html_root_url = "https://docs.rs/laipe-streaming/0.1.0")]

pub mod anthropic;
pub mod openai_chat;
pub mod openai_responses;
pub mod sse;
pub mod throttle;

use async_trait::async_trait;
use laipe_core::error::ChatErrorKind;
use laipe_core::types::{ApiFormat, ChatMessage, ProviderConfig, StreamEvent};
use thiserror::Error;
use tokio::sync::mpsc;

/// Convenience alias for `Result<T, StreamError>`.
pub type StreamResult<T> = std::result::Result<T, StreamError>;

/// Errors raised by streaming implementations. Mid-stream errors after the
/// stream has opened are surfaced as `StreamEvent::Error` instead.
#[derive(Debug, Error)]
pub enum StreamError {
    /// HTTP-level error (connect, TLS, timeout)
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// Underlying tokio task / IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON deserialize failure during SSE parse
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    /// Upstream returned a non-2xx status with a body
    #[error("upstream returned {status}: {body_preview}")]
    Upstream {
        kind: ChatErrorKind,
        status: u16,
        body_preview: String,
    },

    /// Anything else — surfaced as-is
    #[error("{0}")]
    Other(String),
}

/// Trait every protocol implementation satisfies.
#[async_trait]
pub trait StreamChat {
    /// Run a chat completion. The returned `Receiver` yields `StreamEvent`
    /// items until the upstream closes or `cancel()` is called.
    async fn run(
        &self,
        cfg: &ProviderConfig,
        messages: &[ChatMessage],
        tools: Option<&[laipe_core::ToolDefinition]>,
    ) -> Result<mpsc::Receiver<StreamEvent>, StreamError>;
}

/// Classify an upstream non-2xx response into a `StreamError` with the right
/// `ChatErrorKind`. Public so the three protocol implementations share it.
pub fn classify_upstream_error(status: u16, body: &str) -> StreamError {
    let kind = match status {
        401 | 403 => ChatErrorKind::Auth,
        404 => ChatErrorKind::ModelNotFound,
        429 => ChatErrorKind::RateLimit,
        500..=599 => ChatErrorKind::ServerError,
        400..=499 => ChatErrorKind::BadRequest,
        _ => ChatErrorKind::Unknown,
    };
    let body_preview = body.chars().take(800).collect::<String>();
    StreamError::Upstream {
        kind,
        status,
        body_preview,
    }
}

/// Map a reqwest error into a `StreamError` carrying any upstream status
/// it managed to read.
pub fn map_reqwest_error(e: reqwest::Error) -> StreamError {
    if let Some(status) = e.status() {
        return classify_upstream_error(status.as_u16(), &e.to_string());
    }
    StreamError::Other(e.to_string())
}

/// Pick a streaming implementation from an `ApiFormat`.
pub fn pick(fmt: ApiFormat) -> &'static dyn StreamChatDispatch {
    use ApiFormat::*;
    match fmt {
        OpenAiChat => &openai_chat::OpenAiChatStreamer,
        OpenAiResponses => &openai_responses::OpenAiResponsesStreamer,
        Anthropic => &anthropic::AnthropicStreamer,
    }
}

/// Type-erased dispatcher so the trait-object chain works without
/// `async_trait` at the call site.
#[async_trait]
pub trait StreamChatDispatch: Send + Sync {
    async fn dispatch(
        &self,
        cfg: &ProviderConfig,
        messages: &[ChatMessage],
        tools: Option<&[laipe_core::ToolDefinition]>,
    ) -> Result<mpsc::Receiver<StreamEvent>, StreamError>;
}

#[async_trait]
impl<T: StreamChat + Send + Sync> StreamChatDispatch for T {
    async fn dispatch(
        &self,
        cfg: &ProviderConfig,
        messages: &[ChatMessage],
        tools: Option<&[laipe_core::ToolDefinition]>,
    ) -> Result<mpsc::Receiver<StreamEvent>, StreamError> {
        StreamChat::run(self, cfg, messages, tools).await
    }
}
