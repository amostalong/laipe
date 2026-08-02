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
