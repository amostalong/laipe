//! LLM-debuggable diagnostic context for chat errors.
//!
//! Every error raised by laipe (pre-stream `StreamError` or mid-stream
//! `StreamEvent::Error`) carries enough context for an LLM assistant to
//! understand and fix the problem without re-running the original request.
//!
//! ## Two layers of debugging
//!
//! 1. **In-memory** — `ConsoleEntry` fields (`kind`, `request_digest`,
//!    `response_digest`, `cause`) give the running app's debug console
//!    structured context per error.
//! 2. **On-disk** — `DiagnosticRecorder` captures the **full** request
//!    body + **raw** response bytes per chat turn, so the user can hand
//!    a single self-contained `.md` file to their LLM assistant.
//!
//! ## Design principles
//!
//! - **Zero new HTTP/async deps in `laipe-core`.** This file is plain
//!   serde types; the I/O lives in `laipe-streaming` (recorder) and
//!   `laipe-app` (Tauri commands).
//! - **LLM-readable by default.** The `ChatErrorKind::to_debug_hint()`
//!   method returns a `&'static str` common-cause + next-step recipe
//!   per error class. This is the content that lands in the
//!   `README-FOR-LLM.md` and in each saved report.
//! - **Pluggable.** Apps that don't want the diagnostic overhead don't
//!   have to use the recorder. `laipe-streaming` ships a no-op
//!   `NullRecorder` as the default.
//!
//! See `.agents/docs/DIAGNOSTICS.md` for the full design.

use serde::{Deserialize, Serialize};

/// Optional diagnostic context that travels alongside an error.
///
/// All fields are optional because the streaming layer only fills them
/// when it has the data. `request_digest` is always present for
/// `StreamError::Upstream`; `response_digest` is present for
/// `StreamEvent::Error`; `cause` is the original lower-level error
/// (reqwest / serde / etc.) the streaming layer classified.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorContext {
    /// Compact summary of the outgoing request.
    /// Example: `model=gpt-4o format=openai_chat messages=5 tools=2 (~2.3KB)`.
    /// The full request body lives in the on-disk report, never here.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_digest: Option<String>,

    /// Compact summary of the incoming response.
    /// Example: `status=429 content-type=application/json body=312B (truncated to 800B)`.
    /// The full raw bytes live in the on-disk report, never here.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_digest: Option<String>,

    /// The original lower-level error string the streaming layer
    /// classified. Useful for stack-style chain reading; the user-facing
    /// copy is the cleaner `message` on `StreamEvent::Error`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,

    /// Internal stage at which the error was raised
    /// (e.g. `"build_body"`, `"connect"`, `"parse_sse"`, `"decode_tool_call"`).
    /// Mirrors `ChatErrorDiag::stage` but lives here so the context
    /// is self-contained.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage: Option<String>,

    /// Upstream `x-request-id` / `request-id` header if surfaced. Useful
    /// when opening a support ticket with the LLM provider.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upstream_request_id: Option<String>,
}

impl ErrorContext {
    /// Convenience constructor for an empty context. Used by the
    /// streaming layer as the default before filling any known field.
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder-style setter: attach the request digest.
    pub fn with_request_digest(mut self, digest: impl Into<String>) -> Self {
        self.request_digest = Some(digest.into());
        self
    }

    /// Builder-style setter: attach the response digest.
    pub fn with_response_digest(mut self, digest: impl Into<String>) -> Self {
        self.response_digest = Some(digest.into());
        self
    }

    /// Builder-style setter: attach the underlying cause string.
    pub fn with_cause(mut self, cause: impl Into<String>) -> Self {
        self.cause = Some(cause.into());
        self
    }

    /// Builder-style setter: attach the stage label.
    pub fn with_stage(mut self, stage: impl Into<String>) -> Self {
        self.stage = Some(stage.into());
        self
    }

    /// Builder-style setter: attach the upstream request id.
    pub fn with_upstream_request_id(mut self, id: impl Into<String>) -> Self {
        self.upstream_request_id = Some(id.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_context_serializes_to_null() {
        // Default has all fields None → serde_json::to_value gives `{}`.
        // (snake_case is implicit because the field names are already
        //  single-word; skip_serializing_if prunes the Nones.)
        let v = serde_json::to_value(ErrorContext::new()).unwrap();
        assert_eq!(v, serde_json::json!({}));
    }

    #[test]
    fn builders_set_fields() {
        let c = ErrorContext::new()
            .with_request_digest("model=gpt-4o")
            .with_cause("connect timeout")
            .with_stage("connect");
        assert_eq!(c.request_digest.as_deref(), Some("model=gpt-4o"));
        assert_eq!(c.cause.as_deref(), Some("connect timeout"));
        assert_eq!(c.stage.as_deref(), Some("connect"));
    }

    #[test]
    fn serializes_only_set_fields() {
        let c = ErrorContext::new().with_stage("parse_sse");
        let v = serde_json::to_value(&c).unwrap();
        let obj = v.as_object().unwrap();
        assert!(obj.contains_key("stage"));
        assert!(!obj.contains_key("request_digest"));
        assert!(!obj.contains_key("response_digest"));
        assert!(!obj.contains_key("cause"));
        assert!(!obj.contains_key("upstream_request_id"));
    }
}
