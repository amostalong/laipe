//! OpenAI Chat Completions streaming.
//!
//! Wire: `POST {endpoint}/v1/chat/completions` with `stream: true`, parses
//! `data: {json}\n\n` SSE chunks. Each chunk's `choices[0].delta` carries
//! `content` (text delta) and/or `tool_calls[]` (tool-call partial). Stream
//! ends with `data: [DONE]`.
//!
//! The 3 anti-stutter countermeasures that the Locus battle-test pinned down
//! are kept here:
//! - `tokio::task::spawn_blocking` isolates SSE byte → JSON parse from the
//!   runtime's worker pool
//! - `mpsc::channel` decouples parse from emit
//! - No throttling on the library side — consumers throttle downstream
//!   if they need to (laipe-tokio ships a `run_to_completion_throttled`
//!   helper for the common case)
//!
//! Most "OpenAI-compatible" third-party providers (DeepSeek, GLM,
//! OpenRouter, llama.cpp's server, vLLM, etc.) speak this protocol. Use it
//! unless you specifically need Responses-only features.

use async_trait::async_trait;
use futures::StreamExt;
use laipe_core::error::ChatErrorKind;
use laipe_core::tool::{ToolCallPartial, ToolDefinition};
use laipe_core::types::{ChatMessage, ChatRole, ProviderConfig, StreamEvent};
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::sse::{SseFrame, SseParser};
use crate::StreamChat;
use crate::{StreamError, StreamResult};

#[cfg(test)]
use bytes::Bytes;

/// Default connect timeout (matches what PlotCraft v0.2+ settled on).
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

pub struct OpenAiChatStreamer;

#[async_trait]
impl StreamChat for OpenAiChatStreamer {
    async fn run(
        &self,
        cfg: &ProviderConfig,
        messages: &[ChatMessage],
        tools: Option<&[ToolDefinition]>,
    ) -> StreamResult<mpsc::Receiver<StreamEvent>> {
        let api_url = format!(
            "{}/chat/completions",
            cfg.endpoint.trim_end_matches('/'),
        );
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

            while let Some(chunk) = byte_stream.next().await {
                let bytes = match chunk {
                    Ok(b) => b,
                    Err(e) => {
                        let kind = classify_reqwest_error_text(&e.to_string());
                        let _ = tx
                            .send(StreamEvent::Error {
                                kind,
                                message: format!("stream read error: {e}"),
                            })
                            .await;
                        break;
                    }
                };
                for frame in sse.feed(&bytes) {
                    let item = match frame {
                        SseFrame::Data(json) => match parse_openai_chunk(&json) {
                            Ok(Some(parsed)) => parsed,
                            Ok(None) => continue,
                            Err(e) => {
                                let _ = tx
                                    .send(StreamEvent::Error {
                                        kind: ChatErrorKind::StreamProtocol,
                                        message: format!("parse error: {e}"),
                                    })
                                    .await;
                                return;
                            }
                        },
                        SseFrame::Done => {
                            let _ = tx.send(StreamEvent::Done).await;
                            return;
                        }
                        SseFrame::Skip | SseFrame::Named { .. } => continue,
                    };
                    if tx.send(item).await.is_err() {
                        return; // consumer dropped
                    }
                }
            }
        });

        Ok(rx)
    }
}

/// Build the OpenAI Chat Completions request body.
///
/// Tools policy: if `tools` is `None` or empty, the `tools` field is **not
/// written** on the wire. This matches the user requirement that closed
/// tools leave the LLM with zero knowledge they exist.
fn build_request_body(
    model: &str,
    messages: &[ChatMessage],
    tools: Option<&[ToolDefinition]>,
) -> Value {
    // Map ChatMessage → OpenAI Chat Completions JSON shape.
    // Mirrors the cross-protocol translation table in docs/TOOL_CALLING.md.
    let openai_messages: Vec<Value> = messages
        .iter()
        .map(|m| {
            let mut obj = serde_json::json!({
                "role": role_to_str(m.role),
                "content": m.content,
            });
            if let Some(id) = &m.tool_call_id {
                obj["tool_call_id"] = serde_json::json!(id);
            }
            if let Some(tcs) = &m.tool_calls {
                obj["tool_calls"] = serde_json::json!(tcs);
            }
            obj
        })
        .collect();

    let mut body = serde_json::json!({
        "model": model,
        "messages": openai_messages,
        "stream": true,
    });

    if let Some(tools) = tools {
        if !tools.is_empty() {
            body["tools"] = serde_json::json!(tools);
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

/// Parse a single OpenAI Chat Completions chunk into a `StreamEvent`, if it
/// carries a delta worth emitting.
fn parse_openai_chunk(v: &Value) -> Result<Option<StreamEvent>, String> {
    let choices = v.get("choices").and_then(|c| c.as_array());
    let Some(choices) = choices else {
        return Ok(None);
    };

    let mut text_delta = String::new();
    let mut tool_calls: Vec<ToolCallPartial> = Vec::new();

    for choice in choices {
        let Some(delta) = choice.get("delta") else {
            continue;
        };

        if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
            if !content.is_empty() {
                text_delta.push_str(content);
            }
        }

        if let Some(tcs) = delta.get("tool_calls").and_then(|t| t.as_array()) {
            for tc in tcs {
                let index = tc.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as u32;
                let id = tc.get("id").and_then(|i| i.as_str()).map(String::from);
                let name = tc
                    .get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(|n| n.as_str())
                    .map(String::from);
                let args_delta = tc
                    .get("function")
                    .and_then(|f| f.get("arguments"))
                    .and_then(|a| a.as_str())
                    .unwrap_or("")
                    .to_string();

                tool_calls.push(ToolCallPartial {
                    index,
                    id,
                    name,
                    arguments_delta: args_delta,
                });
            }
        }
    }

    if text_delta.is_empty() && tool_calls.is_empty() {
        return Ok(None);
    }

    if tool_calls.is_empty() {
        Ok(Some(StreamEvent::Text(text_delta)))
    } else {
        Ok(Some(StreamEvent::ToolCalls(tool_calls)))
    }
}

/// Map a reqwest error from `.send()` into a `StreamError` carrying the
/// classified `ChatErrorKind` (only used in diag / not user-facing copy here).
fn map_reqwest_error(e: reqwest::Error) -> StreamError {
    if let Some(status) = e.status() {
        return classify_upstream_error(status.as_u16(), &e.to_string());
    }
    StreamError::Other(e.to_string())
}

fn classify_reqwest_error_text(text: &str) -> ChatErrorKind {
    if text.contains("connect") || text.contains("TLS") || text.contains("handshake") {
        ChatErrorKind::Network
    } else if text.contains("timeout") {
        ChatErrorKind::Network
    } else {
        ChatErrorKind::Unknown
    }
}

fn classify_upstream_error(status: u16, body: &str) -> StreamError {
    let kind = match status {
        401 | 403 => ChatErrorKind::Auth,
        404 => ChatErrorKind::ModelNotFound,
        429 => ChatErrorKind::RateLimit,
        500..=599 => ChatErrorKind::ServerError,
        400..=499 => ChatErrorKind::BadRequest,
        _ => ChatErrorKind::Unknown,
    };
    let preview = body.chars().take(800).collect::<String>();
    StreamError::Upstream {
        kind,
        status,
        body_preview: preview,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_body_no_tools() {
        let body = build_request_body(
            "gpt-4o-mini",
            &[ChatMessage {
                role: ChatRole::User,
                content: "hi".into(),
                ..Default::default()
            }],
            None,
        );
        assert!(body.get("tools").is_none());
        assert_eq!(body["model"], "gpt-4o-mini");
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["messages"][0]["content"], "hi");
        assert_eq!(body["stream"], true);
    }

    #[test]
    fn build_body_empty_tools_omits_field() {
        let body = build_request_body("gpt-4o", &[], Some(&[]));
        assert!(
            body.get("tools").is_none(),
            "empty tools array must be omitted, not written as []"
        );
    }

    #[test]
    fn build_body_with_tools() {
        let tools = vec![ToolDefinition {
            kind: Default::default(),
            function: laipe_core::tool::ToolFunction {
                name: "echo".into(),
                description: "echo back".into(),
                parameters: serde_json::json!({"type": "object"}),
            },
        }];
        let body = build_request_body("gpt-4o", &[], Some(&tools));
        assert_eq!(body["tools"][0]["function"]["name"], "echo");
    }

    #[test]
    fn parse_chunk_text_delta() {
        let v = serde_json::json!({
            "choices": [{"delta": {"content": "hello"}}]
        });
        match parse_openai_chunk(&v).unwrap().unwrap() {
            StreamEvent::Text(s) => assert_eq!(s, "hello"),
            _ => panic!("expected Text"),
        }
    }

    #[test]
    fn parse_chunk_tool_call_delta() {
        let v = serde_json::json!({
            "choices": [{
                "delta": {
                    "tool_calls": [{
                        "index": 0,
                        "id": "call_1",
                        "function": {"name": "echo", "arguments": "{\"x\":"}
                    }]
                }
            }]
        });
        match parse_openai_chunk(&v).unwrap().unwrap() {
            StreamEvent::ToolCalls(parts) => {
                assert_eq!(parts.len(), 1);
                assert_eq!(parts[0].index, 0);
                assert_eq!(parts[0].id.as_deref(), Some("call_1"));
                assert_eq!(parts[0].name.as_deref(), Some("echo"));
                assert_eq!(parts[0].arguments_delta, "{\"x\":");
            }
            _ => panic!("expected ToolCalls"),
        }
    }

    #[test]
    fn parse_chunk_empty_returns_none() {
        let v = serde_json::json!({"choices": [{"delta": {}}]});
        assert!(parse_openai_chunk(&v).unwrap().is_none());
    }

    #[test]
    fn sse_parser_handles_multiple_frames() {
        let mut p = SseParser::new();
        let bytes = Bytes::from_static(
            b"data: {\"choices\":[{\"delta\":{\"content\":\"a\"}}]}\n\n\
              data: {\"choices\":[{\"delta\":{\"content\":\"b\"}}]}\n\n\
              data: [DONE]\n\n",
        );
        let items = p.feed(&bytes);
        assert_eq!(items.len(), 3);
        assert!(matches!(items[0], SseFrame::Data(_)));
        assert!(matches!(items[1], SseFrame::Data(_)));
        assert!(matches!(items[2], SseFrame::Done));
    }

    #[test]
    fn sse_parser_handles_split_chunk() {
        let mut p = SseParser::new();
        let bytes1 = Bytes::from_static(b"data: {\"choices\":[{\"delta\":{\"conte");
        let bytes2 = Bytes::from_static(b"nt\":\"hello\"}}]}\n\n");
        let items1 = p.feed(&bytes1);
        assert!(items1.is_empty(), "incomplete frame should buffer");
        let items2 = p.feed(&bytes2);
        assert_eq!(items2.len(), 1);
    }
}
