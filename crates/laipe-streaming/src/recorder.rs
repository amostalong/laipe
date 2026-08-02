//! LLM-debuggable diagnostic recording for streaming chat requests.
//!
//! The streaming layer is the **only** place that sees the raw HTTP request
//! body before serialization and the raw response bytes as they arrive.
//! That makes it the natural place to capture diagnostics that the higher
//! layers (Tauri commands, console UI) can't reconstruct on their own.
//!
//! ## What gets recorded
//!
//! For one chat round-trip, the protocol implementation calls:
//!
//! 1. `record_request(ctx, body)` — once, before the HTTP POST. `body` is
//!    the **exact bytes** the streaming layer sent (auth header redaction
//!    is the caller's responsibility; see `redact_request_bytes`).
//! 2. `record_response_chunk(ctx, chunk)` — zero or more times, once per
//!    network-level byte chunk. Chunks may be partial SSE frames; the
//!    recorder must buffer them only if it cares about frame boundaries
//!    (the default `FileRecorder` does not — it just dumps raw bytes).
//! 3. `record_completion(ctx, outcome)` — exactly once, after the stream
//!    ends (Done / Error / cancelled / pre-stream failure). No further
//!    callbacks for this `ctx` happen after this call.
//!
//! `ctx.id` ties the three callbacks together. The protocol implementation
//! generates the id (a UUID v4 is fine) and threads it through the spawn.
//!
//! ## Pluggability (global design principle)
//!
//! The trait is the seam. `NullRecorder` is the default no-op; `FileRecorder`
//! is the default disk-writing impl. Apps that want to forward errors to
//! Sentry / their own backend implement `DiagnosticRecorder` and pass it
//! to `pick(fmt).dispatch(cfg, messages, tools, recorder)`.
//!
//! Consumers that don't care about diagnostics use `NullRecorder` (or
//! pass `None` and the streaming layer falls back to the null impl) and
//! pay zero runtime cost — `record_*` are async no-ops returning
//! immediately, and the compiler will inline the trait dispatch.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use laipe_core::error::ChatErrorKind;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

/// What terminated a chat round. Sent on the final `record_completion`
/// call for a given `ctx.id`. The streaming layer never re-uses an id
/// after this.
#[derive(Debug, Clone)]
pub enum CompletionOutcome {
    /// Stream ended cleanly on a `[DONE]` / `message_stop` / equivalent.
    Done {
        /// Number of text-delta events emitted.
        text_events: u32,
        /// Number of tool-call partial events emitted.
        tool_call_events: u32,
    },
    /// Stream ended on `StreamEvent::Error` (mid-stream).
    Error {
        kind: ChatErrorKind,
        /// The user-facing message the streaming layer emitted.
        message: String,
    },
    /// Pre-stream failure: the HTTP request never opened, or the
    /// upstream returned a non-2xx that the streaming layer classified
    /// as a `StreamError::Upstream`. The protocol impl records this
    /// even though the response is `Err` from the caller's POV.
    PreStreamFailure {
        kind: ChatErrorKind,
        /// The streaming-layer error string (e.g. "upstream returned 429: ...").
        message: String,
    },
    /// Consumer dropped the `Receiver` or the `CancelHandle` fired.
    Cancelled,
}

impl CompletionOutcome {
    /// One-line label suitable for log lines and the saved report.
    pub fn label(&self) -> String {
        match self {
            Self::Done {
                text_events,
                tool_call_events,
            } => {
                format!("done (text={text_events}, tools={tool_call_events})")
            }
            Self::Error { kind, .. } => format!("error ({})", kind.as_str()),
            Self::PreStreamFailure { kind, .. } => {
                format!("pre-stream failure ({})", kind.as_str())
            }
            Self::Cancelled => "cancelled".to_string(),
        }
    }
}

/// Per-round context. One per `dispatch()` call. The streaming layer
/// constructs this once and threads the id through the `tokio::spawn`
/// that drains the byte stream.
#[derive(Debug, Clone)]
pub struct RecordingContext {
    /// Unique id. UUID v4 is fine. Strings only (no UUID dep) so the
    /// recorder can use it in file paths.
    pub id: String,
    /// Wall-clock time the dispatch started. Set by the streaming layer.
    pub started_at: DateTime<Utc>,
    /// Which protocol. Drives the report's section headers.
    pub api_format: &'static str,
    /// Compact request metadata. Mirrors `ProviderConfig` fields the
    /// user / LLM cares about.
    pub model: String,
    pub endpoint: String,
    /// Conversation id from the app layer, if the caller passed one.
    /// `None` when the streaming layer is called without a `conv_id`
    /// (e.g. tests, CLI examples).
    pub conversation_id: Option<String>,
    /// Agent-loop turn (0-based). Lets the report group multiple
    /// streamings under the same logical user turn.
    pub turn: Option<u32>,
}

/// Pluggable diagnostic recorder.
///
/// All three methods are async so a future Sentry / OpenTelemetry impl
/// can do network I/O without blocking the protocol's `tokio::spawn`.
/// The default `NullRecorder` and `FileRecorder` impls do not block.
#[async_trait]
pub trait DiagnosticRecorder: Send + Sync {
    /// Called once per dispatch, before the HTTP POST.
    async fn record_request(&self, ctx: &RecordingContext, body: &[u8]);

    /// Called for each network-level byte chunk as it arrives from
    /// the wire. May be called many times. Chunks may split SSE frames.
    async fn record_response_chunk(&self, ctx: &RecordingContext, chunk: &[u8]);

    /// Called exactly once per dispatch, when the round is over.
    /// The recorder may flush / close any per-id resources here.
    async fn record_completion(&self, ctx: &RecordingContext, outcome: &CompletionOutcome);
}

// ---------------------------------------------------------------------------
// NullRecorder
// ---------------------------------------------------------------------------

/// No-op recorder. The default when the caller doesn't want diagnostics.
/// All methods are zero-cost async no-ops.
pub struct NullRecorder;

#[async_trait]
impl DiagnosticRecorder for NullRecorder {
    async fn record_request(&self, _ctx: &RecordingContext, _body: &[u8]) {}
    async fn record_response_chunk(&self, _ctx: &RecordingContext, _chunk: &[u8]) {}
    async fn record_completion(&self, _ctx: &RecordingContext, _outcome: &CompletionOutcome) {}
}

// ---------------------------------------------------------------------------
// FileRecorder
// ---------------------------------------------------------------------------

/// Per-recording directory layout:
///
/// ```text
/// {root}/<ctx.id>/
///   request.json     — exact request bytes (post-redaction)
///   response.bin     — concatenated response chunks, raw
///   meta.json        — ctx + outcome (one JSON object)
/// ```
///
/// The directory is created on the first `record_request` call. The
/// `record_completion` call writes `meta.json` and marks the recording
/// as closed (in-memory state).
///
/// Size cap: `max_response_bytes` (default 5 MiB) per recording.
/// Chunks beyond the cap are dropped and a `truncated: true` flag is
/// set in `meta.json`.
///
/// API key redaction: the caller is expected to call
/// `redact_request_bytes()` before passing the request bytes to
/// `record_request`. The recorder itself does not rewrite the body —
/// that keeps the trait contract simple ("you give me bytes, I write
/// them").
pub struct FileRecorder {
    root: PathBuf,
    max_response_bytes: usize,
    /// Open recordings, keyed by `ctx.id`. The recorder's `record_*`
    /// methods are called sequentially per ctx (the protocol impl
    /// doesn't await two callbacks for the same id concurrently),
    /// so a plain `Mutex<HashMap>` is sufficient. The `tokio::sync::Mutex`
    /// is used so `.lock().await` works inside the async methods
    /// without `blocking_lock` warnings.
    open: Arc<Mutex<std::collections::HashMap<String, OpenRecording>>>,
}

struct OpenRecording {
    dir: PathBuf,
    written_response_bytes: usize,
    truncated: bool,
    meta: MetaOnDisk,
}

#[derive(Debug, Clone, serde::Serialize)]
struct MetaOnDisk {
    id: String,
    started_at: String,
    api_format: String,
    model: String,
    endpoint: String,
    conversation_id: Option<String>,
    turn: Option<u32>,
    request_bytes: usize,
    response_bytes: usize,
    truncated: bool,
    outcome: String,
}

impl FileRecorder {
    /// Create a `FileRecorder` rooted at `root`. The directory is
    /// created lazily on first `record_request`. The caller is
    /// expected to pass `app.path().app_log_dir()` (Tauri 2) or
    /// any other absolute path; relative paths are accepted but
    /// discouraged (the working dir may differ between Tauri dev
    /// and release).
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self::with_max(root, DEFAULT_MAX_RESPONSE_BYTES)
    }

    /// Same as `new`, but with a custom response size cap.
    /// Use a smaller cap if disk space is tight; a larger one if
    /// you're debugging a model that streams a 50MB tool-call
    /// response.
    pub fn with_max(root: impl Into<PathBuf>, max_response_bytes: usize) -> Self {
        Self {
            root: root.into(),
            max_response_bytes,
            open: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    /// Test helper. Returns the directory a given recording lives in,
    /// or `None` if the recorder has no open / completed recording for
    /// that id (i.e. the streaming layer never called any callback for
    /// this id, or the file was already cleaned up).
    pub async fn recording_dir(&self, id: &str) -> Option<PathBuf> {
        let g = self.open.lock().await;
        g.get(id).map(|o| o.dir.clone())
    }
}

const DEFAULT_MAX_RESPONSE_BYTES: usize = 5 * 1024 * 1024; // 5 MiB

#[async_trait]
impl DiagnosticRecorder for FileRecorder {
    async fn record_request(&self, ctx: &RecordingContext, body: &[u8]) {
        let dir = self.root.join(&ctx.id);
        // create_dir_all is cheap when the dir already exists.
        // We do it synchronously inside the async fn — the call
        // is fast on warm caches and the streaming layer is
        // already on a worker thread that can absorb a one-shot
        // mkdir. (The alternative is `tokio::fs::create_dir_all`,
        // which adds IO to the runtime for no real benefit here.)
        if let Err(e) = std::fs::create_dir_all(&dir) {
            tracing::warn!(
                target: "laipe.diagnostics",
                id = %ctx.id,
                error = %e,
                "failed to create recording dir; recording dropped",
            );
            return;
        }
        let req_path = dir.join("request.json");
        if let Err(e) = std::fs::write(&req_path, body) {
            tracing::warn!(
                target: "laipe.diagnostics",
                id = %ctx.id,
                error = %e,
                "failed to write request.json; recording dropped",
            );
            return;
        }
        let meta = MetaOnDisk {
            id: ctx.id.clone(),
            started_at: ctx.started_at.to_rfc3339(),
            api_format: ctx.api_format.to_string(),
            model: ctx.model.clone(),
            endpoint: ctx.endpoint.clone(),
            conversation_id: ctx.conversation_id.clone(),
            turn: ctx.turn,
            request_bytes: body.len(),
            response_bytes: 0,
            truncated: false,
            outcome: "in_progress".to_string(),
        };
        let mut g = self.open.lock().await;
        g.insert(
            ctx.id.clone(),
            OpenRecording {
                dir,
                written_response_bytes: 0,
                truncated: false,
                meta,
            },
        );
    }

    async fn record_response_chunk(&self, ctx: &RecordingContext, chunk: &[u8]) {
        // Fast path: if we already hit the cap, drop.
        let mut g = self.open.lock().await;
        let Some(open) = g.get_mut(&ctx.id) else {
            // record_request was never called, or already completed.
            return;
        };
        if open.truncated {
            return;
        }
        let remaining = self
            .max_response_bytes
            .saturating_sub(open.written_response_bytes);
        if remaining == 0 {
            open.truncated = true;
            return;
        }
        let to_write = if chunk.len() <= remaining {
            chunk
        } else {
            open.truncated = true;
            &chunk[..remaining]
        };
        let resp_path = open.dir.join("response.bin");
        // Append. We use std::fs::OpenOptions so we can grow the file
        // chunk-by-chunk without holding the whole response in memory.
        // This is on the async fn path but we don't block the tokio
        // worker pool with it (the chunks are small and the disk is
        // fast for the 5MB cap); a contended test suite might want to
        // move this to spawn_blocking. In practice the streaming
        // layer's `record_response_chunk` is the only writer and
        // nothing else is awaiting the open-state lock.
        let res = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&resp_path)
            .and_then(|mut f| std::io::Write::write_all(&mut f, to_write));
        if let Err(e) = res {
            tracing::warn!(
                target: "laipe.diagnostics",
                id = %ctx.id,
                error = %e,
                "failed to append response.bin; recording dropped",
            );
            g.remove(&ctx.id);
            return;
        }
        open.written_response_bytes += to_write.len();
    }

    async fn record_completion(&self, ctx: &RecordingContext, outcome: &CompletionOutcome) {
        let mut g = self.open.lock().await;
        let Some(mut open) = g.remove(&ctx.id) else {
            return;
        };
        open.meta.response_bytes = open.written_response_bytes;
        open.meta.truncated = open.truncated;
        open.meta.outcome = outcome.label();
        let meta_path = open.dir.join("meta.json");
        let bytes = match serde_json::to_vec_pretty(&open.meta) {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!(
                    target: "laipe.diagnostics",
                    id = %ctx.id,
                    error = %e,
                    "failed to serialize meta.json",
                );
                return;
            }
        };
        if let Err(e) = std::fs::write(&meta_path, &bytes) {
            tracing::warn!(
                target: "laipe.diagnostics",
                id = %ctx.id,
                error = %e,
                "failed to write meta.json",
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Redaction helper
// ---------------------------------------------------------------------------

/// Redact common API-key / secret patterns from a request body **before**
/// the caller hands it to `record_request`. Conservative: matches the
/// `Authorization: Bearer ...` and `x-api-key: ...` headers (case-insensitive)
/// plus the JSON `"api_key": "..."` field. Returns the redacted bytes.
///
/// This is a best-effort scrub. If a key is concatenated into a free-form
/// field (e.g. a system prompt saying "use key sk-xxx"), the scrubber will
/// miss it. The recorder's saved report is a **dev artifact** — users
/// should review it before sharing.
pub fn redact_request_bytes(body: &[u8]) -> Vec<u8> {
    let text = match std::str::from_utf8(body) {
        Ok(s) => s,
        Err(_) => return body.to_vec(), // non-UTF-8 body (binary) — return as-is
    };
    let mut out = text.to_string();

    // 1) HTTP header / JSON-keyed form: "Authorization: Bearer <value>"
    //    and "x-api-key: <value>". We handle both by scanning for a
    //    colon after the name and replacing the value.
    out = redact_header_field(&out, "authorization", /*has_bearer_scheme=*/ true);
    out = redact_header_field(&out, "x-api-key", /*has_bearer_scheme=*/ false);

    // 2) JSON string field: "api_key": "<value>"  — handled separately
    //    because the field/value is JSON-string-quoted, not header-form.
    out = redact_json_string_field(&out, "api_key");

    out.into_bytes()
}

/// Find each occurrence of `<header>` (case-insensitive, word-boundary) and
/// replace the **value** that follows the next `:` with `REDACTED`. If
/// `has_bearer_scheme` is true, an optional `Bearer` (case-insensitive) +
/// whitespace prefix between the colon and the value is preserved (so the
/// redacted text remains syntactically valid for the consumer reading it).
fn redact_header_field(text: &str, header: &str, has_bearer_scheme: bool) -> String {
    let bytes = text.as_bytes();
    let lower = text.to_ascii_lowercase();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < bytes.len() {
        let needle = header.to_ascii_lowercase();
        let Some(rel) = lower[i..].find(&needle) else {
            out.push_str(&text[i..]);
            break;
        };
        let abs = i + rel;
        // Word boundary: char before must not be `[A-Za-z0-9_-]`, char
        // after must not be `[A-Za-z0-9_-]`. (JSON form: `"Authorization"`
        // — the preceding char is `"`, the following char is `"`; both
        // pass the boundary check.)
        let prev_ok = abs == 0 || !is_word_char(bytes[abs - 1]);
        let after = abs + header.len();
        let next_ok = after >= bytes.len() || !is_word_char(bytes[after]);
        if !(prev_ok && next_ok) {
            // Not a real match — copy one char and advance.
            out.push(bytes[i] as char);
            i += 1;
            continue;
        }
        // Copy everything up to and including the header name.
        out.push_str(&text[i..after]);
        // Now scan forward for the `:` that separates the field from
        // its value. Allow up to ~64 bytes of header noise (closing quote
        // in JSON, whitespace, etc.).
        let mut j = after;
        let mut saw_colon = false;
        while j < bytes.len() && j - after < 64 {
            let b = bytes[j];
            if b == b':' {
                saw_colon = true;
                j += 1;
                break;
            }
            // Skip JSON quotes / whitespace as we scan.
            if b == b'"' || b == b' ' || b == b'\t' {
                j += 1;
                continue;
            }
            // Hit something else (e.g. another field name) — not our match.
            break;
        }
        if !saw_colon {
            // No value to redact; this wasn't a real field. Advance past
            // the match and keep going.
            i = after;
            continue;
        }
        // Copy the `:` and any intervening JSON quote.
        out.push(':');
        // Skip optional whitespace and an optional `"`.
        while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t' || bytes[j] == b'"') {
            if bytes[j] == b'"' {
                out.push('"');
            }
            j += 1;
        }
        if has_bearer_scheme {
            // Optional "Bearer " prefix — case-insensitive match. We
            // preserve the prefix so the redacted output is still
            // readable to a human.
            let bearer = "bearer";
            if lower[j..].starts_with(bearer) {
                out.push_str("Bearer ");
                j += bearer.len();
                // Skip one separator (space or tab).
                if j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
                    j += 1;
                }
            }
        }
        // Copy everything in the value verbatim until we hit a value
        // terminator. The terminator is any of `,` `}` `]` `;` `"` `\n`
        // or a closing JSON quote (handled by `"` in the set).
        let value_start = j;
        while j < bytes.len() {
            let b = bytes[j];
            if b == b','
                || b == b'}'
                || b == b']'
                || b == b';'
                || b == b'"'
                || b == b'\n'
                || b == b'\r'
            {
                break;
            }
            j += 1;
        }
        // If the value was non-empty, replace it with REDACTED.
        if j > value_start {
            out.push_str("REDACTED");
        } else {
            // Empty value — copy as-is (nothing to redact).
            out.push_str(&text[value_start..j]);
        }
        i = j;
    }
    out
}

fn is_word_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'-' || b == b'_'
}

fn redact_json_string_field(text: &str, field: &str) -> String {
    // Match `"<field>": "VALUE"` (any whitespace around `:`) and replace
    // VALUE. The whole match is quoted, so the redaction is straightforward.
    let needle = format!("\"{field}\"");
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let lower = text.to_ascii_lowercase();
    while i < text.len() {
        let Some(rel) = lower[i..].find(&needle) else {
            out.push_str(&text[i..]);
            break;
        };
        let abs = i + rel;
        out.push_str(&text[i..abs + needle.len()]);
        let mut j = abs + needle.len();
        // Skip whitespace + colon + optional whitespace + opening quote.
        while j < text.len()
            && (text.as_bytes()[j] == b' '
                || text.as_bytes()[j] == b'\t'
                || text.as_bytes()[j] == b':')
        {
            out.push(text.as_bytes()[j] as char);
            j += 1;
        }
        if j >= text.len() || text.as_bytes()[j] != b'"' {
            // Not a string field. Advance past the needle and continue.
            i = abs + needle.len();
            continue;
        }
        out.push('"');
        j += 1;
        // Scan to closing quote, handling backslash escapes.
        let value_start = j;
        while j < text.len() {
            let b = text.as_bytes()[j];
            if b == b'\\' && j + 1 < text.len() {
                j += 2;
                continue;
            }
            if b == b'"' {
                break;
            }
            j += 1;
        }
        if j > value_start {
            out.push_str("REDACTED");
        }
        if j < text.len() {
            out.push('"');
            j += 1;
        }
        i = j;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(id: &str) -> RecordingContext {
        RecordingContext {
            id: id.to_string(),
            started_at: Utc::now(),
            api_format: "openai_chat",
            model: "gpt-4o".to_string(),
            endpoint: "https://api.openai.com/v1".to_string(),
            conversation_id: Some("conv-1".to_string()),
            turn: Some(0),
        }
    }

    #[tokio::test]
    async fn null_recorder_is_zero_cost() {
        let r = NullRecorder;
        r.record_request(&ctx("a"), b"{}").await;
        r.record_response_chunk(&ctx("a"), b"data: 1\n\n").await;
        r.record_completion(
            &ctx("a"),
            &CompletionOutcome::Done {
                text_events: 1,
                tool_call_events: 0,
            },
        )
        .await;
        // Nothing to assert — the test is "does it not panic".
    }

    #[tokio::test]
    async fn file_recorder_writes_three_files() {
        let dir = tempdir_like();
        let r = FileRecorder::new(&dir);
        let id = "rec-1";
        r.record_request(&ctx(id), br#"{"model":"gpt-4o"}"#).await;
        r.record_response_chunk(&ctx(id), b"data: 1\n\n").await;
        r.record_response_chunk(&ctx(id), b"data: 2\n\n").await;
        r.record_completion(
            &ctx(id),
            &CompletionOutcome::Done {
                text_events: 2,
                tool_call_events: 0,
            },
        )
        .await;

        let rec_dir = dir.join(id);
        assert!(rec_dir.join("request.json").exists());
        assert!(rec_dir.join("response.bin").exists());
        assert!(rec_dir.join("meta.json").exists());

        let meta = std::fs::read_to_string(rec_dir.join("meta.json")).unwrap();
        assert!(meta.contains("\"outcome\": \"done (text=2, tools=0)\""));
        // Two chunks of `b"data: 1\n\n"` and `b"data: 2\n\n"` = 18 bytes total.
        assert!(meta.contains("\"response_bytes\": 18"));
    }

    #[tokio::test]
    async fn file_recorder_truncates_response_at_cap() {
        let dir = tempdir_like();
        let r = FileRecorder::with_max(&dir, 8);
        let id = "rec-2";
        r.record_request(&ctx(id), b"{}").await;
        r.record_response_chunk(&ctx(id), b"0123456789").await; // 10 bytes, cap 8
        r.record_completion(
            &ctx(id),
            &CompletionOutcome::Done {
                text_events: 0,
                tool_call_events: 0,
            },
        )
        .await;
        let body = std::fs::read(dir.join(id).join("response.bin")).unwrap();
        assert_eq!(body, b"01234567");
        let meta = std::fs::read_to_string(dir.join(id).join("meta.json")).unwrap();
        assert!(meta.contains("\"truncated\": true"));
        assert!(meta.contains("\"response_bytes\": 8"));
    }

    #[tokio::test]
    async fn file_recorder_ignores_chunks_after_completion() {
        let dir = tempdir_like();
        let r = FileRecorder::new(&dir);
        let id = "rec-3";
        r.record_request(&ctx(id), b"{}").await;
        r.record_completion(&ctx(id), &CompletionOutcome::Cancelled)
            .await;
        // Late chunk — must not panic, must not resurrect the recording.
        r.record_response_chunk(&ctx(id), b"late").await;
        assert!(r.recording_dir(id).await.is_none());
    }

    #[test]
    fn redaction_strips_bearer() {
        let body = br#"{"model":"gpt-4o","headers":{"Authorization":"Bearer sk-abc123"}}"#;
        let r = redact_request_bytes(body);
        let s = String::from_utf8(r).unwrap();
        assert!(s.contains("REDACTED"), "{s}");
        assert!(!s.contains("sk-abc123"), "{s}");
    }

    #[test]
    fn redaction_strips_x_api_key() {
        let body = br#"{"headers":{"x-api-key":"sk-ant-abc123"}}"#;
        let r = redact_request_bytes(body);
        let s = String::from_utf8(r).unwrap();
        assert!(s.contains("REDACTED"));
        assert!(!s.contains("sk-ant-abc123"));
    }

    #[test]
    fn redaction_strips_json_api_key_field() {
        let body = br#"{"api_key": "sk-abc123", "model": "gpt-4o"}"#;
        let r = redact_request_bytes(body);
        let s = String::from_utf8(r).unwrap();
        assert!(s.contains("\"api_key\": \"REDACTED\""), "{s}");
        assert!(!s.contains("sk-abc123"), "{s}");
        // Non-secret fields are left alone.
        assert!(s.contains("\"model\": \"gpt-4o\""), "{s}");
    }

    /// Build a unique temp dir under the OS temp dir. Returns a
    /// `PathBuf` that the caller should `remove_dir_all` after the test
    /// (we don't auto-clean — keeps the test code simple and lets you
    /// inspect artifacts on failure).
    fn tempdir_like() -> PathBuf {
        let mut p = std::env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        p.push(format!("laipe-recorder-test-{nanos}"));
        std::fs::create_dir_all(&p).unwrap();
        p
    }
}
