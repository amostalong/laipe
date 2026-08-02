//! Anthropic Messages streaming.
//!
//! Wire: `POST {endpoint}/v1/messages` with `stream: true`, parses
//! `event: {message_start|content_block_*,message_delta,message_stop,ping,error}` SSE frames.
//! Auth uses `x-api-key` (not `Authorization: Bearer`), and `anthropic-version`
//! is required.
//!
//! Tool calling is fundamentally different from OpenAI's:
//! - Tools go out as `[{name, description, input_schema: parameters}]` (no
//!   nested `function`, and `input_schema` instead of `parameters`)
//! - Assistant tool calls come back as content blocks of `type: "tool_use"`
//!   with `input` (final parsed JSON) — input is streamed as `input_json_delta`
//! - Player replies are sent as `role: "user"` content blocks of
//!   `type: "tool_result"` referencing the `tool_use_id`
//!
//! v0.1 minimal: does **not** translate outgoing `ChatMessage::role = Tool`
//! into `tool_result` blocks, and does **not** translate `assistant.tool_calls`
//! into `tool_use` blocks. v0.1 minimal covers single-round text + tool call.

use async_trait::async_trait;
use futures::StreamExt;
use laipe_core::error::ChatErrorKind;
use laipe_core::tool::{ToolCallPartial, ToolDefinition};
use laipe_core::types::{ChatMessage, ChatRole, ProviderConfig, StreamEvent};
use reqwest::Client;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::recorder::{
    redact_request_bytes, CompletionOutcome, DiagnosticRecorder, RecordingContext,
};
use crate::sse::{SseFrame, SseParser};
use crate::StreamChat;
use crate::{classify_upstream_error, map_reqwest_error, StreamError, StreamResult};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const ANTHROPIC_VERSION: &str = "2023-06-01";
const DEFAULT_MAX_TOKENS: u32 = 4096;

pub struct AnthropicStreamer;

/// In-flight tool_use state. `name` arrives in `content_block_start`,
/// `input_json` accumulates across `content_block_delta` events.
#[derive(Default, Debug)]
struct PendingToolUse {
    id: Option<String>,
    name: Option<String>,
    input_json: String,
}

#[async_trait]
impl StreamChat for AnthropicStreamer {
    async fn run(
        &self,
        cfg: &ProviderConfig,
        messages: &[ChatMessage],
        tools: Option<&[ToolDefinition]>,
        recorder: Arc<dyn DiagnosticRecorder>,
        ctx: &RecordingContext,
    ) -> StreamResult<mpsc::Receiver<StreamEvent>> {
        let api_url = format!("{}/messages", cfg.endpoint.trim_end_matches('/'));
        let body = build_request_body(&cfg.model, messages, tools);
        let body_bytes = serde_json::to_vec(&body)
            .map_err(|e| StreamError::Other(format!("request serialization: {e}")))?;

        recorder
            .record_request(ctx, &redact_request_bytes(&body_bytes))
            .await;

        let client = Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .build()
            .map_err(|e| StreamError::Other(format!("reqwest builder: {e}")))?;

        let mut req = client
            .post(&api_url)
            .header("Content-Type", "application/json")
            .header("anthropic-version", ANTHROPIC_VERSION)
            .body(body_bytes);

        if !cfg.api_key.is_empty() {
            req = req.header("x-api-key", cfg.api_key.clone());
        }

        let resp = match req.send().await {
            Ok(r) => r,
            Err(e) => {
                let kind = match e.status() {
                    Some(s) => match s.as_u16() {
                        401 | 403 => ChatErrorKind::Auth,
                        404 => ChatErrorKind::ModelNotFound,
                        429 => ChatErrorKind::RateLimit,
                        500..=599 => ChatErrorKind::ServerError,
                        400..=499 => ChatErrorKind::BadRequest,
                        _ => ChatErrorKind::Unknown,
                    },
                    None => ChatErrorKind::Network,
                };
                let message = e.to_string();
                recorder
                    .record_completion(
                        ctx,
                        &CompletionOutcome::PreStreamFailure {
                            kind,
                            message: message.clone(),
                        },
                    )
                    .await;
                return Err(map_reqwest_error(e));
            }
        };
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            let kind = match status.as_u16() {
                401 | 403 => ChatErrorKind::Auth,
                404 => ChatErrorKind::ModelNotFound,
                429 => ChatErrorKind::RateLimit,
                500..=599 => ChatErrorKind::ServerError,
                400..=499 => ChatErrorKind::BadRequest,
                _ => ChatErrorKind::Unknown,
            };
            recorder
                .record_completion(
                    ctx,
                    &CompletionOutcome::PreStreamFailure {
                        kind,
                        message: format!("upstream returned {}: {}", status.as_u16(), body),
                    },
                )
                .await;
            return Err(classify_upstream_error(status.as_u16(), &body));
        }

        let (tx, rx) = mpsc::channel::<StreamEvent>(64);
        let mut byte_stream = resp.bytes_stream();

        let recorder_task = recorder.clone();
        let ctx_task = ctx.clone();
        let mut text_events: u32 = 0;
        let mut tool_events: u32 = 0;

        tokio::spawn(async move {
            let mut sse = SseParser::new();
            let mut pending: HashMap<u32, PendingToolUse> = HashMap::new();

            while let Some(chunk) = byte_stream.next().await {
                let bytes = match chunk {
                    Ok(b) => b,
                    Err(e) => {
                        let kind = ChatErrorKind::Network;
                        let message = format!("stream read error: {e}");
                        let _ = tx
                            .send(StreamEvent::Error {
                                kind,
                                message: message.clone(),
                            })
                            .await;
                        recorder_task
                            .record_completion(
                                &ctx_task,
                                &CompletionOutcome::Error { kind, message },
                            )
                            .await;
                        break;
                    }
                };
                recorder_task.record_response_chunk(&ctx_task, &bytes).await;
                for frame in sse.feed(&bytes) {
                    let events = match frame {
                        SseFrame::Named { event, data } => {
                            handle_anthropic_event(&event, &data, &mut pending)
                        }
                        SseFrame::Data(_) | SseFrame::Skip | SseFrame::Done => Vec::new(),
                    };
                    for ev in events {
                        if matches!(ev, StreamEvent::Done) {
                            let _ = tx.send(ev).await;
                            recorder_task
                                .record_completion(
                                    &ctx_task,
                                    &CompletionOutcome::Done {
                                        text_events,
                                        tool_call_events: tool_events,
                                    },
                                )
                                .await;
                            return;
                        }
                        match &ev {
                            StreamEvent::Text(_) => text_events += 1,
                            StreamEvent::ToolCalls(_) => tool_events += 1,
                            _ => {}
                        }
                        if tx.send(ev).await.is_err() {
                            recorder_task
                                .record_completion(&ctx_task, &CompletionOutcome::Cancelled)
                                .await;
                            return;
                        }
                    }
                }
            }
            let _ = tx
                .send(StreamEvent::Error {
                    kind: ChatErrorKind::StreamProtocol,
                    message: "upstream closed stream without message_stop".to_string(),
                })
                .await;
            recorder_task
                .record_completion(
                    &ctx_task,
                    &CompletionOutcome::Error {
                        kind: ChatErrorKind::StreamProtocol,
                        message: "upstream closed stream without message_stop".to_string(),
                    },
                )
                .await;
        });

        Ok(rx)
    }
}

/// Build the Anthropic Messages request body.
///
/// Tools policy: if `tools` is None or empty, the `tools` field is **not
/// written** — closed tools leave the LLM with zero knowledge they exist.
fn build_request_body(
    model: &str,
    messages: &[ChatMessage],
    tools: Option<&[ToolDefinition]>,
) -> Value {
    // System messages get pulled out to the top-level `system` field.
    // User / Assistant / Tool messages map to the `messages` array.
    // v0.1 minimal: tool messages are flattened to text content with a
    // [tool_result {id}] prefix — full `tool_result` content-block support
    // is v0.2. tool_calls on assistant messages are NOT translated (v0.1
    // is single-round only).
    let mut system_text: Option<String> = None;
    let mut anthropic_messages: Vec<Value> = Vec::new();

    for m in messages {
        match m.role {
            ChatRole::System => {
                // Concatenate multiple system messages with \n\n
                system_text = Some(match system_text.take() {
                    Some(prev) => format!("{prev}\n\n{}", m.content),
                    None => m.content.clone(),
                });
            }
            ChatRole::User => {
                anthropic_messages.push(json!({
                    "role": "user",
                    "content": m.content,
                }));
            }
            ChatRole::Assistant => {
                anthropic_messages.push(json!({
                    "role": "assistant",
                    "content": m.content,
                }));
            }
            ChatRole::Tool => {
                // v0.1 minimal: fall back to user message with a marker
                // prefix so the LLM at least sees the content. v0.2 should
                // translate properly to {type: "tool_result", tool_use_id, content}.
                let prefix = m
                    .tool_call_id
                    .as_deref()
                    .map(|id| format!("[tool_result {id}] "))
                    .unwrap_or_default();
                anthropic_messages.push(json!({
                    "role": "user",
                    "content": format!("{prefix}{}", m.content),
                }));
            }
        }
    }

    let mut body = json!({
        "model": model,
        "messages": anthropic_messages,
        "max_tokens": DEFAULT_MAX_TOKENS,
        "stream": true,
    });

    if let Some(system) = system_text {
        body["system"] = json!(system);
    }

    if let Some(tools) = tools {
        if !tools.is_empty() {
            // Anthropic: top-level `tools: [{name, description, input_schema: parameters}]`
            // — no `function` wrapper, `input_schema` not `parameters`.
            let flat_tools: Vec<Value> = tools
                .iter()
                .map(|t| {
                    json!({
                        "name": t.function.name,
                        "description": t.function.description,
                        "input_schema": t.function.parameters,
                    })
                })
                .collect();
            body["tools"] = json!(flat_tools);
        }
    }

    body
}

/// Translate a single Anthropic `event: ...` frame into zero-or-more
/// `StreamEvent`s. Tracks in-flight tool_use state across frames so
/// consumers can accumulate by `index`.
fn handle_anthropic_event(
    event: &str,
    data: &Value,
    pending: &mut HashMap<u32, PendingToolUse>,
) -> Vec<StreamEvent> {
    match event {
        // new content block — if tool_use, capture id + name
        "content_block_start" => {
            let index = data.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as u32;
            if let Some(block) = data.get("content_block") {
                if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                    let id = block.get("id").and_then(|i| i.as_str()).map(String::from);
                    let name = block.get("name").and_then(|n| n.as_str()).map(String::from);
                    let tu = pending.entry(index).or_default();
                    tu.id = id;
                    tu.name = name;
                }
            }
            Vec::new()
        }

        // text delta
        "content_block_delta" => {
            let index = data.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as u32;
            let delta = data.get("delta");
            let Some(delta) = delta else {
                return Vec::new();
            };
            match delta.get("type").and_then(|t| t.as_str()) {
                Some("text_delta") => {
                    if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
                        if !text.is_empty() {
                            return vec![StreamEvent::Text(text.to_string())];
                        }
                    }
                    Vec::new()
                }
                Some("input_json_delta") => {
                    let partial = delta
                        .get("partial_json")
                        .and_then(|p| p.as_str())
                        .unwrap_or("")
                        .to_string();
                    let tu = pending.entry(index).or_default();
                    tu.input_json.push_str(&partial);
                    if partial.is_empty() && tu.id.is_none() && tu.name.is_none() {
                        return Vec::new();
                    }
                    vec![StreamEvent::ToolCalls(vec![ToolCallPartial {
                        index,
                        id: tu.id.clone(),
                        name: tu.name.clone(),
                        arguments_delta: partial,
                    }])]
                }
                _ => Vec::new(),
            }
        }

        // content block finished — for tool_use, emit the final state
        "content_block_stop" => {
            let index = data.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as u32;
            if let Some(tu) = pending.get(&index) {
                if tu.id.is_some() || tu.name.is_some() {
                    // Emit a final partial with whatever arguments we've accumulated
                    return vec![StreamEvent::ToolCalls(vec![ToolCallPartial {
                        index,
                        id: tu.id.clone(),
                        name: tu.name.clone(),
                        arguments_delta: String::new(), // already streamed in pieces
                    }])];
                }
            }
            Vec::new()
        }

        // message finished
        "message_stop" => vec![StreamEvent::Done],

        // server error mid-stream
        "error" => {
            let message = data
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("anthropic stream error")
                .to_string();
            vec![StreamEvent::Error {
                kind: ChatErrorKind::StreamProtocol,
                message,
            }]
        }

        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn build_body_no_tools_pulls_system_out() {
        let body = build_request_body(
            "claude-3-5-sonnet-20241022",
            &[
                ChatMessage {
                    role: ChatRole::System,
                    content: "you are helpful".into(),
                    ..Default::default()
                },
                ChatMessage {
                    role: ChatRole::User,
                    content: "hi".into(),
                    ..Default::default()
                },
            ],
            None,
        );
        assert!(body.get("tools").is_none());
        assert_eq!(body["model"], "claude-3-5-sonnet-20241022");
        assert_eq!(body["system"], "you are helpful");
        // system should NOT be in messages
        assert!(body["messages"]
            .as_array()
            .unwrap()
            .iter()
            .all(|m| m["role"] != "system"));
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["messages"][0]["content"], "hi");
        assert_eq!(body["stream"], true);
        assert!(body.get("max_tokens").is_some());
    }

    #[test]
    fn build_body_with_tools_uses_input_schema_not_parameters() {
        let tools = vec![ToolDefinition {
            kind: Default::default(),
            function: laipe_core::tool::ToolFunction {
                name: "echo".into(),
                description: "echo back".into(),
                parameters: json!({"type": "object"}),
            },
        }];
        let body = build_request_body("claude-3-5-sonnet", &[], Some(&tools));
        assert_eq!(body["tools"][0]["name"], "echo");
        assert_eq!(body["tools"][0]["input_schema"]["type"], "object");
        // Anthropic must NOT have `parameters` or `function` wrapper
        assert!(body["tools"][0].get("parameters").is_none());
        assert!(body["tools"][0].get("function").is_none());
    }

    #[test]
    fn build_body_empty_tools_omits_field() {
        let body = build_request_body("claude-3-5-sonnet", &[], Some(&[]));
        assert!(body.get("tools").is_none());
    }

    #[test]
    fn parses_text_delta() {
        let mut pending = HashMap::new();
        let data = json!({
            "index": 0,
            "delta": {"type": "text_delta", "text": "hi"}
        });
        let events = handle_anthropic_event("content_block_delta", &data, &mut pending);
        assert_eq!(events.len(), 1);
        match &events[0] {
            StreamEvent::Text(s) => assert_eq!(s, "hi"),
            _ => panic!(),
        }
    }

    #[test]
    fn tracks_tool_use_across_start_and_delta() {
        let mut pending = HashMap::new();

        // content_block_start for tool_use
        let start = json!({
            "index": 1,
            "content_block": {"type": "tool_use", "id": "toolu_1", "name": "echo"}
        });
        let _ = handle_anthropic_event("content_block_start", &start, &mut pending);

        // input_json_delta
        let delta = json!({
            "index": 1,
            "delta": {"type": "input_json_delta", "partial_json": "{\"x\":"}
        });
        let events = handle_anthropic_event("content_block_delta", &delta, &mut pending);
        assert_eq!(events.len(), 1);
        match &events[0] {
            StreamEvent::ToolCalls(parts) => {
                assert_eq!(parts[0].id.as_deref(), Some("toolu_1"));
                assert_eq!(parts[0].name.as_deref(), Some("echo"));
                assert_eq!(parts[0].arguments_delta, "{\"x\":");
            }
            _ => panic!(),
        }
    }

    #[test]
    fn message_stop_emits_done() {
        let mut pending = HashMap::new();
        let events = handle_anthropic_event("message_stop", &json!({}), &mut pending);
        assert!(matches!(events.as_slice(), [StreamEvent::Done]));
    }
}
