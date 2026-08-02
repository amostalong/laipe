# laipe

> **a**gent + **l**lm + **p**ipe — an agent starter framework, distilled from the PlotCraft chat stack.

`laipe` is a lean, opinionated starting point for building **LLM-powered agent desktop tools** in Rust. It packages the four things you keep rewriting every time you start a new agent project:

1. **3-protocol streaming** — OpenAI Chat Completions, OpenAI Responses, Anthropic Messages, all behind one `StreamEvent` enum.
2. **Tool calling** — cross-protocol tool schema, accumulating partials, and `ask_user_question` / `ask_free_text` / `update_doc_item` patterns.
3. **Anti-stutter pipeline** — `tokio::task::spawn_blocking` + `mpsc` + 16ms rAF + 256-char batch, battle-tested at 1k tok/s.
4. **Friendly errors** — 8-way error classification with player-facing copy and developer-only diagnostic dumps.

If you've ever opened a new Tauri project and immediately thought *"ok now where do I put the chat, the streaming, the tool wiring, the settings tab…"* — `laipe` is the answer.

---

## Quick start

```bash
# Clone the repo
git clone https://github.com/amostalong/laipe.git
cd laipe

# Run the vanilla example (just needs OPENAI_API_KEY in your env)
export OPENAI_API_KEY=sk-...
cargo run --bin laipe-vanilla-rust
```

You should see a single chat round-trip stream to stdout. That's the whole thing.

## What's in the box

| Crate / package | Purpose | Status |
|---|---|---|
| `laipe-core` | Protocol-agnostic types: `ChatMessage`, `ToolDefinition`, `ChatErrorKind`, `StreamEvent` | ✅ v0.1 |
| `laipe-streaming` | 3-protocol SSE streaming: `openai_chat` / `openai_responses` / `anthropic` | 🟡 stub (v0.1) → full port (v0.1.1) |
| `laipe-tokio` | `CancelHandle`, `run_to_completion` helpers | ✅ v0.1 |
| `examples/vanilla-rust` | One-bin demo, no UI | 🟡 stub |
| `examples/tauri-minimal` | Tauri 2 desktop demo, 1 chat tab + 1 example tab | 🔴 v0.2 |
| `packages/laipe-ts` | Frontend `fetchSSE` + `useStreamReducer` + 8-error mapper | 🔴 v0.2 |
| `packages/laipe-vue` | `AiChatPanel` / `AltCard` / `AskFreeTextInput` Vue 3 components | 🔴 v0.2 |

## Usage

```rust
use laipe_core::types::{ApiFormat, ChatMessage, ChatRole, ProviderConfig};
use laipe_streaming::pick;

let cfg = ProviderConfig {
    endpoint: "https://api.openai.com/v1".into(),
    api_key:  std::env::var("OPENAI_API_KEY")?,
    model:    "gpt-4o".into(),
    api_format: ApiFormat::OpenAiChat,
    ..Default::default()
};

let messages = vec![ChatMessage {
    role: ChatRole::User,
    content: "Hello!".into(),
    ..Default::default()
}];

let mut rx = pick(cfg.api_format).dispatch(&cfg, &messages, None).await?;

while let Some(ev) = rx.recv().await {
    match ev {
        StreamEvent::Text(delta)        => print!("{delta}"),
        StreamEvent::ToolCalls(parts)   => { /* accumulate by index */ }
        StreamEvent::Done               => break,
    }
}
```

That's the whole API. Swap `ApiFormat::OpenAiChat` for `Anthropic` and you get Anthropic. Pass `Some(&tools)` to enable function calling. Pass `None` and the upstream never even sees a `tools` field.

## Architecture

```
┌──────────────────────────────────────────────────────────────┐
│ your app (Tauri / Electron / axum / CLI / …)                │
└──────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌──────────────────────────────────────────────────────────────┐
│ laipe-streaming                                              │
│   openai_chat  │  openai_responses  │  anthropic             │
│   (parse SSE + chunk + tool call delta)                      │
└──────────────────────────────────────────────────────────────┘
                              │ StreamEvent: Text | ToolCalls | Done
                              ▼
┌──────────────────────────────────────────────────────────────┐
│ your state machine (Vue ref / React useReducer / tokio task) │
└──────────────────────────────────────────────────────────────┘
```

The 4 anti-stutter countermeasures — `spawn_blocking` for SSE parse, `mpsc::channel(64)` to decouple parse from emit, 16ms rAF + 256-char batch on emit, and identity-stable downstream state — are baked in. You don't see them, but they're why the LLM doesn't freeze the UI.

## Where this came from

`laipe` is the **agent-stripped** core of [PlotCraft](https://github.com/amostalong/plotcraft), an AI-screenwriter desktop tool. PlotCraft's chat / streaming / tool calling layers were 4000+ lines of carefully-iterated Rust + TS code; `laipe` is that same code with the RPG-screenwriter business logic removed.

If you want to see what a full agent app looks like on top of `laipe`, read [PlotCraft's `src-tauri/src/llm/`](https://github.com/amostalong/plotcraft/tree/main/src-tauri/src/llm) and [PlotCraft's `src/components/ai/`](https://github.com/amostalong/plotcraft/tree/main/src/components/ai) — that's exactly the kind of thing `laipe` is meant to let you build faster.

## License

MIT. See [LICENSE](LICENSE).

## Status

**v0.1 is the skeleton release.** Crates compile, `cargo check --workspace` is green, types are nailed down. The three streaming implementations are stubs that return `not yet implemented` — they're being ported in from PlotCraft in the next few commits. Watch this repo or follow along in `docs/PLAN.md` (coming soon).
