# Changelog

All notable changes to laipe are documented here. Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Changed
- Documentation refresh for v0.2 starter model (README, VISION, ROADMAP rewritten; AGENTS updated)
- Removed `EXAMPLES.md` — laipe has one app now (`laipe-app/`), its own README covers it
- Project layout: `laipe-app/` is now top-level (no longer under `examples/`)

## [0.2.0] - 2026-08-02

### Added
- **`packages/laipe-ts`** — TypeScript SDK: 1:1 type mirror of `laipe-core` + shared `SseParser` + `dispatchStream` for all 3 protocols (openai_chat / openai_responses / anthropic). Includes `ChatStreamError` for pre-stream errors and a `StreamEvent` discriminated union matching the Rust shape.
- **`packages/laipe-vue`** — Vue 3 component layer split into 3 explicit layers:
  - **Primitives** (no state): `MessageBubble`, `MessageInput`, `EmptyState`, `IconButton`
  - **Composites** (assemble primitives): `ChatView`, `Sidebar`, `SettingsModal` — all with explicit slots (`header`, `footer`, `message`, `message-actions`, `input-before`, etc.) and CSS-variable theming
  - **Batteries-included**: `AiChatPanel` — one-line drop-in chat UI
- **StreamSource abstraction** — pluggable streaming source. Three implementations: `tauriStream` (production, default), `fetchStream` (browser-direct for testing / web-only), `mockStream` (offline UI dev). User-provided `StreamSource` impls supported.
- **`laipe-app`** — Tauri 2 desktop starter. Vue 3 frontend + Rust backend (Tauri 2 commands). Single .exe, native window, no browser, no CORS. Supports Windows / macOS / Linux / iOS / Android (via `cargo tauri build --target <triple>`).
- `laipe-app` Rust backend — `#[tauri::command] chat` calls `laipe_streaming::pick(...).dispatch(...)` and emits events (`chat:chunk`, `chat:done`, `chat:error`, `chat:tool_calls`, `chat:cancelled`). `CancelHandle` integration for the stop button.
- `laipe-app` Vue frontend — `App.vue` showcases the deep-composition path: composes `ChatView` + `Sidebar` + `SettingsModal` + `useChat(tauriStream)` directly, demonstrating how to build a custom layout from the primitives.
- `bun run gates` — single command runs the full gate suite: `cargo fmt --check` + `cargo clippy --workspace --all-targets -- -D warnings` + `cargo test --workspace` + `bun run typecheck:ts` + `bun run build:app-fe` + `cargo check -p laipe-app`.
- `run-laipe-app.bat` — Windows launcher: detects `cargo-tauri`, prompts to install if missing, runs `bun run tauri:dev` to open the native window.
- Bun workspace at the repo root (`package.json` + `bun.lock`) — `laipe-app` can `import { ... } from "laipe-vue"` and `from "laipe-ts"` via `workspace:*` protocol.

### Changed
- **Project structure**: laipe is now a **starter** (Rust + Vue 3 + Tauri 2, stack-locked), not a "framework" you wire into your own project. The deliverable is `laipe-app` — fork it, customize it, ship it.
- All 3 v0.1 examples (`vanilla-rust`, `vanilla-web`, `chat-app`) retired — they no longer fit the starter model. The Rust + TS + Vue + Tauri 2 stack is the only path.
- `clippy.toml` — removed 2 deprecated options (`single-char-elision`, `trivially-copy-pass-by-ref`).
- `crates/laipe-core/src/types.rs` — `impl Default for ChatRole` replaced with `#[derive(Default)]` + `#[default]` attribute on `User` (clippy `derivable_impls`).
- `crates/laipe-streaming/src/openai_chat.rs` — merged duplicate `Network` if-branches in error classifier (clippy `if_same_then_else`).
- `crates/laipe-streaming/src/sse.rs` — `loop { let Some = ... else { break; }; ... }` → `while let Some = ... { ... }` (clippy `while_let_loop`); manual `for + return` → `Iterator::find` (clippy `manual_find`).

## [0.1.1] - 2026-08-02

### Added
- Full minimal implementations of all three streaming protocols (OpenAI Chat / OpenAI Responses / Anthropic Messages)
- Shared SSE byte parser (supports both `data:` and `event:` shapes)
- 25 unit tests across `sse`, `openai_chat`, `openai_responses`, `anthropic`
- `StreamError::Upstream` carries `ChatErrorKind` for player-facing copy
- `StreamEvent::Error` variant for mid-stream errors
- `classify_upstream_error` + `map_reqwest_error` shared helpers
- README updated with status, quick start, architecture diagram
- CHANGELOG tracking v0.1 → v0.1.1 progress

## [0.1.0] - 2026-08-02

### Added
- Initial skeleton release — types nailed, crates wired, `cargo check` green. Streaming implementations were stubs.
