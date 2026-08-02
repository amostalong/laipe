//! 8-way error classification shared with the TS layer.
//!
//! These kinds are mirrored 1:1 in TS as `ChatErrorKind` in
//! `packages/laipe-ts/src/errors.ts`. They drive the player-facing copy
//! (see `lib/error-messages.ts`) and the dev-only "copy diagnostic info"
//! button.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatErrorKind {
    /// Connect / TLS / DNS / read timeout
    Network,
    /// 401 / 403 — api key wrong, missing, or expired
    Auth,
    /// 404 — model name not recognized by upstream
    ModelNotFound,
    /// 400 — bad request (malformed messages, wrong params)
    BadRequest,
    /// 429 — rate limit / quota exceeded
    RateLimit,
    /// 5xx — upstream server error
    ServerError,
    /// SSE protocol violated (unexpected chunk shape, missing `[DONE]`, etc.)
    StreamProtocol,
    /// Anything we don't recognize
    Unknown,
}

impl ChatErrorKind {
    /// All 8 kinds, in the same order as the variants are declared.
    /// Used by diagnostic tooling (saved reports, README-FOR-LLM.md)
    /// to iterate the taxonomy without hard-coding the order in
    /// each consumer.
    pub const ALL: &'static [ChatErrorKind] = &[
        Self::Network,
        Self::Auth,
        Self::ModelNotFound,
        Self::BadRequest,
        Self::RateLimit,
        Self::ServerError,
        Self::StreamProtocol,
        Self::Unknown,
    ];

    /// Stable snake_case name. Mirrors the serde representation so it
    /// round-trips through `serde_json::to_value` unchanged.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Network => "network",
            Self::Auth => "auth",
            Self::ModelNotFound => "model_not_found",
            Self::BadRequest => "bad_request",
            Self::RateLimit => "rate_limit",
            Self::ServerError => "server_error",
            Self::StreamProtocol => "stream_protocol",
            Self::Unknown => "unknown",
        }
    }

    /// Human-friendly label. Used in the README-FOR-LLM.md and in the
    /// saved report's "Likely causes" section header.
    pub fn label(self) -> &'static str {
        match self {
            Self::Network => "Network / connect / TLS / timeout",
            Self::Auth => "Authentication failed (401 / 403)",
            Self::ModelNotFound => "Model not found (404)",
            Self::BadRequest => "Bad request (400 / 4xx)",
            Self::RateLimit => "Rate limit / quota exceeded (429)",
            Self::ServerError => "Upstream server error (5xx)",
            Self::StreamProtocol => "SSE protocol violation",
            Self::Unknown => "Unclassified error",
        }
    }

    /// LLM-facing debug hint: a 3-6 line markdown block listing the
    /// most common causes for this error class and concrete next
    /// debugging steps. This is the **single source of truth** for
    /// per-kind debug recipes — the README-FOR-LLM.md generator and
    /// every saved report's "Likely causes" section both render from
    /// this string. Keep it current; the LLM assistant uses it to
    /// decide what to investigate first.
    ///
    /// Format: a markdown bullet list. First line is a one-sentence
    /// summary, subsequent lines are concrete checks.
    pub fn to_debug_hint(self) -> &'static str {
        match self {
            Self::Network => "\
- The Rust side could not connect, complete TLS handshake, or read the response within the timeout.
- Check the `endpoint` in Settings — typo, wrong scheme (http vs https), or a stale custom URL are the usual culprits.
- If using a self-hosted LLM (llama.cpp, vLLM, ollama, etc.), confirm the server is running and listening on the configured port (`curl {endpoint}/v1/models` from the same machine is a fast smoke test).
- Corporate proxies or VPNs can break TLS or DNS; try a different network to isolate.
- Look at the `cause` field in the console entry for the underlying reqwest error string (it carries the OS-level reason).",
            Self::Auth => "\
- The API key was rejected (HTTP 401 / 403).
- Re-paste the key in Settings — leading/trailing whitespace or a truncated paste are the #1 cause.
- Confirm the key has access to the model you selected. Some providers (OpenAI org-scoped keys, Azure, Anthropic workspaces) require extra setup.
- If the key was just rotated, the provider may take a few seconds to propagate; retry once.
- The saved report's request body has the `Authorization` header redacted; check the raw response body for the upstream's exact rejection reason.",
            Self::ModelNotFound => "\
- The model id you selected is not served by the endpoint, or the API key lacks access to it.
- Cross-check the model id in the provider's model list. Common gotchas: case (`gpt-4o` vs `GPT-4o`), version suffix (`-preview`, `-2024-08-06`), provider aliases (Anthropic `claude-3-5-sonnet-latest` vs the dated id).
- If using OpenAI-compatible third-party providers (OpenRouter, DeepSeek, GLM, etc.), each one accepts its own model id set; the curated catalog in the app lists the supported ones.",
            Self::BadRequest => "\
- The request body was rejected as malformed (HTTP 400, sometimes 422).
- The most common cause is a tool schema that the provider can't parse — required field missing, wrong JSON Schema type, or a `parameters` object that doesn't have `type: \"object\"` at the root.
- Cross-protocol tool translation is a known sharp edge: a tool that works on `openai_chat` may need a tweak to work on `openai_responses` (different field names).
- Long conversation history can hit context-window limits — the request may need summarization before the next retry.
- The saved report's request body shows exactly what the provider saw; the response body shows the provider's structured error.",
            Self::RateLimit => "\
- You've exceeded the provider's per-minute token / request quota (HTTP 429).
- Most providers return a `Retry-After` header; the saved report's response digest shows the status, and the raw response body has the full retry instructions.
- If the rate limit is per-organization (not per-key), switching keys won't help — you have to wait or upgrade the plan.
- The agent loop in this app retries the same turn automatically once after a short delay; if you see two 429s back-to-back, the limit is being hit every turn and the conversation is too chatty.",
            Self::ServerError => "\
- The provider's server returned 5xx. This is on their side, not yours.
- Retry once after a few seconds — transient 502 / 503 / 504 are common during provider restarts.
- Check the provider's status page before assuming the app is broken.
- If the error persists across multiple minutes and multiple models, your network path (proxy, firewall) is likely rewriting or dropping responses.",
            Self::StreamProtocol => "\
- The SSE byte stream violated the expected wire shape (missing `[DONE]`, unexpected event type, malformed JSON, premature close).
- This almost always means the endpoint is not actually speaking the protocol you configured. Switch `api_format` to the right one for the endpoint.
- Self-hosted servers behind reverse proxies (nginx, Caddy) often buffer or chunk the response, breaking the SSE framing. Try disabling proxy buffering for the path.
- A custom openai-compatible provider may add a non-standard event the parser doesn't know — the saved report's raw response body shows the offending frames.",
            Self::Unknown => "\
- The error didn't match any of the 7 known classes. This is rare; usually it's a wrapped library error that escaped classification.
- The `cause` field in the console entry has the original error string; that's the most useful starting point for the LLM assistant.
- The saved report includes the full request + raw response — paste both into the LLM and ask it to classify and propose a fix.",
        }
    }
}

/// Optional developer-facing diagnostic blob (original reqwest error string,
/// status code, request id from upstream, etc.). Not shown to the player by
/// default — only behind a "copy diagnostic info" button.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChatErrorDiag {
    /// HTTP status code if any
    pub status: Option<u16>,
    /// First 1KB of the upstream error body (if any)
    pub body: Option<String>,
    /// Upstream `x-request-id` header (for support tickets)
    pub request_id: Option<String>,
    /// Internal stage where the error was raised (e.g. "build_body",
    /// "parse_sse", "decode_tool_call")
    pub stage: Option<String>,
}
