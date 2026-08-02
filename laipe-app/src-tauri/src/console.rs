//! Debug console — in-memory log buffer + Tauri event + commands.
//!
//! Pattern borrowed from PlotCraft's `console.rs` (AGENTS.md 硬规则 #1：结构对齐，代码自写).
//! Simplifications for laipe-starter:
//! - 3 levels (info / warn / error), 2 sources (backend / frontend)
//! - No automatic tracing hook (key error points call `console_log()` manually)
//! - In-memory only, max 1000 entries, FIFO truncation
//!
//! ## Diagnostic context (v0.2+)
//!
//! `ConsoleEntry` now carries LLM-debuggable context fields. All are
//! optional so existing call sites that just want a plain log line
//! don't have to fill them in. The chat command fills them when it
//! surfaces an error so the saved-error-report flow has everything
//! it needs in one round-trip.
//!
//! Data flow:
//! - Rust: `console::console_log(app, "error", "llm", msg)` → push + emit `console:entry`
//! - Frontend: `listen<ConsoleEntry>("console:entry", ...)` accumulates
//! - Frontend init: `invoke("get_console_entries")` for snapshot
//! - UI: `useConsoleEntries()` reactive list, filter / search / clear

use laipe_core::ChatErrorKind;
use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

/// One console log entry — frontend mirrors this shape exactly.
#[derive(Debug, Clone, Serialize)]
pub struct ConsoleEntry {
    /// Unique id (timestamp + counter, sufficient for in-memory dedup).
    pub id: String,
    /// Log level: "info" | "warn" | "error".
    pub level: String,
    /// Source: "backend" | "frontend".
    pub source: String,
    /// Module / category name (e.g. "llm", "settings", "app").
    pub module: String,
    /// The log message.
    pub message: String,
    /// Timestamp in milliseconds since epoch.
    pub timestamp_ms: i64,
    // === Diagnostic context (all optional; see `push_with_diag`) ===
    /// Conversation id the entry belongs to. `None` for app-wide logs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,
    /// Agent-loop turn (0-based). `None` for logs outside an agent turn.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn: Option<u32>,
    /// Typed error class. Drives the saved report's "Likely causes"
    /// section via `ChatErrorKind::to_debug_hint()`. `None` for non-error
    /// entries or errors that don't fit the 8-way taxonomy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// One-line summary of the outgoing request. Mirrors what the
    /// diagnostic recorder's `meta.json` has. Saved report includes
    /// the full body; this is the in-memory digest.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_digest: Option<String>,
    /// One-line summary of the incoming response (or error). The saved
    /// report's `response.bin` is the full raw bytes; this is the digest.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_digest: Option<String>,
    /// The original lower-level error string (reqwest / serde / etc.)
    /// the streaming layer classified. Useful for stack-style reading.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,
}

/// Tauri-managed in-memory console buffer.
pub struct ConsoleState {
    entries: Mutex<Vec<ConsoleEntry>>,
    max_entries: usize,
    counter: Mutex<u64>,
}

impl Default for ConsoleState {
    fn default() -> Self {
        Self::new()
    }
}

impl ConsoleState {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(Vec::new()),
            max_entries: 1000,
            counter: Mutex::new(0),
        }
    }

    /// Push a backend entry. Newest at index 0; FIFO truncation; emit event.
    pub fn push(&self, app: &AppHandle, level: &str, module: &str, message: impl Into<String>) {
        self.push_with_diag(app, level, module, message, ConsoleDiag::default());
    }

    /// Push a backend entry with diagnostic context. All `ConsoleDiag`
    /// fields are optional; the empty `ConsoleDiag` is the same as `push`.
    pub fn push_with_diag(
        &self,
        app: &AppHandle,
        level: &str,
        module: &str,
        message: impl Into<String>,
        diag: ConsoleDiag,
    ) {
        let id = {
            let mut c = self.counter.lock().expect("console counter mutex poisoned");
            *c += 1;
            format!("console-{}-{}", chrono::Utc::now().timestamp_millis(), *c)
        };
        let entry = ConsoleEntry {
            id,
            level: level.to_string(),
            source: "backend".to_string(),
            module: module.to_string(),
            message: message.into(),
            timestamp_ms: chrono::Utc::now().timestamp_millis(),
            conversation_id: diag.conversation_id,
            turn: diag.turn,
            kind: diag.kind.map(|k| k.as_str().to_string()),
            request_digest: diag.request_digest,
            response_digest: diag.response_digest,
            cause: diag.cause,
        };
        {
            let mut entries = self.entries.lock().expect("console entries mutex poisoned");
            entries.insert(0, entry.clone());
            if entries.len() > self.max_entries {
                entries.truncate(self.max_entries);
            }
        }
        // Emit — failure is non-fatal (Tauri runtime may not be up yet).
        let _ = app.emit("console:entry", &entry);
    }

    pub fn snapshot(&self) -> Vec<ConsoleEntry> {
        self.entries
            .lock()
            .expect("console entries mutex poisoned")
            .clone()
    }

    pub fn clear(&self) {
        self.entries
            .lock()
            .expect("console entries mutex poisoned")
            .clear();
    }
}

/// Optional diagnostic context for a `ConsoleEntry`. Builder-style
/// construction keeps call sites readable:
///
/// ```ignore
/// ConsoleDiag::new()
///     .with_conversation_id(conv_id)
///     .with_turn(turn)
///     .with_kind(ChatErrorKind::Auth)
///     .with_request_digest("model=gpt-4o format=openai_chat")
///     .with_response_digest("status=401 content-type=application/json")
///     .with_cause("HTTP status client error (401) for url (...)")
/// ```
#[derive(Debug, Clone, Default)]
pub struct ConsoleDiag {
    pub conversation_id: Option<String>,
    pub turn: Option<u32>,
    pub kind: Option<ChatErrorKind>,
    pub request_digest: Option<String>,
    pub response_digest: Option<String>,
    pub cause: Option<String>,
}

impl ConsoleDiag {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_conversation_id(mut self, id: impl Into<String>) -> Self {
        self.conversation_id = Some(id.into());
        self
    }

    pub fn with_turn(mut self, turn: u32) -> Self {
        self.turn = Some(turn);
        self
    }

    pub fn with_kind(mut self, kind: ChatErrorKind) -> Self {
        self.kind = Some(kind);
        self
    }

    pub fn with_request_digest(mut self, digest: impl Into<String>) -> Self {
        self.request_digest = Some(digest.into());
        self
    }

    pub fn with_response_digest(mut self, digest: impl Into<String>) -> Self {
        self.response_digest = Some(digest.into());
        self
    }

    pub fn with_cause(mut self, cause: impl Into<String>) -> Self {
        self.cause = Some(cause.into());
        self
    }
}

/// Helper for any Rust code that wants to log to the console.
/// Resolves `ConsoleState` from the `AppHandle` and pushes an entry.
///
/// Usage:
/// ```ignore
/// console::log(&app, "error", "llm", format!("start_chat failed: {e}"));
/// ```
pub fn log(app: &AppHandle, level: &str, module: &str, message: impl Into<String>) {
    let state = app.state::<ConsoleState>();
    state.push(app, level, module, message);
}

/// Helper for diagnostic-context pushes. Same as `log` but with `ConsoleDiag`.
///
/// Usage:
/// ```ignore
/// console::log_with_diag(
///     &app, "error", "llm", format!("..."),
///     ConsoleDiag::new()
///         .with_kind(ChatErrorKind::Auth)
///         .with_request_digest(format!("model={}", cfg.model)),
/// );
/// ```
pub fn log_with_diag(
    app: &AppHandle,
    level: &str,
    module: &str,
    message: impl Into<String>,
    diag: ConsoleDiag,
) {
    let state = app.state::<ConsoleState>();
    state.push_with_diag(app, level, module, message, diag);
}

/// Tauri command: pull a full console snapshot (frontend first open).
#[tauri::command]
pub fn get_console_entries(state: tauri::State<ConsoleState>) -> Vec<ConsoleEntry> {
    state.snapshot()
}

/// Tauri command: clear all console entries.
#[tauri::command]
pub fn clear_console(state: tauri::State<ConsoleState>) {
    state.clear();
}

/// Tauri command: look up a single entry by id. Used by the
/// `dump_error_report` flow so the report can correlate the saved
/// recording with the console entry that triggered the save.
#[tauri::command]
pub fn get_console_entry_by_id(
    state: tauri::State<ConsoleState>,
    id: String,
) -> Option<ConsoleEntry> {
    state.snapshot().into_iter().find(|e| e.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn console_diag_builder() {
        let d = ConsoleDiag::new()
            .with_conversation_id("conv-1")
            .with_turn(2)
            .with_kind(ChatErrorKind::Auth)
            .with_request_digest("model=gpt-4o")
            .with_response_digest("status=401")
            .with_cause("HTTP 401");
        assert_eq!(d.conversation_id.as_deref(), Some("conv-1"));
        assert_eq!(d.turn, Some(2));
        assert_eq!(d.kind, Some(ChatErrorKind::Auth));
        assert_eq!(d.request_digest.as_deref(), Some("model=gpt-4o"));
        assert_eq!(d.response_digest.as_deref(), Some("status=401"));
        assert_eq!(d.cause.as_deref(), Some("HTTP 401"));
    }

    #[test]
    fn empty_diag_is_default() {
        let d = ConsoleDiag::default();
        assert!(d.conversation_id.is_none());
        assert!(d.turn.is_none());
        assert!(d.kind.is_none());
        assert!(d.request_digest.is_none());
        assert!(d.response_digest.is_none());
        assert!(d.cause.is_none());
    }
}
