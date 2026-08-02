//! LLM-debuggable diagnostic subsystem for laipe-app.
//!
//! Owns the per-app `FileRecorder`, the saved-error-report writer, the
//! auto-snapshot index (`INDEX.jsonl`), and the `README-FOR-LLM.md`
//! generator. The chat command holds a `DiagnosticsState` and asks
//! it to record, snapshot, and dump reports.
//!
//! ## Lifecycle
//!
//! 1. App start: `DiagnosticsState::default()` is registered with
//!    Tauri's `manage` (a placeholder), then `setup` calls
//!    `state.initialize(&app_handle)` to swap in the real
//!    `FileRecorder` rooted at `<app_log_dir>/recordings/`. README
//!    is generated on first launch.
//! 2. Each chat turn: the chat command constructs a `RecordingContext`
//!    (with `conversation_id` and `turn`) and hands it to the recorder
//!    via `pick(fmt).dispatch(cfg, messages, tools, recorder, ctx)`.
//! 3. On error (pre-stream or mid-stream): the chat command calls
//!    `snapshot_error(...)`, which synthesizes a self-contained `.md`
//!    report at `<log_dir>/reports/<ts>-<id>.md` and appends a
//!    one-line entry to `INDEX.jsonl`.
//! 4. On user click ("Save report" in the console): the UI calls the
//!    `dump_error_report` command, which runs the same synthesis but
//!    on demand (no prior snapshot required — the recording dir is
//!    still there from step 2 even if step 3 didn't fire).
//!
//! ## Why a single module
//!
//! The Tauri-managed state (`DiagnosticsState`) owns one `FileRecorder`
//! and one `RwLock<Inner>`. The chat command holds a `State` for the
//! lifetime of the app and reads through it. Splitting into multiple
//! files would just add `mod foo; mod bar;` noise for no benefit at
//! this size.

use laipe_core::{ChatErrorKind, ErrorContext};
use laipe_streaming::FileRecorder;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, Manager, Runtime};

/// User-facing toggles. Persisted by the Tauri `set_diagnostic_mode`
/// command. The defaults are conservative: auto-snapshot is OFF
/// (the user has to opt in), and per-error report size is bounded.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticConfig {
    /// If `true`, every error auto-snapshots a `.md` report. The
    /// in-memory console is unaffected; this only controls whether a
    /// report file is also written to disk.
    pub auto_snapshot: bool,
    /// Maximum bytes per saved report (truncates the raw response
    /// beyond this). 5 MiB by default.
    pub max_report_bytes: usize,
    /// If `true`, every chat turn writes a full on-disk recording
    /// (request + raw response) even when the round succeeds. OFF
    /// by default.
    pub record_successful_rounds: bool,
}

impl Default for DiagnosticConfig {
    fn default() -> Self {
        Self {
            auto_snapshot: false,
            max_report_bytes: 5 * 1024 * 1024,
            record_successful_rounds: false,
        }
    }
}

/// One line per saved report in `<log_dir>/reports/INDEX.jsonl`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexEntry {
    pub id: String,
    pub ts: String,
    pub conversation_id: Option<String>,
    pub turn: Option<u32>,
    pub kind: Option<String>,
    pub model: Option<String>,
    pub api_format: Option<String>,
    pub endpoint: Option<String>,
    pub message: String,
    pub report_path: String,
}

/// Tauri-managed state. The public API is a thin shell; the mutable
/// fields live in an inner `RwLock` so we can swap them out at
/// `setup` time (Tauri's `manage()` doesn't give you an `AppHandle`,
/// but `setup()` does).
pub struct DiagnosticsState {
    inner: Arc<tokio::sync::RwLock<Inner>>,
}

pub(crate) struct Inner {
    /// The active recorder. Swapped to a `FileRecorder` rooted at
    /// `app_log_dir/recordings` after `initialize`. Before that, it's
    /// a fallback `FileRecorder` rooted at a temp dir (so the chat
    /// command always has *something* to pass to `dispatch`).
    recorder: Arc<FileRecorder>,
    /// The on-disk root for reports + index + README. Set once at
    /// `initialize`.
    log_dir: PathBuf,
    /// Configurable toggles.
    config: DiagnosticConfig,
}

impl Default for DiagnosticsState {
    fn default() -> Self {
        // Used at `manage()` time, before `setup()` runs. The
        // recorder and log_dir are placeholders; setup replaces them.
        let placeholder_root = std::env::temp_dir().join("laipe-diagnostics-placeholder");
        let _ = std::fs::create_dir_all(&placeholder_root);
        Self {
            inner: Arc::new(tokio::sync::RwLock::new(Inner {
                recorder: Arc::new(FileRecorder::new(placeholder_root.join("recordings"))),
                log_dir: placeholder_root,
                config: DiagnosticConfig::default(),
            })),
        }
    }
}

impl DiagnosticsState {
    /// Resolve the per-app log dir (Tauri 2 `app_log_dir()`), create
    /// the `recordings/`, `reports/` subdirs, write
    /// `README-FOR-LLM.md` if missing, and construct a `FileRecorder`
    /// rooted at `recordings/`. Called from `setup`.
    ///
    /// Failures here are non-fatal — we log to `tracing::warn!` and
    /// leave the placeholder state. The app must still run even if
    /// the disk is full or read-only.
    #[allow(dead_code)]
    pub async fn initialize<R: Runtime>(&self, app: &AppHandle<R>) {
        let log_dir = match app.path().app_log_dir() {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(
                    target: "laipe.diagnostics",
                    error = %e,
                    "could not resolve app_log_dir; diagnostics will use temp fallback",
                );
                return;
            }
        };

        // Best-effort. Don't fail the app over this.
        for sub in ["recordings", "reports"] {
            let _ = std::fs::create_dir_all(log_dir.join(sub));
        }

        let recorder = Arc::new(FileRecorder::new(log_dir.join("recordings")));

        // README-FOR-LLM.md — the file the user's LLM assistant should
        // read first. Generated on first launch; updated when the
        // schema version changes (bump `README_SCHEMA_VERSION`).
        let _ = write_readme_for_llm(&log_dir);

        let mut g = self.inner.write().await;
        g.recorder = recorder;
        g.log_dir = log_dir;
    }

    /// Expose the inner `Arc` so it can be passed to
    /// `initialize_from_arc` (we can't pass the `tauri::State` itself
    /// across the `async move` boundary because its lifetime is tied
    /// to the `setup` call). Used only by `lib.rs` `setup`.
    pub fn shared(&self) -> Arc<tokio::sync::RwLock<Inner>> {
        self.inner.clone()
    }

    /// Same as [`Self::initialize`], but takes the inner `Arc` so it
    /// can be called from a `setup` closure where the `tauri::State`
    /// can't cross the async boundary. Avoids the `&self`-on-borrowed-
    /// state problem.
    pub async fn initialize_from_arc<R: Runtime>(
        inner: Arc<tokio::sync::RwLock<Inner>>,
        app: &AppHandle<R>,
    ) {
        let log_dir = match app.path().app_log_dir() {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(
                    target: "laipe.diagnostics",
                    error = %e,
                    "could not resolve app_log_dir; diagnostics will use temp fallback",
                );
                return;
            }
        };

        for sub in ["recordings", "reports"] {
            let _ = std::fs::create_dir_all(log_dir.join(sub));
        }

        let recorder = Arc::new(FileRecorder::new(log_dir.join("recordings")));
        let _ = write_readme_for_llm(&log_dir);

        let mut g = inner.write().await;
        g.recorder = recorder;
        g.log_dir = log_dir;
    }

    /// Clone the recorder for the chat command. Cheap (`Arc::clone`).
    pub async fn recorder(&self) -> Arc<FileRecorder> {
        self.inner.read().await.recorder.clone()
    }

    /// Current config snapshot. Cheap lock-and-copy.
    pub async fn config(&self) -> DiagnosticConfig {
        self.inner.read().await.config.clone()
    }

    /// Replace the config. Used by the Tauri command.
    pub async fn set_config(&self, new_cfg: DiagnosticConfig) {
        self.inner.write().await.config = new_cfg;
    }

    /// Root directory holding `reports/`, `recordings/`, `INDEX.jsonl`,
    /// `README-FOR-LLM.md`. Returned for the "Reveal in Explorer"
    /// button in the console UI.
    pub async fn log_dir(&self) -> PathBuf {
        self.inner.read().await.log_dir.clone()
    }

    /// Whether auto-snapshot is on.
    pub async fn auto_snapshot_enabled(&self) -> bool {
        self.inner.read().await.config.auto_snapshot
    }

    /// Build a `RecordingContext` for one chat round. The id is a
    /// timestamp + counter so it's monotonic and unique within an
    /// app session; the file path it produces is human-readable.
    pub fn new_context(
        &self,
        conversation_id: Option<String>,
        turn: Option<u32>,
        model: &str,
        endpoint: &str,
        api_format: &str,
    ) -> laipe_streaming::RecordingContext {
        use laipe_streaming::RecordingContext;
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        // Counter is best-effort uniqueness. Two contexts in the same
        // nanosecond is a real race; the file system will let the
        // second one overwrite the first's `meta.json` and we'll lose
        // the recording. Acceptable for v0.1 — the alternative is a
        // global atomic counter, which adds an Arc<AtomicU64> and
        // complexity for a vanishingly rare race.
        let id = format!(
            "rec-{}-{:x}",
            chrono::Utc::now().timestamp_millis(),
            nanos & 0xFFFF_FFFF
        );
        RecordingContext {
            id,
            started_at: chrono::Utc::now(),
            api_format: api_format_match(api_format),
            model: model.to_string(),
            endpoint: endpoint.to_string(),
            conversation_id,
            turn,
        }
    }
}

fn api_format_match(s: &str) -> &'static str {
    match s {
        "OpenAiChat" => "openai_chat",
        "OpenAiResponses" => "openai_responses",
        "Anthropic" => "anthropic",
        "openai_chat" => "openai_chat",
        "openai_responses" => "openai_responses",
        "anthropic" => "anthropic",
        _ => "unknown",
    }
}

// ---------------------------------------------------------------------------
// Auto-snapshot on error
// ---------------------------------------------------------------------------

/// Synthesize a self-contained `.md` report for one error and write
/// it to `<log_dir>/reports/<ts>-<id>.md`. Appends a one-line entry
/// to `INDEX.jsonl`.
///
/// `rec_id` is the `ctx.id` passed to the recorder (matches the
/// directory name under `<log_dir>/recordings/<rec_id>/`).
/// `ctx` carries the model/endpoint/conversation metadata.
/// `kind` + `message` + `cause` come from the chat command's error
/// handling. `conv_messages` is the conversation at the time of the
/// error — included verbatim in the report's "Conversation context"
/// section so the LLM can see what the user asked.
///
/// Returns the path to the written report (relative to the log dir).
//
// 8 args is over the project's `too-many-arguments-threshold = 7`.
// They're all distinct concerns (state, recording id, ctx metadata,
// error kind, message, cause, conversation snapshot, error context) and
// bundling them into a struct would just rename the same 8 fields.
// Lifting the threshold here keeps the call sites at the chat command
// readable.
#[allow(clippy::too_many_arguments)]
pub async fn snapshot_error(
    state: &DiagnosticsState,
    rec_id: &str,
    ctx: &laipe_streaming::RecordingContext,
    kind: ChatErrorKind,
    message: &str,
    cause: Option<&str>,
    conv_messages: &[laipe_core::ChatMessage],
    _error_context: &ErrorContext,
) -> Result<String, String> {
    let rec_dir = state.log_dir().await.join("recordings").join(rec_id);
    let report_dir = state.log_dir().await.join("reports");
    std::fs::create_dir_all(&report_dir).map_err(|e| format!("create reports dir failed: {e}"))?;

    // Filename: <ts>-<rec_id>.md — sortable + dedupes on rec_id.
    let ts = chrono::Utc::now().format("%Y%m%dT%H%M%S").to_string();
    let report_filename = format!("{}-{}.md", ts, sanitize_id(rec_id));
    let report_path = report_dir.join(&report_filename);

    // Load the request/response/meta from disk (written by the
    // FileRecorder during the round). Missing files are tolerated
    // — the section just says "not available".
    let request_text = read_to_string_lossy(&rec_dir.join("request.json"));
    let response_bytes = read_response_bytes(&rec_dir.join("response.bin"));
    let meta_text = read_to_string_lossy(&rec_dir.join("meta.json"));

    let cap = state.config().await.max_report_bytes;
    let response_section = render_response_section(&response_bytes, cap);

    let conv_section = render_conversation(conv_messages);
    let debug_hint = kind.to_debug_hint();
    let label = kind.label();

    let report = format!(
        "# laipe error report — {label}\n\
         \n\
         ---\n\
         \n\
         ```yaml\n\
         error_id: {rec_id}\n\
         ts: {ts_iso}\n\
         kind: {kind_str}\n\
         conversation_id: {conv}\n\
         turn: {turn}\n\
         model: {model}\n\
         api_format: {fmt}\n\
         endpoint: {endpoint}\n\
         ```\n\
         \n\
         ## Error\n\
         \n\
         {label}: {message}\n\
         {cause_section}\
         \n\
         ## Request\n\
         \n\
         <details><summary>Full request body (auth redacted)</summary>\n\
         \n\
         ```json\n\
         {request}\n\
         ```\n\
         \n\
         </details>\n\
         \n\
         ## Response\n\
         \n\
         {response_section}\n\
         \n\
         <details><summary>meta.json (recording metadata)</summary>\n\
         \n\
         ```json\n\
         {meta}\n\
         ```\n\
         \n\
         </details>\n\
         \n\
         ## Likely causes\n\
         \n\
         {debug_hint}\n\
         \n\
         ## Conversation context (this turn)\n\
         \n\
         {conv_section}\n\
         \n\
         ---\n\
         \n\
         _Generated by laipe diagnostics. For schema docs and LLM-friendly reading instructions, see `README-FOR-LLM.md` in the same directory._\n",
        ts_iso = chrono::Utc::now().to_rfc3339(),
        kind_str = kind.as_str(),
        conv = ctx.conversation_id.as_deref().unwrap_or("(none)"),
        turn = ctx.turn.map(|t| t.to_string()).unwrap_or_else(|| "(none)".to_string()),
        model = ctx.model,
        fmt = ctx.api_format,
        endpoint = ctx.endpoint,
        cause_section = cause
            .map(|c| format!("\nUnderlying cause: `{c}`\n"))
            .unwrap_or_default(),
        request = request_text,
        response_section = response_section,
        meta = meta_text,
    );

    // Atomic write: write to a temp file, then rename. Avoids
    // half-written reports if the process is killed mid-write.
    let tmp = report_path.with_extension("md.tmp");
    std::fs::write(&tmp, &report).map_err(|e| format!("write report failed: {e}"))?;
    std::fs::rename(&tmp, &report_path).map_err(|e| format!("rename report failed: {e}"))?;

    // Index line. Best-effort: if the index is corrupted or the disk
    // is full, we still have the report itself.
    let rel = format!("reports/{report_filename}");
    let index_entry = IndexEntry {
        id: rec_id.to_string(),
        ts: chrono::Utc::now().to_rfc3339(),
        conversation_id: ctx.conversation_id.clone(),
        turn: ctx.turn,
        kind: Some(kind.as_str().to_string()),
        model: Some(ctx.model.clone()),
        api_format: Some(ctx.api_format.to_string()),
        endpoint: Some(ctx.endpoint.clone()),
        message: message.to_string(),
        report_path: rel.clone(),
    };
    if let Ok(line) = serde_json::to_string(&index_entry) {
        let index_path = state.log_dir().await.join("reports").join("INDEX.jsonl");
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&index_path)
        {
            let _ = writeln!(f, "{line}");
        }
    }

    Ok(rel)
}

fn sanitize_id(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn read_to_string_lossy(path: &Path) -> String {
    match std::fs::read(path) {
        Ok(b) => String::from_utf8_lossy(&b).into_owned(),
        Err(_) => "(not available)".to_string(),
    }
}

fn read_response_bytes(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_default()
}

fn render_response_section(bytes: &[u8], cap: usize) -> String {
    if bytes.is_empty() {
        return "_No response bytes recorded._".to_string();
    }
    let truncated = bytes.len() > cap;
    let shown = if truncated { &bytes[..cap] } else { bytes };
    let preview = String::from_utf8_lossy(shown);
    let header = if truncated {
        format!(
            "_Truncated to first {cap} bytes (full size: {} bytes)._\n\n",
            bytes.len()
        )
    } else {
        format!("_Full response: {} bytes._\n\n", bytes.len())
    };
    format!("{header}```text\n{preview}\n```")
}

fn render_conversation(messages: &[laipe_core::ChatMessage]) -> String {
    if messages.is_empty() {
        return "_No conversation context captured._".to_string();
    }
    let mut out = String::new();
    for (i, m) in messages.iter().enumerate() {
        let role = match m.role {
            laipe_core::ChatRole::System => "system",
            laipe_core::ChatRole::User => "user",
            laipe_core::ChatRole::Assistant => "assistant",
            laipe_core::ChatRole::Tool => "tool",
        };
        let content = if m.content.len() > 800 {
            format!(
                "{}…(_truncated, {} bytes total_)",
                &m.content[..800],
                m.content.len()
            )
        } else {
            m.content.clone()
        };
        out.push_str(&format!("**[{i}] {role}**: {content}\n\n"));
    }
    out
}

// ---------------------------------------------------------------------------
// Recording lookup (used by dump_error_report)
// ---------------------------------------------------------------------------

/// Find the recording directory under `<log_dir>/recordings/` whose
/// `meta.json` matches the given (conversation_id, turn). We use
/// file content match — there's no global index, so a metadata scan
/// is the simplest path. For the volumes we expect (a handful of
/// recordings per session) this is fine; if it ever becomes a
/// bottleneck, switch to maintaining an in-memory map.
pub async fn find_recording_for(
    diag: &DiagnosticsState,
    conv_id: &str,
    turn: Option<u32>,
) -> Option<String> {
    let recordings_dir = diag.log_dir().await.join("recordings");
    let entries = match std::fs::read_dir(&recordings_dir) {
        Ok(e) => e,
        Err(_) => return None,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let meta_path = path.join("meta.json");
        let Ok(text) = std::fs::read_to_string(&meta_path) else {
            continue;
        };
        let Ok(meta) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        let meta_conv = meta
            .get("conversation_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if meta_conv != conv_id {
            continue;
        }
        if let Some(t) = turn {
            let meta_turn = meta.get("turn").and_then(|v| v.as_u64()).map(|n| n as u32);
            if meta_turn != Some(t) {
                continue;
            }
        }
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            return Some(name.to_string());
        }
    }
    None
}

// ---------------------------------------------------------------------------
// README-FOR-LLM.md
// ---------------------------------------------------------------------------

/// Bump when the schema changes in a way that requires an LLM to
/// re-read this file. The generator compares this against the value
/// in the existing README and rewrites if they differ.
const README_SCHEMA_VERSION: u32 = 1;

/// Write `<log_dir>/README-FOR-LLM.md` if it doesn't exist or is at
/// an older schema version. Idempotent — safe to call on every app
/// start.
pub fn write_readme_for_llm(log_dir: &Path) -> std::io::Result<()> {
    let path = log_dir.join("README-FOR-LLM.md");
    let need_rewrite = match std::fs::read_to_string(&path) {
        Ok(existing) => !existing.contains(&format!("schema_version: {README_SCHEMA_VERSION}")),
        Err(_) => true,
    };
    if !need_rewrite {
        return Ok(());
    }

    let body = render_readme_for_llm();
    std::fs::write(&path, body)
}

fn render_readme_for_llm() -> String {
    let kinds_doc: String = ChatErrorKind::ALL
        .iter()
        .map(|k| {
            format!(
                "### `{}` — {}\n\n{}\n",
                k.as_str(),
                k.label(),
                k.to_debug_hint()
            )
        })
        .collect();

    format!(
        "# README-FOR-LLM.md — laipe diagnostics\n\
         \n\
         **You are an LLM assistant helping the user debug a laipe app.**\
         This file is generated by the app and explains exactly where\
         to find the diagnostic data and how to read it. Read this once\
         at the start of a debug session, then jump to the relevant\
         report.\n\
         \n\
         schema_version: {ver}\n\
         \n\
         ## Directory layout\n\
         \n\
         ```\n\
         <app_log_dir>/\n\
         ├── README-FOR-LLM.md        ← this file\n\
         ├── INDEX.jsonl              ← one line per saved report; grep this\n\
         ├── recordings/\n\
         │   └── <rec-id>/\n\
         │       ├── request.json     ← exact request bytes (auth redacted)\n\
         │       ├── response.bin     ← raw response bytes, concatenated\n\
         │       └── meta.json        ← model, endpoint, conversation_id, turn, byte counts, outcome\n\
         └── reports/\n\
             └── <ts>-<rec-id>.md     ← self-contained .md report for one error\n\
         ```\n\
         \n\
         Where `<app_log_dir>` is the platform-specific Tauri 2 log dir:\n\
         \n\
         - **Windows**: `%LOCALAPPDATA%\\dev.laipe.app\\logs`\n\
         - **macOS**: `~/Library/Logs/dev.laipe.app`\n\
         - **Linux**: `~/.local/share/dev.laipe.app/logs`\n\
         \n\
         ## How to read the data\n\
         \n\
         1. **Find the error.** The user will typically paste one of:\n\
            - a `.md` report from `<app_log_dir>/reports/` (preferred — it has all context)\n\
            - a single `console:entry` from the in-app debug console (start with that one if so)\n\
            - a `rec-id` like `rec-1722700000000-1a2b3c4d`\n\
         2. **Read the .md first.** It has YAML frontmatter (ts, kind, model, conversation_id, turn), the error message, the full request, the raw response, and the likely causes for the `ChatErrorKind`.\n\
         3. **Cross-reference `INDEX.jsonl`** if the user asks about a pattern (\"show me all rate limit errors\"). It's one JSON object per line, greppable with `jq` or `grep`.\n\
         4. **The raw `response.bin`** is the bytes the upstream actually sent. If the .md's response section is truncated, you can read the file directly (up to 5 MiB by default).\n\
         \n\
         ## Per-`ChatErrorKind` debug recipes\n\
         \n\
         These are the **same hints** embedded in every saved report's \"Likely causes\" section. The app calls them from `laipe_core::ChatErrorKind::to_debug_hint()` — single source of truth.\n\
         \n\
         {kinds}\n\
         \n\
         ## What this is NOT\n\
         \n\
         - **Not a telemetry pipeline.** Nothing is sent off-device. All data is local.\n\
         - **Not a session replay tool.** Each `.md` is one error, not the full conversation timeline.\n\
         - **Not always-on.** The default config records nothing to disk; auto-snapshot is off. The user has to opt in via Settings.\n\
         \n\
         ## When updating the schema\n\
         \n\
         If you (the LLM assistant) recommend changes to the report format, ask the user to bump `README_SCHEMA_VERSION` in `laipe-app/src-tauri/src/diagnostics.rs` so this file regenerates on next start.\n",
        ver = README_SCHEMA_VERSION,
        kinds = kinds_doc,
    )
}

// ---------------------------------------------------------------------------
// ChatErrorKind::ALL — defined in laipe-core (inherent impls for a type
// must live in the type's defining crate).
// ---------------------------------------------------------------------------
