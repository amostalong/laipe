# Vision

## One-line positioning

**`laipe` is an agent client starter** — a set of composable components (Rust + Vue 3 + Tauri 2) for building LLM-powered agent desktop apps. Not a framework, not a backend, not a UI library. A starter you fork, modify, and ship.

## What `laipe` is

- **A starter, not a framework** — you fork `laipe-app` and own the result. laipe's components are open, replaceable, and explicitly designed for extension
- **A component set, not a single thing** — primitives (MessageBubble, MessageInput), composites (ChatView, Sidebar, SettingsModal), and a batteries-included `AiChatPanel`. You pick the layer that matches your customization appetite
- **Stack-locked at Rust + Vue 3 + Tauri 2** — one stack, five platforms (Windows, macOS, Linux, iOS, Android). No "use React" or "use Electron" decisions to make
- **3 protocols, 1 shape** — OpenAI Chat Completions, OpenAI Responses, Anthropic Messages all surface as the same `StreamEvent` (Rust: `mpsc::Receiver`, TS: `AsyncGenerator`)
- **Componentized at the right level** — infrastructure is fixed, app features are pluggable. Swap MessageBubble for MessageCodeBlock; swap tauriStream for fetchStream; swap localStorage for IndexedDB. The primitives are open, the wire format is stable
- **Friendly errors** — 8-way `ChatErrorKind` classification, raw `reqwest` strings preserved as developer-only diagnostic dump, player never sees `OpenSSL error: ...`

## What `laipe` is NOT

- **Not a framework** — no required event loop, no required UI integration, no required project layout. laipe is a starter you reshape
- **Not a backend** — laipe's Rust crates are libraries, not services. There's no "laipe server" you run; you ship the Tauri app and the Rust code runs in-process
- **Not a UI library** — the Vue components are an opinionated starter set, not a design system. Fork them, replace them, ignore them
- **Not an agent framework** — no planning loop, no ReAct, no LangGraph-style orchestration. You compose that on top using laipe's `StreamEvent` stream
- **Not a web stack** — laipe is desktop-first. Tauri uses the OS webview, not a browser, and the Rust backend is in-process. There is no separate "frontend" deploy target
- **Not provider-coupled** — laipe ships no opinionated catalog of model names or pricing. You bring your own `ProviderConfig`
- **Not a session / settings / project framework** — those are app concerns. laipe provides the LLM plumbing, not the rest

## Target users

1. **Solo devs** building an LLM agent desktop tool who keep rewriting the same SSE + tool plumbing
2. **Small teams** who want one stack across web and desktop, with one consistent LLM API
3. **Anyone** who found PlotCraft's chat stack useful but doesn't want to depend on a specific app

## Stack-locked by design

The decision to lock the stack at **Rust + Vue 3 + Tauri 2** is deliberate:

- **Rust** = fast streaming, type-safe IPC, single binary per platform, no runtime
- **Vue 3** = SFC, Composition API, small bundle, great TS support
- **Tauri 2** = OS webview (no Chromium bloat), Rust backend, mobile-ready (iOS + Android), single dev experience

Tauri's webview means "no browser required" — the app ships as a native window. The Rust backend means **no CORS** (we call the LLM API directly), **no leaked API keys** (they live in Rust process memory, not localStorage visible to devtools), and **real cancellation** via `CancelHandle`.

**Customization happens at the component level, not the infrastructure level.** You can swap MessageBubble for your own. You can swap tauriStream for fetchStream. You can swap localStorage for your backend. You cannot swap Vue for React. (If you want React, fork laipe — or wait for a `laipe-react` ports package.)

## Origin

`laipe` is the **agent-stripped** core of [PlotCraft](https://github.com/amostalong/plotcraft). PlotCraft is an AI-screenwriter tool built in Tauri 2; its chat / streaming / tool-calling layers were 4000+ lines of carefully-iterated Rust + TS code. `laipe` is that same code with the RPG-screenwriter business logic removed.

The name `laipe` is `a-gent` + `l-lm` + `p-ipe` — the three things it stitches together. It also happens to read like "all pipe" if you squint.

## Design principles

1. **Protocol-agnostic core, protocol-specific edges** — `laipe-core` has zero knowledge of HTTP / SSE. The three streaming impls are in `laipe-streaming`. Tomorrow's new protocol only touches streaming.
2. **Stream as a single shape** — `StreamEvent` is the contract. Rust side and TS side yield the same enum, so your consumer logic is portable.
3. **Componentized at the app-feature level** — infrastructure is fixed (Rust + Vue + Tauri); features (chat panel, sidebar, settings, theming) are replaceable components with explicit slots and props.
4. **Pluggable stream source** — the `StreamSource` interface lets you swap where chat events come from. Tauri backend (default), browser fetch (dev), mock (tests), or your own WebSocket bridge.
5. **Player-first error handling** — `ChatErrorKind` keys into a player-facing copy table; the raw `reqwest` string is a developer-only diagnostic dump, never shown raw.
6. **Closed tools leave the LLM blind** — if a tool is off, the `tools` field is **omitted** from the wire, not sent as `[]`. The LLM has zero knowledge tools exist.
7. **Minimal v0** — v0.1 ships the protocol layer. v0.2 adds the TS + Vue + Tauri starter. v1.0 freezes the public API.
