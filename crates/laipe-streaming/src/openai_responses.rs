//! OpenAI Responses streaming.
//!
//! Wire: `POST {endpoint}/v1/responses` with `stream: true`, parses
//! `event: response.*` SSE frames (not the older `data: {json}` shape).
//! The Responses API is OpenAI's newer protocol; laipe speaks both it and
//! the older Chat Completions.

use async_trait::async_trait;
use futures::StreamExt;
use laipe_core::error::ChatErrorKind;
use laipe_core::tool::{ToolCallPartial, ToolDefinition};
use laipe_core::types::{ChatMessage, ChatRole, ProviderConfig, StreamEvent};
use reqwest::Client;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::sse::{SseFrame, SseParser};
use crate::StreamChat;
use crate::{classify_upstream_error, map_reqwest_error, StreamError, StreamResult};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

pub struct OpenAiResponsesStreamer;

/// In-flight tool call state. We track the `added` event (id + name) and
/// accumulate `delta` events (arguments) until `done`.
#[derive(Default, Debug)]
struct PendingToolCall {
    id: Option<String>,
    name: Option<String>,
    arguments: String,
}

#[async_trait]
impl StreamChat for OpenAiResponsesStreamer {
    async fn run(
        &self,
        cfg: &ProviderConfig,
        messages: &[ChatMessage],
        tools: Option<&[ToolDefinition]>,
    ) -> StreamResult<mpsc::Receiver<StreamEvent>> {
        let api_url = format!("{}/responses", cfg.endpoint.trim_end_matches('/'));
        let body = build_request_body(&cfg.model, messages, tools);
        let body_bytes = serde_json::to_vec(&body)
            .map_err(|e| StreamError::Other(format!("request serialization: {e}")))?;

        let client = Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .build()
            .map_err(|e| StreamError::Other(format!("reqwest builder: {e}")))?;

        let mut req = client
            .post(&api_url)
            .header("Content-Type", "application/json")
            .body(body_bytes);

        if !cfg.api_key.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", cfg.api_key));
        }

        let resp = req.send().await.map_err(map_reqwest_error)?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(classify_upstream_error(status.as_u16(), &body));
        }

        let (tx, rx) = mpsc::channel::<StreamEvent>(64);
        let mut byte_stream = resp.bytes_stream();

        tokio::spawn(async move {
            let mut sse = SseParser::new();
            // `output_index` → pending tool call state
            let mut pending: HashMap<u32, PendingToolCall> = HashMap::new();

            while let Some(chunk) = byte_stream.next().await {
                let bytes = match chunk {
                    Ok(b) => b,
                    Err(e) => {
                        let _ = tx
                            .send(StreamEvent::Error {
                                kind: ChatErrorKind::Network,
                                message: format!("stream read error: {e}"),
                            })
                            .await;
                        break;
                    }
                };
                for frame in sse.feed(&bytes) {
                    let events = match frame {
                        SseFrame::Named { event, data } => {
                            handle_responses_event(&event, &data, &mut pending)
                        }
                        SseFrame::Data(_) | SseFrame::Skip | SseFrame::Done => Vec::new(),
                    };
                    for ev in events {
                        if matches!(ev, StreamEvent::Done) {
                            let _ = tx.send(ev).await;
                            return;
                        }
                        if tx.send(ev).await.is_err() {
                            return; // consumer dropped
                        }
                    }
                }
            }
        });

        Ok(rx)
    }
}

/// Build the OpenAI Responses request body.
///
/// Tools policy: if `tools` is None or empty, the `tools` field is **not
/// written**. Closed tools leave the LLM with zero knowledge they exist.
fn build_request_body(
    model: &str,
    messages: &[ChatMessage],
    tools: Option<&[ToolDefinition]>,
) -> Value {
    // OpenAI Responses accepts `input` as a messages array — each message
    // gets `{role, content}` (text-only for v0.1 minimal). Tool messages
    // and tool_calls on assistant messages would need conversion to
    // `{type: "function_call_output", ...}` / `{type: "function_call", ...}`
    // for full multi-round; v0.1 minimal omits that — your app is expected
    // to handle round-1 only or extend here.
    let input: Vec<Value> = messages
        .iter()
        .map(|m| {
            json!({
                "role": role_to_str(m.role),
                "content": m.content,
            })
        })
        .collect();

    let mut body = json!({
        "model": model,
        "input": input,
        "stream": true,
    });

    if let Some(tools) = tools {
        if !tools.is_empty() {
            // OpenAI Responses expects `tools: [{type, name, description, parameters}]`
            // (flat, no nested `function`).
            let flat_tools: Vec<Value> = tools
                .iter()
                .map(|t| {
                    json!({
                        "type": "function",
                        "name": t.function.name,
                        "description": t.function.description,
                        "parameters": t.function.parameters,
                    })
                })
                .collect();
            body["tools"] = json!(flat_tools);
        }
    }

    body
}

fn role_to_str(role: ChatRole) -> &'static str {
    match role {
        ChatRole::System => "system",
        ChatRole::User => "user",
        ChatRole::Assistant => "assistant",
        ChatRole::Tool => "tool",
    }
}

/// Translate a single Responses `event: response.*` frame into zero-or-more
/// `StreamEvent`s. Tracks in-flight tool-call state across frames so consumers
/// can accumulate by `output_index`.
fn handle_responses_event(
    event: &str,
    data: &Value,
    pending: &mut HashMap<u32, PendingToolCall>,
) -> Vec<StreamEvent> {
    match event {
        // text delta: emit directly
        "response.output_text.delta" => {
            if let Some(delta) = data.get("delta").and_then(|d| d.as_str()) {
                if !delta.is_empty() {
                    return vec![StreamEvent::Text(delta.to_string())];
                }
            }
            Vec::new()
        }

        // new tool call: start tracking
        "response.output_item.added" => {
            if let Some(item) = data.get("item") {
                if item.get("type").and_then(|t| t.as_str()) == Some("function_call") {
                    let output_index = data
                        .get("output_index")
                        .and_then(|i| i.as_u64())
                        .unwrap_or(0) as u32;
                    let id = item.get("id").and_then(|i| i.as_str()).map(String::from);
                    let name = item.get("name").and_then(|n| n.as_str()).map(String::from);
                    let tc = pending.entry(output_index).or_default();
                    tc.id = id;
                    tc.name = name;
                }
            }
            Vec::new()
        }

        // tool call arguments delta
        "response.function_call_arguments.delta" => {
            let output_index = data
                .get("output_index")
                .and_then(|i| i.as_u64())
                .unwrap_or(0) as u32;
            let delta = data
                .get("delta")
                .and_then(|d| d.as_str())
                .unwrap_or("");
            let tc = pending.entry(output_index).or_default();
            tc.arguments.push_str(delta);

            if delta.is_empty() && tc.id.is_none() && tc.name.is_none() {
                return Vec::new();
            }
            vec![StreamEvent::ToolCalls(vec![ToolCallPartial {
                index: output_index,
                id: tc.id.clone(),
                name: tc.name.clone(),
                arguments_delta: delta.to_string(),
            }])]
        }

        // tool call finished — emit final ToolCalls
        "response.function_call_arguments.done" => {
            let output_index = data
                .get("output_index")
                .and_then(|i| i.as_u64())
                .unwrap_or(0) as u32;
            if let Some(item) = data.get("item") {
                let id = item.get("id").and_then(|i| i.as_str()).map(String::from);
                let name = item.get("name").and_then(|n| n.as_str()).map(String::from);
                let arguments = item
                    .get("arguments")
                    .and_then(|a| a.as_str())
                    .unwrap_or("")
                    .to_string();
                let tc = pending.entry(output_index).or_default();
                if id.is_some() {
                    tc.id = id.clone();
                }
                if name.is_some() {
                    tc.name = name.clone();
                }
                tc.arguments = arguments.clone();
                vec![StreamEvent::ToolCalls(vec![ToolCallPartial {
                    index: output_index,
                    id: tc.id.clone(),
                    name: tc.name.clone(),
                    arguments_delta: arguments,
                }])]
            } else {
                Vec::new()
            }
        }

        // whole response done
        "response.completed" | "response.incomplete" => {
            vec![StreamEvent::Done]
        }

        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn build_body_no_tools() {
        let body = build_request_body(
            "gpt-4o",
            &[ChatMessage {
                role: ChatRole::User,
                content: "hi".into(),
                ..Default::default()
            }],
            None,
        );
        assert!(body.get("tools").is_none());
        assert_eq!(body["model"], "gpt-4o");
        assert_eq!(body["input"][0]["role"], "user");
        assert_eq!(body["input"][0]["content"], "hi");
        assert_eq!(body["stream"], true);
    }

    #[test]
    fn build_body_empty_tools_omits_field() {
        let body = build_request_body("gpt-4o", &[], Some(&[]));
        assert!(body.get("tools").is_none());
    }

    #[test]
    fn build_body_with_tools_flattens_to_responses_shape() {
        let tools = vec![ToolDefinition {
            kind: Default::default(),
            function: laipe_core::tool::ToolFunction {
                name: "echo".into(),
                description: "echo back".into(),
                parameters: json!({"type": "object"}),
            },
        }];
        let body = build_request_body("gpt-4o", &[], Some(&tools));
        // Responses shape: flat, no nested `function`
        assert_eq!(body["tools"][0]["type"], "function");
        assert_eq!(body["tools"][0]["name"], "echo");
        assert!(body["tools"][0].get("function").is_none());
    }

    #[test]
    fn parses_text_delta() {
        let mut pending = HashMap::new();
        let data = json!({"delta": "hello", "output_index": 0});
        let events = handle_responses_event("response.output_text.delta", &data, &mut pending);
        assert_eq!(events.len(), 1);
        match &events[0] {
            StreamEvent::Text(s) => assert_eq!(s, "hello"),
            _ => panic!(),
        }
    }

    #[test]
    fn tracks_function_call_across_added_and_delta() {
        let mut pending = HashMap::new();

        // Step 1: output_item.added with id + name
        let added = json!({
            "item": {
                "type": "function_call",
                "id": "call_1",
                "name": "echo"
            },
            "output_index": 0
        });
        let _ = handle_responses_event("response.output_item.added", &added, &mut pending);

        // Step 2: function_call_arguments.delta
        let delta = json!({
            "delta": "{\"x\":",
            "output_index": 0
        });
        let events = handle_responses_event(
            "response.function_call_arguments.delta",
            &delta,
            &mut pending,
        );
        assert_eq!(events.len(), 1);
        match &events[0] {
            StreamEvent::ToolCalls(parts) => {
                assert_eq!(parts[0].id.as_deref(), Some("call_1"));
                assert_eq!(parts[0].name.as_deref(), Some("echo"));
                assert_eq!(parts[0].arguments_delta, "{\"x\":");
            }
            _ => panic!(),
        }
    }

    #[test]
    fn completed_event_emits_done() {
        let mut pending = HashMap::new();
        let events = handle_responses_event("response.completed", &json!({}), &mut pending);
        assert!(matches!(events.as_slice(), [StreamEvent::Done]));
    }
}
