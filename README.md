# Laipe

> **a**gent + **l**lm + **p**ipe — a lean starting point for LLM-powered agent desktop tools.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org)
[![Status: v0.1.1](https://img.shields.io/badge/status-v0.1.1-yellow.svg)](ROADMAP.md)

`laipe` is a Rust + (optional) TS framework that handles the three things you keep rewriting every time you start an LLM agent project: 3-protocol streaming, tool calling, and friendly errors. Pull it in, wire it to a Tauri command (or axum handler, or Electron IPC, or CLI), and skip the 2 weeks of plumbing.

```rust
use laipe_core::types::{ApiFormat, ChatMessage, ChatRole, ProviderConfig};
use laipe_streaming::pick;

let cfg = ProviderConfig {
    endpoint: "https://api.openai.com/v1".into(),
    api_key: std::env::var("OPENAI_API_KEY")?,
    model: "gpt-4o".into(),
    api_format: ApiFormat::OpenAiChat,
    ..Default::default()
};

let messages = vec![ChatMessage { role: ChatRole::User, content: "Hello!".into(), ..Default::default() }];
let mut rx = pick(cfg.api_format).dispatch(&cfg, &messages, None).await?;

while let Some(ev) = rx.recv().await {
    match ev {
        StreamEvent::Text(delta)     => print!("{delta}"),
        StreamEvent::ToolCalls(parts) => { /* accumulate by index */ }
        StreamEvent::Done            => break,
        StreamEvent::Error { kind, message } => eprintln!("[error] {kind:?}: {message}"),
    }
}
```

Three protocols, one API. **OpenAI Chat Completions**, **OpenAI Responses**, and **Anthropic Messages** all surface as the same `StreamEvent` enum on a `tokio::sync::mpsc::Receiver`.

---

## Why `laipe`?

| You want to… | Use `laipe` if… | Use something else if… |
|---|---|---|
| Build a Tauri / Electron / axum / CLI app that talks to LLMs | ✅ This is the sweet spot | — |
| Avoid 3 different vendor SDKs in your deps | ✅ | — |
| Get the 4 Locus anti-stutter tricks (spawn_blocking, mpsc, rAF, identity-stable state) without re-deriving them | ✅ | — |
| ReAct / LangGraph-style agent orchestration | ❌ Not in scope — compose on top | Use [LangChain](https://github.com/langchain-ai/langchain), [Rig](https://github.com/0xplaydust/rig), etc. |
| Pure drop-in agent with planning loop | ❌ Not in v0.1 (planned v0.4 as `laipe-agent`) | Use [agentkit](https://crates.io/crates/agentkit) or [langgraph-rs](https://github.com/bbuneci/langgraph-rs) |
| OpenAI-only, no tool calls, no streaming | ❌ Overkill | Use [async-openai](https://crates.io/crates/async-openai) directly |
| Just want a hosted agent endpoint | ❌ Wrong layer | Use OpenAI Assistants API / Anthropic Bedrock Agents |

---

## Quick start

```bash
git clone https://github.com/amostalong/laipe.git
cd laipe

# Run the minimum-viable example (needs OPENAI_API_KEY in your env)
export OPENAI_API_KEY=sk-...
cargo run --bin laipe-vanilla-rust
```

You should see a single chat round-trip stream to stdout. That's the whole thing.

For a Tauri 2 desktop demo with a real chat window, see [`examples/tauri-minimal`](EXAMPLES.md) (planned v0.2).

## What's in the box

| Crate / path | Purpose | Status |
|---|---|---|
| [`laipe-core`](crates/laipe-core/src/lib.rs) | Protocol-agnostic types: `ChatMessage`, `ToolDefinition`, `ChatErrorKind`, `StreamEvent`. Zero HTTP/async deps. | ✅ v0.1.1 |
| [`laipe-streaming`](crates/laipe-streaming/src/lib.rs) | 3-protocol SSE streaming implementations + shared SSE byte parser. | ✅ v0.1.1 |
| [`laipe-tokio`](crates/laipe-tokio/src/lib.rs) | `CancelHandle` + `run_to_completion` runtime glue. | ✅ v0.1.1 |
| [`examples/vanilla-rust`](examples/vanilla-rust) | Minimum-viable: 1 binary, 1 message, 1 model. | ✅ v0.1.1 |
| `examples/tauri-minimal` | Tauri 2 desktop app, 1 chat tab + 1 example tab. | 🔴 v0.2 |
| `packages/laipe-ts` | Frontend mirror of laipe-core types + `fetchSSE()` helper. | 🔴 v0.2 |
| `packages/laipe-vue` | Vue 3 `AiChatPanel` / `AltCard` / `AskFreeTextInput` components. | 🔴 v0.3 |

25 unit tests across 4 test modules. `cargo check --workspace` is **green with 0 warnings, 0 errors**.

## Architecture

```
your app
   │
   │  pick(ApiFormat).dispatch(cfg, msgs, tools) → mpsc::Receiver<StreamEvent>
   ▼
┌─────────────────────── laipe-streaming ───────────────────────┐
│                                                                │
│  openai_chat  ·  openai_responses  ·  anthropic               │
│      │                │                   │                    │
│      └────────────────┴───────────────────┘                    │
│                         │                                     │
│                  sse::SseParser                               │
│              (data: / event: / : ping / split)                 │
│                                                                │
└────────────────────────────────────────────────────────────────┘
                         │
                         ▼
        StreamEvent::Text | ToolCalls | Done | Error
                         │
                         ▼
              ┌────── laipe-tokio ──────┐
              │  CancelHandle          │
              │  run_to_completion     │
              └────────────────────────┘
```

See [`ARCHITECTURE.md`](ARCHITECTURE.md) for the full writeup, including the 4 anti-stutter countermeasures and the data flow for tool calling.

## Documentation

| File | What's in it |
|---|---|
| [`VISION.md`](VISION.md) | One-line positioning, what `laipe` is and isn't, design principles, target users, origin story |
| [`ROADMAP.md`](ROADMAP.md) | v0.1 → v1.0 plan, including what's deliberately deferred to v0.2+ |
| [`ARCHITECTURE.md`](ARCHITECTURE.md) | Crate boundaries, streaming pipeline, anti-stutter tricks, tool-calling data flow, error handling |
| [`CONTRIBUTING.md`](CONTRIBUTING.md) | Pre-community status, ground rules, commit message style, what to work on |
| [`EXAMPLES.md`](EXAMPLES.md) | Example catalog (current and planned), how to write your own |
| [`CHANGELOG.md`](CHANGELOG.md) | Per-version what changed |
| [`docs/PROTOCOLS.md`](docs/PROTOCOLS.md) | 3-protocol comparison table + when to pick which |
| [`docs/STREAMING.md`](docs/STREAMING.md) | 4 anti-stutter countermeasures + StreamEvent flow |
| [`docs/TOOL_CALLING.md`](docs/TOOL_CALLING.md) | Tool schema cross-protocol translation table + 3 built-in patterns |

## Use it in your own Tauri project

Once v0.1 is published (or even now, with a `path = "..."` reference), adding laipe to a Tauri 2 project's `src-tauri/Cargo.toml` looks like:

```toml
[dependencies]
laipe-core = { path = "../laipe/crates/laipe-core" }
laipe-streaming = { path = "../laipe/crates/laipe-streaming" }
laipe-tokio = { path = "../laipe/crates/laipe-tokio" }
```

Then expose it as a Tauri command:

```rust
#[tauri::command]
async fn chat(
    cfg: ProviderConfig,
    messages: Vec<ChatMessage>,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut rx = pick(cfg.api_format)
        .dispatch(&cfg, &messages, None)
        .await
        .map_err(|e| e.to_string())?;

    while let Some(ev) = rx.recv().await {
        match ev {
            StreamEvent::Text(delta) => { state.app.emit("chat:chunk", delta).ok(); }
            StreamEvent::ToolCalls(parts) => { state.app.emit("chat:tool_call", parts).ok(); }
            StreamEvent::Done => { state.app.emit("chat:done", ()).ok(); }
            StreamEvent::Error { kind, message } => {
                state.app.emit("chat:error", ChatError { kind, message }).ok();
            }
        }
    }
    Ok(())
}
```

Your frontend listens for `chat:chunk` / `chat:tool_call` / `chat:done` / `chat:error` events. That's the whole integration.

## Where this came from

`laipe` is the **agent-stripped** core of [PlotCraft](https://github.com/amostalong/plotcraft), an AI-screenwriter desktop tool. PlotCraft's chat / streaming / tool calling layers were 4000+ lines of carefully-iterated Rust + TS code; `laipe` is that same code with the RPG-screenwriter business logic removed.

If you want to see what a full agent app looks like on top of `laipe`, read [PlotCraft's `src-tauri/src/llm/`](https://github.com/amostalong/plotcraft/tree/main/src-tauri/src/llm) — that's exactly the kind of thing `laipe` is meant to let you build faster.

## License

MIT. See [`LICENSE`](LICENSE).
