# Vision

## One-line positioning

**`laipe` is the lean, opinionated starting point for building LLM-powered agent desktop tools** — a Rust + (optional) TS framework that handles the 3-protocol streaming + tool-calling + state plumbing, so your app starts at "build my domain tabs" instead of "wire up SSE + spawn_blocking + figure out which JSON shape my LLM wants today".

## What `laipe` is

- **A library**, not a product — you import it into your own Tauri / Electron / axum / CLI project
- **A protocol layer**, not an agent framework — it doesn't decide what your agent does; it gives you the streaming + tool-calling plumbing to build your own
- **3-protocol, 1 API** — OpenAI Chat Completions, OpenAI Responses, Anthropic Messages all surface as the same `StreamEvent` enum on a `tokio::sync::mpsc::Receiver`
- **Anti-stutter by default** — the 4 Locus-battle-tested countermeasures are baked in (spawn_blocking, mpsc, identity-stable downstream state; rAF throttling is opt-in via `laipe-tokio`)
- **Tool calling first-class** — cross-protocol tool schema, accumulating partials, the "ask user / ask free text / update doc" patterns
- **Friendly errors** — 8-way `ChatErrorKind` classification, raw error preserved for devs but player never sees `OpenSSL error: ...`

## What `laipe` is NOT

- **Not an agent framework** — no planning loop, no ReAct, no LangGraph-style orchestration. You compose that yourself on top
- **Not provider-coupled** — doesn't ship an opinionated catalog of model names or pricing
- **Not a Tauri-locked** — works with any async Rust + reqwest
- **Not full LLM application boilerplate** — sessions, settings UI, project files are *your* responsibility (or your framework's); laipe is the lower layer those things sit on top of
- **Not PlotCraft** — PlotCraft is a specific AI-screenwriter desktop tool; laipe is the agent core extracted from it. PlotCraft's `concept` / `world` / `art` / `sessions` layers are not part of laipe

## Target users

1. **Solo devs** building an LLM agent desktop tool who keep rewriting the same SSE + tool plumbing
2. **Small teams** who want a single dependency that handles 3 LLM protocols consistently instead of vendoring 3 different SDKs
3. **Anyone** who found PlotCraft's chat stack useful but doesn't want to depend on a specific app

## Origin

`laipe` is the **agent-stripped** core of [PlotCraft](https://github.com/amostalong/plotcraft). PlotCraft is an AI-screenwriter tool built in Tauri 2; its chat / streaming / tool-calling layers were 4000+ lines of carefully-iterated Rust + TS code. `laipe` is that same code with the RPG-screenwriter business logic removed — so the lessons learned from running PlotCraft against OpenAI / Anthropic / DeepSeek / GLM are reusable in any agent project.

The name `laipe` is `a-gent` + `l-lm` + `p-ipe` — the three things it stitches together. It also happens to read like "all pipe" if you squint.

## Design principles

1. **Protocol-agnostic core, protocol-specific edges** — `laipe-core` has zero knowledge of HTTP / SSE. The three streaming impls are in `laipe-streaming`. Tomorrow's new protocol only touches streaming.
2. **Library, not framework** — no required event loop, no required UI integration. Bring your own tokio runtime, your own Tauri command, your own React state.
3. **Player-first error handling** — `ChatErrorKind` keys into a player-facing copy table; the raw `reqwest` string is a developer-only diagnostic dump, never shown raw.
4. **Closed tools leave the LLM blind** — if a tool is off, the `tools` field is **omitted** from the wire, not sent as `[]`. The LLM has zero knowledge tools exist.
5. **Anti-stutter by default, opt-in to throttling** — the parts that *must* live in laipe (spawn_blocking, mpsc) are always on; the parts that *can* live elsewhere (16ms rAF) are opt-in via `laipe-tokio::run_to_completion_throttled`.
6. **Minimal v0.1** — v0.1 ships the protocol layer and the SSE parser. Frontend wrappers (laipe-ts, laipe-vue) and Tauri integration (laipe-tauri) come in v0.2/v0.3 once the protocol layer has been hammered in production.
