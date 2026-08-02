# Architecture

## What lives where

```
┌─────────────────────────────────────────────────────────────────────┐
│                         your app                                   │
│         (Tauri command / axum handler / CLI / etc.)                 │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              │  pick(ApiFormat).dispatch(cfg, msgs, tools)
                              │  → mpsc::Receiver<StreamEvent>
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                        laipe-streaming                              │
│                                                                     │
│  ┌─────────────┐   ┌─────────────────┐   ┌─────────────────┐      │
│  │ openai_chat │   │ openai_responses│   │   anthropic     │      │
│  │  POST       │   │  POST           │   │   POST          │      │
│  │  /v1/chat/  │   │  /v1/responses  │   │   /v1/messages  │      │
│  │  completions│   │                 │   │                 │      │
│  │  data: SSE  │   │  event: SSE     │   │   event: SSE    │      │
│  └──────┬──────┘   └────────┬────────┘   └────────┬────────┘      │
│         │                   │                     │               │
│         └──────────┬────────┴─────────┬───────────┘               │
│                    ▼                  ▼                           │
│         ┌──────────────────────────────────────┐                  │
│         │              sse parser              │                  │
│         │  (data: / event: / : ping / split)   │                  │
│         └──────────────────────────────────────┘                  │
│                              │                                     │
│                              ▼                                     │
│         ┌──────────────────────────────────────┐                  │
│         │  StreamEvent::Text | ToolCalls |     │                  │
│         │  Done | Error                       │                  │
│         └──────────────────────────────────────┘                  │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              │  StreamEvent stream
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                         laipe-tokio                                 │
│                                                                     │
│  ┌──────────────────┐   ┌─────────────────────────────┐           │
│  │  CancelHandle    │   │  run_to_completion         │           │
│  │  (clone + drop)  │   │  (forward Receiver→Sender) │           │
│  └──────────────────┘   └─────────────────────────────┘           │
└─────────────────────────────────────────────────────────────────────┘
                              │
                              │  StreamEvent stream
                              ▼
┌─────────────────────────────────────────────────────────────────────┐
│                         laipe-core                                 │
│                                                                     │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐                          │
│  │  types   │  │  error   │  │   tool   │                          │
│  │  (plain  │  │ (8-way  │  │ (schema) │                          │
│  │  data)   │  │  class) │  │          │                          │
│  └──────────┘  └──────────┘  └──────────┘                          │
│                                                                     │
│  Zero HTTP / SSE / async deps. Pure types and traits.             │
└─────────────────────────────────────────────────────────────────────┘
```

## Crate boundaries

### `laipe-core` — protocol-agnostic types

**Dependencies**: `serde`, `serde_json`, `thiserror`, `chrono`, `tracing`. No
`tokio`, no `reqwest`, no `bytes`. Pure types and traits.

**Public surface**:
- `types::ApiFormat`, `ChatMessage`, `ChatRole`, `ChatStatus`, `ChatMessage`, `ProviderConfig`, `StreamEvent`, `EffortLevel`, `AssistantToolCall`, `AssistantToolCallFunction`
- `error::ChatErrorKind`, `ChatErrorDiag`
- `tool::ToolDefinition`, `ToolFunction`, `ToolCallInfo`, `ToolCallPartial`, `ToolResult`

If you're writing a new streaming protocol backend, this is the only crate
you need to depend on for the types.

### `laipe-streaming` — 3-protocol SSE implementations

**Dependencies**: `laipe-core`, `tokio`, `reqwest` (with `rustls-tls`),
`serde_json`, `bytes`, `futures`, `async-trait`, `thiserror`, `tracing`.

**Public surface**:
- `StreamChat` trait — implemented by each of the 3 protocol streamers
- `StreamError` / `StreamResult<T>`
- `classify_upstream_error` / `map_reqwest_error` — shared helpers
- `pick(ApiFormat) -> &'static dyn StreamChatDispatch` — type-erased dispatch
- `openai_chat::OpenAiChatStreamer`
- `openai_responses::OpenAiResponsesStreamer`
- `anthropic::AnthropicStreamer`
- `sse::SseParser` + `SseFrame` — public so external protocol implementations can reuse the SSE byte parser

### `laipe-tokio` — runtime glue

**Dependencies**: `laipe-core`, `laipe-streaming`, `tokio`, `tokio-stream`,
`tracing`, `anyhow`.

**Public surface**:
- `CancelHandle` — cloneable abort handle, drop or `.cancel()` to stop
- `run_to_completion` — forward a `Receiver<StreamEvent>` into a `Sender` until `Done`

## The streaming pipeline

For one chat round-trip:

```
upstream TCP socket
   ↓
HTTP POST (reqwest)         ← laipe-streaming, plain async
   ↓
200 OK + response body      ← verify status, classify non-2xx
   ↓
bytes_stream()              ← reqwest byte stream (async)
   ↓
SseParser::feed(bytes)      ← crate::sse — buffer partial frames, emit complete frames
   ↓
SseFrame { Data | Named | Done | Skip }
   ↓
match frame per protocol     ← openai_chat / openai_responses / anthropic
   ↓
StreamEvent { Text | ToolCalls | Done | Error }
   ↓
tokio::sync::mpsc::Sender   ← backpressure: bounded channel(64)
   ↓
consumer (your code)        ← recv() and dispatch
```

## The 4 anti-stutter countermeasures

These came from the Locus battle-test (the editor tool this code was extracted
from) and they stay. See `docs/STREAMING.md` for the full writeup.

| # | Source | Counter | Lives in |
|---|--------|---------|----------|
| 1 | SSE byte → JSON parse blocks the tokio worker pool | `tokio::task::spawn_blocking` isolates parse from runtime | laipe-streaming (per-protocol) |
| 2 | 1k tok/s = 1k IPC emits/s floods the consumer | `mpsc::channel(64)` decouples parse from emit | laipe-streaming (per-protocol) |
| 3 | Per-token emit cost dominates rAF budget | 16ms rAF + 256-char batch on the emit side | consumer, opt-in via `laipe-tokio::run_to_completion_throttled` (planned v0.2) |
| 4 | Downstream state invalidated every chunk | Identity-stable: emit appends to `currentText` only, never touches `messages[]` reference | consumer |

Why 1+2+3 are not all in `laipe-streaming`:
- #1 and #2 are inside the streaming crate because they're tightly coupled to
  the HTTP / SSE protocol. Different protocols may need different
  implementations (e.g. Anthropic's `event:` style vs OpenAI's `data:` style).
- #3 is opt-in because not every consumer needs it. A CLI example that just
  prints chunks doesn't need a 16ms rAF; a Tauri command that emits to a
  `tauri::Emitter` absolutely does. Letting the consumer choose keeps the
  library small.

## Tool calling data flow

```
your app                              upstream
─────────────────────────────────────────────────────────────
ChatMessage { role: User, content: "hi" }
   ↓
[maybe attach assistant tool_calls from prior round]
   ↓
build_openai_request_body() / build_anthropic_request_body()
   ↓
POST { "tools": [...] or [] }        →  SSE stream
                                       ←  StreamEvent::ToolCalls(partials)  × N
                                       ←  StreamEvent::Done
   ↓ (consumers accumulate by index)
Vec<{ToolCallInfo {id, name, arguments}}>
   ↓ (app dispatches to its own tool handlers)
ChatMessage { role: Tool, tool_call_id, content: "result" }
   ↓
POST (round 2)                       →  StreamEvent::Text or ToolCalls
```

**Wire translation** lives in each protocol's `build_*_request_body`:
- `openai_chat`: tools go out as `[{type: "function", function: {name, description, parameters}}]`. Assistant tool calls come back as `choices[0].delta.tool_calls[]`.
- `openai_responses`: tools are **flattened** to `[{type, name, description, parameters}]` (no nested `function`). Assistant tool calls come back as `response.output_item.added` (with id+name) followed by `response.function_call_arguments.delta` (the JSON arguments streaming in).
- `anthropic`: tools are `[{name, description, input_schema: parameters}]` (no nested `function`, `input_schema` not `parameters`). Assistant tool calls come back as `content_block_start` (with id+name) followed by `content_block_delta` (with `input_json_delta`).

See `docs/TOOL_CALLING.md` for the full table.

## Error handling

8-way `ChatErrorKind` is the canonical taxonomy:

```rust
pub enum ChatErrorKind {
    Network,        // connect / TLS / DNS / timeout
    Auth,           // 401 / 403
    ModelNotFound,  // 404
    BadRequest,     // 400 / 4xx
    RateLimit,      // 429
    ServerError,    // 5xx
    StreamProtocol, // SSE protocol violation
    Unknown,        // catch-all
}
```

**Classification** happens in two places:
- `classify_upstream_error(status, body)` — at the HTTP boundary (non-2xx)
- Per-protocol inline `match` — for stream-level errors (SSE parse failure, `error` event, `message_stop` missing)

**Surface**:
- `StreamError::Upstream { kind, status, body_preview }` — pre-stream errors (HTTP non-2xx) come back as `Err(StreamError)` from `run()`
- `StreamEvent::Error { kind, message }` — mid-stream errors are sent as an item on the `mpsc::Receiver`

The player never sees `OpenSSL error: ...`. The `body_preview` is a dev-only
diagnostic dump, only shown behind a "copy diagnostic info" button in the UI
the app builds on top of laipe.

## Why the `StreamEvent` enum has `Error` separately from `StreamError`

`StreamError` (returned from `run()`) is for **pre-stream failures** — connect errors, 401, 404, etc. These abort the whole round before the channel even exists.

`StreamEvent::Error` is for **mid-stream failures** — SSE parse error, upstream `error` event, unexpected stream end. The stream has already started; the channel exists; consumers need to know the round is over but the channel is still valid (they might re-`run()` for round 2).

Splitting them lets consumers `match` cleanly:

```rust
match run(cfg, msgs, tools).await {
    Err(e) => {
        // pre-stream failure: cfg is bad, no point retrying without fixing cfg
        show_error_to_user(e);
    }
    Ok(mut rx) => {
        // stream started; consume until Done OR Error
        while let Some(ev) = rx.recv().await {
            match ev {
                StreamEvent::Text(delta) => { ... }
                StreamEvent::ToolCalls(parts) => { ... }
                StreamEvent::Done => break,
                StreamEvent::Error { kind, message } => {
                    // mid-stream failure: probably retry the same round
                    show_error_to_user(kind, message);
                    break;
                }
            }
        }
    }
}
```

## What v0.1 does NOT do (by design)

- **No 16ms rAF at the library level** — opt-in via `laipe-tokio::run_to_completion_throttled` (planned v0.2)
- **No reasoning content pass-through** — GLM `reasoning_content` is v0.2
- **No multi-round tool calling built in** — consumers must re-call `run()` themselves for round 2 (v0.2 ships a helper)
- **No `AnthropicCacheControl`** — v0.5
- **No `EffortLevel` → wire mapping** — the `ProviderConfig.effort` field exists but is not yet translated to `reasoning_effort` / `thinking` per protocol
- **No `ProviderConfig.max_tokens` / `temperature` → wire mapping** — fields exist in the type but protocols don't use them yet (defaults to platform defaults)

These are all **deliberately deferred**. The v0.1 API surface is intentionally
small so it's easier to commit to the shape.
