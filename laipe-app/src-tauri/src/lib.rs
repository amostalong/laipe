//! laipe-app — Tauri 2 desktop app, Vue 3 frontend, Rust + laipe streaming backend.
//!
//! Architecture
//! ============
//!
//! ```text
//!   ┌──────────────── Tauri window (OS webview) ────────────────┐
//!   │                                                           │
//!   │   Vue 3 UI (Vite-bundled)                                 │
//!   │        │                                                  │
//!   │        │ invoke('chat', { cfg, messages, tools,           │
//!   │        │                  conversation_id })              │
//!   │        ▼                                                  │
//!   │   Tauri IPC                                               │
//!   │        │                                                  │
//!   │        │  events: chat:chunk, chat:tool_calls,            │
//!   │        │          chat:done, chat:error, chat:cancelled   │
//!   │        ▼                                                  │
//!   │   Rust backend (this crate)                               │
//!   │        │                                                  │
//!   │        │  1. dispatch(cfg, messages, tools, recorder)     │
//!   │        │  2. receive ToolCalls → execute →                │
//!   │        │     append tool results to messages →             │
//!   │        │  3. re-dispatch (up to MAX_AGENT_TURNS)          │
//!   │        ▼                                                  │
//!   │   mpsc::Receiver<StreamEvent>                             │
//!   │                                                           │
//!   └───────────────────────────────────────────────────────────┘
//! ```
//!
//! Why this is good
//! ================
//!
//! - **No CORS**: the browser is a webview; outbound HTTP is done by Rust, not JS.
//! - **API key is safe**: lives in Rust process memory; not in `localStorage`.
//! - **Tool calls happen in Rust**: tools can access the filesystem, network,
//!   databases, native APIs — anything Tauri can do.
//! - **Mobile-ready**: same code compiles to iOS / Android / desktop.
//! - **LLM-debuggable**: every chat turn records to disk via
//!   `laipe_streaming::FileRecorder`; on error, a self-contained `.md`
//!   report is written so the user can hand it to an LLM assistant.
//!   See `diagnostics.rs` + `.agents/docs/DIAGNOSTICS.md`.

use laipe_core::error::ChatErrorKind;
use laipe_core::tool::{ToolCallPartial, ToolDefinition};
use laipe_core::types::{
    AssistantToolCall, AssistantToolCallFunction, ChatMessage, ChatRole, ProviderConfig,
    StreamEvent,
};
use laipe_streaming::RecordingContext;
use laipe_tokio::CancelHandle;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Mutex;

mod console;
mod diagnostics;

use console::{ConsoleDiag, ConsoleState};
use diagnostics::{DiagnosticConfig, DiagnosticsState};

/// Maximum number of agent-loop turns (one user turn = 1+1+N for LLM→tools→LLM→…).
/// Bounds runaway loops when a tool keeps making more tool calls.
const MAX_AGENT_TURNS: u32 = 4;

/// Per-app state. Holds the cancel handle for the in-flight chat (if any).
#[derive(Default)]
struct AppState {
    cancel: Arc<Mutex<Option<CancelHandle>>>,
}

/// Stream a chat completion with optional tool-calling agent loop.
///
/// Flow per turn:
///   1. Dispatch to LLM with the current messages + tools + diagnostic recorder
///   2. Stream text deltas and tool-call declarations to the frontend
///   3. On `Done`, if tool calls were declared, execute them, append the
///      results as `role: tool` messages, and re-dispatch (next turn)
///   4. If no tool calls, emit `chat:done` and return
///
/// `conversation_id` flows through to the diagnostic recorder so saved
/// reports can be grouped by conversation. The frontend passes the
/// active `useConversations().currentId`.
#[tauri::command]
async fn chat(
    cfg: ProviderConfig,
    messages: Vec<ChatMessage>,
    tools: Option<Vec<ToolDefinition>>,
    conversation_id: Option<String>,
    app: AppHandle,
    state: State<'_, AppState>,
    diag: State<'_, DiagnosticsState>,
) -> Result<(), String> {
    // Cancel handle for the whole agent loop (cancels mid-turn)
    let cancel = CancelHandle::new();
    {
        let mut guard = state.cancel.lock().await;
        if let Some(old) = guard.take() {
            old.cancel();
        }
        *guard = Some(cancel.clone());
    }
    let _ = app.emit("chat:start", ());
    console::log(
        &app,
        "info",
        "llm",
        format!(
            "chat start: model={} format={:?} messages={} tools={} conv={}",
            cfg.model,
            cfg.api_format,
            messages.len(),
            tools.as_ref().map(|t| t.len()).unwrap_or(0),
            conversation_id.as_deref().unwrap_or("(none)"),
        ),
    );

    // Mutable working copy of the conversation.
    let mut working = messages;
    let conv_id = conversation_id;

    let result: Result<(), String> = async {
        for turn in 0..MAX_AGENT_TURNS {
            if cancel.is_cancelled() {
                break;
            }

            // 1) Build the per-turn context, then dispatch.
            let ctx: RecordingContext = diag.new_context(
                conv_id.clone(),
                Some(turn),
                &cfg.model,
                &cfg.endpoint,
                format!("{:?}", cfg.api_format).as_str(),
            );
            let recorder = diag.recorder().await;

            let mut rx = match laipe_streaming::pick(cfg.api_format)
                .dispatch(&cfg, &working, tools.as_deref(), Some(recorder), &ctx)
                .await
            {
                Ok(rx) => rx,
                Err(e) => {
                    // Pre-stream failure. Determine the kind from the
                    // `Upstream` variant when possible; otherwise fall
                    // back to the message string.
                    let (kind, msg) = match &e {
                        laipe_streaming::StreamError::Upstream { kind, .. } => {
                            (*kind, format!("{e}"))
                        }
                        laipe_streaming::StreamError::Http(_) => {
                            (ChatErrorKind::Network, format!("{e}"))
                        }
                        laipe_streaming::StreamError::Io(_) => {
                            (ChatErrorKind::Network, format!("{e}"))
                        }
                        laipe_streaming::StreamError::Json(_) => {
                            (ChatErrorKind::StreamProtocol, format!("{e}"))
                        }
                        laipe_streaming::StreamError::Other(_) => {
                            (ChatErrorKind::Unknown, format!("{e}"))
                        }
                    };
                    let _ = app.emit(
                        "chat:error",
                        serde_json::json!({"kind": kind, "message": msg}),
                    );
                    console::log_with_diag(
                        &app,
                        "error",
                        "llm",
                        format!("[{kind:?}] turn {turn}: {msg}"),
                        ConsoleDiag::new()
                            .with_conversation_id(conv_id.clone().unwrap_or_default())
                            .with_turn(turn)
                            .with_kind(kind)
                            .with_request_digest(format!(
                                "model={} format={:?} messages={}",
                                cfg.model,
                                cfg.api_format,
                                working.len()
                            ))
                            .with_response_digest(format!("pre-stream error ({})", kind.as_str()))
                            .with_cause(msg.clone()),
                    );
                    if diag.auto_snapshot_enabled().await {
                        let error_context = laipe_core::ErrorContext::new()
                            .with_request_digest(format!(
                                "model={} format={:?}",
                                cfg.model, cfg.api_format
                            ))
                            .with_response_digest(format!("pre-stream ({})", kind.as_str()))
                            .with_stage("dispatch");
                        let _ = diagnostics::snapshot_error(
                            diag.inner(),
                            &ctx.id,
                            &ctx,
                            kind,
                            &msg,
                            Some(&msg),
                            &working,
                            &error_context,
                        )
                        .await;
                    }
                    return Ok(());
                }
            };

            // 2) Stream events for this turn
            let mut tool_calls: Vec<ToolCallPartial> = Vec::new();
            let mut had_error = false;
            let mut first_error: Option<(ChatErrorKind, String)> = None;

            while let Some(ev) = rx.recv().await {
                if cancel.is_cancelled() {
                    break;
                }
                match ev {
                    StreamEvent::Text(delta) => {
                        if app.emit("chat:chunk", delta).is_err() {
                            return Err("emit chat:chunk failed".into());
                        }
                    }
                    StreamEvent::ToolCalls(parts) => {
                        if app.emit("chat:tool_calls", &parts).is_err() {
                            return Err("emit chat:tool_calls failed".into());
                        }
                        tool_calls.extend(parts);
                    }
                    StreamEvent::Done => break,
                    StreamEvent::Error { kind, message } => {
                        let _ = app.emit(
                            "chat:error",
                            serde_json::json!({
                                "kind": kind,
                                "message": message,
                            }),
                        );
                        if first_error.is_none() {
                            first_error = Some((kind, message));
                        }
                        had_error = true;
                        break;
                    }
                }
            }

            if had_error {
                if let Some((kind, message)) = first_error {
                    console::log_with_diag(
                        &app,
                        "error",
                        "llm",
                        format!("[{kind:?}] turn {turn}: {message}"),
                        ConsoleDiag::new()
                            .with_conversation_id(conv_id.clone().unwrap_or_default())
                            .with_turn(turn)
                            .with_kind(kind)
                            .with_request_digest(format!(
                                "model={} format={:?} messages={}",
                                cfg.model,
                                cfg.api_format,
                                working.len()
                            ))
                            .with_response_digest(format!("mid-stream error ({})", kind.as_str()))
                            .with_cause(message.clone()),
                    );
                    if diag.auto_snapshot_enabled().await {
                        let error_context = laipe_core::ErrorContext::new()
                            .with_request_digest(format!(
                                "model={} format={:?}",
                                cfg.model, cfg.api_format
                            ))
                            .with_response_digest(format!("mid-stream ({})", kind.as_str()))
                            .with_stage("stream");
                        let _ = diagnostics::snapshot_error(
                            diag.inner(),
                            &ctx.id,
                            &ctx,
                            kind,
                            &message,
                            Some(&message),
                            &working,
                            &error_context,
                        )
                        .await;
                    }
                }
                return Ok(());
            }
            if cancel.is_cancelled() {
                break;
            }

            // 3) If no tool calls, this turn is final.
            if tool_calls.is_empty() {
                let _ = app.emit("chat:done", ());
                return Ok(());
            }

            // 4) Append the assistant message (with its tool_calls).
            working.push(ChatMessage {
                role: ChatRole::Assistant,
                content: String::new(),
                tool_call_id: None,
                tool_calls: Some(
                    tool_calls
                        .iter()
                        .map(|p| AssistantToolCall {
                            id: p.id.clone().unwrap_or_default(),
                            kind: "function".to_string(),
                            function: AssistantToolCallFunction {
                                name: p.name.clone().unwrap_or_default(),
                                arguments: p.arguments_delta.clone(),
                            },
                        })
                        .collect(),
                ),
            });

            // 5) Execute each tool and append a `role: tool` result message.
            for part in &tool_calls {
                let result_json =
                    execute_tool(part.name.as_deref().unwrap_or(""), &part.arguments_delta);
                let tool_msg = ChatMessage {
                    role: ChatRole::Tool,
                    content: result_json,
                    tool_call_id: part.id.clone(),
                    tool_calls: None,
                };
                working.push(tool_msg);
            }

            // 6) Loop continues → re-dispatch.
        }

        let _ = app.emit("chat:done", ());
        Ok(())
    }
    .await;

    // Clean up cancel handle
    {
        let mut guard = state.cancel.lock().await;
        if let Some(h) = guard.as_ref() {
            if h.is_cancelled() {
                let _ = app.emit("chat:cancelled", ());
            }
        }
        *guard = None;
    }

    result
}

/// Tool registry — a single match against tool name. Add new tools here.
fn execute_tool(name: &str, args_json: &str) -> String {
    match name {
        "get_current_time" => serde_json::json!({
            "current_time": chrono::Utc::now().to_rfc3339(),
            "timezone": "UTC",
        })
        .to_string(),
        "echo" => serde_json::json!({ "echoed": args_json }).to_string(),
        _ => serde_json::json!({
            "error": format!("unknown tool: {name}"),
            "received_args": args_json,
        })
        .to_string(),
    }
}

/// Cancel the in-flight chat (or agent loop), if any. Idempotent.
#[tauri::command]
async fn cancel(state: State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.cancel.lock().await;
    if let Some(h) = guard.take() {
        h.cancel();
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Diagnostic-mode Tauri commands
// ---------------------------------------------------------------------------

/// Get the current diagnostic config.
#[tauri::command]
async fn get_diagnostic_mode(
    diag: State<'_, DiagnosticsState>,
) -> Result<DiagnosticConfig, String> {
    Ok(diag.config().await)
}

/// Set the diagnostic config.
#[tauri::command]
async fn set_diagnostic_mode(
    cfg: DiagnosticConfig,
    diag: State<'_, DiagnosticsState>,
) -> Result<(), String> {
    diag.set_config(cfg).await;
    Ok(())
}

/// Build a saved-error-report for a specific `console_id`. The frontend
/// calls this from the "Save report" button on a console row.
#[tauri::command]
async fn dump_error_report(
    console_id: String,
    app: AppHandle,
    diag: State<'_, DiagnosticsState>,
    console_state: State<'_, ConsoleState>,
) -> Result<String, String> {
    let entry = console_state
        .snapshot()
        .into_iter()
        .find(|e| e.id == console_id)
        .ok_or_else(|| format!("console entry not found: {console_id}"))?;

    let rec_id = match entry.conversation_id.as_deref() {
        Some(conv) => diagnostics::find_recording_for(diag.inner(), conv, entry.turn).await,
        None => None,
    }
    .ok_or_else(|| {
        format!(
            "no on-disk recording matches console entry {console_id} (no matching conversation/turn)"
        )
    })?;

    let meta_path = diag
        .log_dir()
        .await
        .join("recordings")
        .join(&rec_id)
        .join("meta.json");
    let meta: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&meta_path).map_err(|e| format!("read meta.json: {e}"))?,
    )
    .map_err(|e| format!("parse meta.json: {e}"))?;

    let model = meta
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("(unknown)")
        .to_string();
    let endpoint = meta
        .get("endpoint")
        .and_then(|v| v.as_str())
        .unwrap_or("(unknown)")
        .to_string();
    let api_format = meta
        .get("api_format")
        .and_then(|v| v.as_str())
        .unwrap_or("(unknown)")
        .to_string();
    let conv_id_meta = meta
        .get("conversation_id")
        .and_then(|v| v.as_str())
        .map(String::from);
    let turn_meta = meta.get("turn").and_then(|v| v.as_u64()).map(|n| n as u32);

    let ctx = RecordingContext {
        id: rec_id.clone(),
        started_at: chrono::Utc::now(),
        api_format: api_format_match_static(&api_format),
        model,
        endpoint,
        conversation_id: conv_id_meta,
        turn: turn_meta,
    };

    let kind_str = entry.kind.as_deref().unwrap_or("unknown");
    let kind = parse_kind(kind_str);
    let message = entry.message.clone();
    let cause = entry.cause.clone();

    let error_context = laipe_core::ErrorContext::new()
        .with_request_digest(entry.request_digest.clone().unwrap_or_default())
        .with_response_digest(entry.response_digest.clone().unwrap_or_default())
        .with_cause(cause.clone().unwrap_or_default())
        .with_stage("dump_on_demand");

    let report_rel = diagnostics::snapshot_error(
        diag.inner(),
        &rec_id,
        &ctx,
        kind,
        &message,
        cause.as_deref(),
        &[],
        &error_context,
    )
    .await?;

    console::log(
        &app,
        "info",
        "diagnostics",
        format!("saved report for console entry {console_id}: {report_rel}"),
    );

    Ok(report_rel)
}

fn parse_kind(s: &str) -> ChatErrorKind {
    match s {
        "network" => ChatErrorKind::Network,
        "auth" => ChatErrorKind::Auth,
        "model_not_found" => ChatErrorKind::ModelNotFound,
        "bad_request" => ChatErrorKind::BadRequest,
        "rate_limit" => ChatErrorKind::RateLimit,
        "server_error" => ChatErrorKind::ServerError,
        "stream_protocol" => ChatErrorKind::StreamProtocol,
        _ => ChatErrorKind::Unknown,
    }
}

fn api_format_match_static(s: &str) -> &'static str {
    match s {
        "openai_chat" => "openai_chat",
        "openai_responses" => "openai_responses",
        "anthropic" => "anthropic",
        _ => "unknown",
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("laipe=info")),
        )
        .init();

    tauri::Builder::default()
        .manage(AppState::default())
        .manage(console::ConsoleState::new())
        .manage(DiagnosticsState::default())
        .setup(|app| {
            // Initialize the diagnostics subsystem now that we have
            // an AppHandle. The state was registered as a placeholder
            // above; this swap makes it real. We use `block_on` here
            // (not `tokio::spawn`) because setup runs on the main
            // thread and we want the placeholder to be replaced before
            // any chat command is invoked. This is the same
            // `async_runtime::block_on` pattern Tauri 2 itself uses
            // for its setup hooks.
            let handle = app.handle().clone();
            let state: tauri::State<'_, DiagnosticsState> = app.state();
            let inner_arc = state.shared();
            tauri::async_runtime::block_on(async move {
                DiagnosticsState::initialize_from_arc(inner_arc, &handle).await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            chat,
            cancel,
            console::get_console_entries,
            console::clear_console,
            console::get_console_entry_by_id,
            get_diagnostic_mode,
            set_diagnostic_mode,
            dump_error_report,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
