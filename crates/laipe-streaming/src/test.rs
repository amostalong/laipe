//! Non-streaming test/ping of an endpoint + api_key + model combo.
//!
//! `test_provider` validates that a saved provider config actually works
//! end-to-end (auth + model id) without doing a full chat. It exists
//! because:
//!
//! - Chat errors are *deferred* — the first real chat call is the first
//!   time the user finds out their API key is wrong / model doesn't exist.
//!   A separate test endpoint lets the UI catch this in the settings panel
//!   instead of at chat time.
//! - It runs out-of-band from streaming — no SSE parse, no per-chunk
//!   emitter, no recorder thread. Just one POST + parse JSON response.
//!
//! ## Protocol handling
//!
//! The 3 supported [`ApiFormat`](laipe_core::ApiFormat) values each get:
//! - **URL path**: `/chat/completions` (OpenAI Chat), `/v1/responses`
//!   (OpenAI Responses), `/v1/messages` (Anthropic).
//! - **Auth header**: `Authorization: Bearer <key>` for OpenAI flavors;
//!   `x-api-key: <key>` + `anthropic-version: 2023-06-01` for Anthropic.
//! - **Body**: `messages: [{role: user, content: "hi"}]` + `max_tokens: 1`
//!   + `stream: false` (Anthropic gets the system message stripped to
//!   a top-level `system` field, per the protocol's no-system-in-messages
//!   rule).
//! - **Response parser**: pulls first content text from
//!   `choices[0].message.content` / `output[0].content[0].text` /
//!   `content[0].text` respectively.
//!
//! Errors are surfaced via the `error` field of the result, NOT via
//! `Err(_)`. Returning a structured result (rather than bubbling
//! `StreamError`) keeps the UI side free of streaming-specific error
//! enums.

use std::time::Duration;

use laipe_core::error::ChatErrorKind;
use laipe_core::types::{
    ApiFormat, ChatMessage, ChatRole, ProviderConfig, TestProviderParams, TestProviderResult,
};
use reqwest::Client;

/// HTTP status of a successful test round-trip.
const MAX_RESPONSE_PREVIEW: usize = 200;

/// Public entry point — validate an endpoint+key+model combo without
/// touching any streaming state.
///
/// Returns a structured result for every code path; never returns `Err(_)`.
/// Network errors, non-2xx status, invalid JSON, and "200 but no extractable
/// text" all flow through `ok = false` with a descriptive `error` string.
///
/// `params` is the canonical input — callers coming from a saved
/// `ProviderConfig` should use [`params_from_config`] to derive it.
pub async fn test_provider(params: &TestProviderParams) -> TestProviderResult {
    let endpoint = params.endpoint.trim().to_string();
    let model = params.model.trim().to_string();
    let api_format = params.api_format;
    let api_key = params.api_key.as_str();

    // Build a client with reasonable timeouts for a ping.
    let client = match Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(60))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return TestProviderResult {
                ok: false,
                status: None,
                error: Some(format!("reqwest builder: {e}")),
                response: None,
                endpoint,
                model,
                api_format,
            }
        }
    };

    let api_url = format!(
        "{}{}",
        endpoint.trim_end_matches('/'),
        test_endpoint_path(api_format)
    );
    let body = build_test_body(api_format, &model);
    let request_bytes = match serde_json::to_vec(&body) {
        Ok(b) => b,
        Err(e) => {
            return TestProviderResult {
                ok: false,
                status: None,
                error: Some(format!("request serialization: {e}")),
                response: None,
                endpoint,
                model,
                api_format,
            }
        }
    };

    let mut req = client
        .post(&api_url)
        .header("Content-Type", "application/json");
    req = apply_auth(req, api_format, api_key);
    if matches!(api_format, ApiFormat::Anthropic) {
        req = req.header("anthropic-version", "2023-06-01");
    }
    let req = req.body(request_bytes);

    let result = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            return TestProviderResult {
                ok: false,
                status: e.status().map(|s| s.as_u16()),
                error: Some(format!("request failed: {e}")),
                response: None,
                endpoint,
                model,
                api_format,
            }
        }
    };

    let status = result.status();
    let status_code = status.as_u16();
    let body_text = result.text().await.unwrap_or_default();

    if !status.is_success() {
        return TestProviderResult {
            ok: false,
            status: Some(status_code),
            error: Some(format!(
                "HTTP {status_code}: {}",
                truncate(&body_text, 500)
            )),
            response: None,
            endpoint,
            model,
            api_format,
        };
    }

    let parsed: serde_json::Value = match serde_json::from_str(&body_text) {
        Ok(v) => v,
        Err(e) => {
            return TestProviderResult {
                ok: false,
                status: Some(status_code),
                error: Some(format!(
                    "invalid JSON response: {e} (body: {})",
                    truncate(&body_text, 200)
                )),
                response: None,
                endpoint,
                model,
                api_format,
            }
        }
    };

    let response_text = extract_response_text(api_format, &parsed);
    let response_preview = response_text.as_ref().map(|s| truncate(s, MAX_RESPONSE_PREVIEW));

    if response_preview.is_none() {
        return TestProviderResult {
            ok: false,
            status: Some(status_code),
            error: Some(format!(
                "HTTP {status_code} but no extractable content text. Check that model id is correct."
            )),
            response: None,
            endpoint,
            model,
            api_format,
        };
    }

    TestProviderResult {
        ok: true,
        status: Some(status_code),
        error: None,
        response: response_preview,
        endpoint,
        model,
        api_format,
    }
}

// --- internals (private; only this module needs them) ---

/// Build the test request body for any of the 3 protocols.
/// Anthropic gets the system message stripped to a top-level `system` field
/// (per Anthropic's "no system role in messages" rule).
fn build_test_body(api_format: ApiFormat, model: &str) -> serde_json::Value {
    let messages = [ChatMessage {
        role: ChatRole::User,
        content: "hi".to_string(),
        tool_call_id: None,
        tool_calls: None,
    }];

    match api_format {
        ApiFormat::OpenAiChat => serde_json::json!({
            "model": model,
            "messages": messages,
            "max_tokens": 1,
            "stream": false,
        }),
        ApiFormat::OpenAiResponses => serde_json::json!({
            "model": model,
            "input": messages,
            "max_tokens": 1,
            "stream": false,
        }),
        ApiFormat::Anthropic => serde_json::json!({
            "model": model,
            "max_tokens": 1,
            "messages": messages,
        }),
    }
}

/// URL path appended to the user-supplied base endpoint.
fn test_endpoint_path(api_format: ApiFormat) -> &'static str {
    match api_format {
        ApiFormat::OpenAiChat => "/chat/completions",
        ApiFormat::OpenAiResponses => "/v1/responses",
        ApiFormat::Anthropic => "/v1/messages",
    }
}

/// Apply protocol-specific auth headers.
///
/// Empty api_key is a no-op (some local endpoints like Ollama skip auth).
fn apply_auth(
    req: reqwest::RequestBuilder,
    api_format: ApiFormat,
    api_key: &str,
) -> reqwest::RequestBuilder {
    if api_key.is_empty() {
        return req;
    }
    match api_format {
        ApiFormat::Anthropic => req.header("x-api-key", api_key),
        ApiFormat::OpenAiChat | ApiFormat::OpenAiResponses => {
            req.header("Authorization", format!("Bearer {api_key}"))
        }
    }
}

/// Pull first content text from a successful response body.
/// Returns `None` if the body shape doesn't match what we expect (e.g.
/// a brand-new protocol response style we don't recognize).
fn extract_response_text(api_format: ApiFormat, body: &serde_json::Value) -> Option<String> {
    match api_format {
        ApiFormat::OpenAiChat => body
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .map(|s| s.to_string()),
        ApiFormat::OpenAiResponses => body
            .get("output")
            .and_then(|o| o.get(0))
            .and_then(|o| o.get("content"))
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("text"))
            .and_then(|t| t.as_str())
            .map(|s| s.to_string()),
        ApiFormat::Anthropic => body
            .get("content")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("text"))
            .and_then(|t| t.as_str())
            .map(|s| s.to_string()),
    }
}

/// Truncate a string to N chars (char-boundary safe; not byte-boundary safe).
fn truncate(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// Map an HTTP status to the laipe [`ChatErrorKind`] for the result.
#[allow(dead_code)]
pub fn classify_test_status(status: u16) -> ChatErrorKind {
    match status {
        401 | 403 => ChatErrorKind::Auth,
        404 => ChatErrorKind::ModelNotFound,
        429 => ChatErrorKind::RateLimit,
        500..=599 => ChatErrorKind::ServerError,
        400..=499 => ChatErrorKind::BadRequest,
        _ => ChatErrorKind::Unknown,
    }
}

/// Build a `TestProviderParams` from a `ProviderConfig` — convenience for
/// callers that already have a saved `ProviderConfig` in hand.
pub fn params_from_config(cfg: &ProviderConfig) -> TestProviderParams {
    TestProviderParams {
        endpoint: cfg.endpoint.clone(),
        api_key: cfg.api_key.clone(),
        api_format: cfg.api_format,
        model: cfg.model.clone(),
    }
}
