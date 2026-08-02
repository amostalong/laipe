# Roadmap

Versions roughly follow semver. The project is currently in **v0.2** — Rust
protocol layer + TS/Vue component layer + Tauri 2 starter app, all green
across Rust + TS gates.

## Status snapshot

| Component | Status | Next milestone |
|---|---|---|
| `laipe-core` (types, errors, tool schema) | ✅ stable | — |
| `laipe-streaming` (3 protocols) | ✅ minimal impl + 25 tests | v0.3 — multi-round tool calling, GLM `reasoning_content`, Anthropic prompt caching |
| `laipe-tokio` (cancel + run helper) | ✅ minimal | v0.3 — `run_to_completion_throttled` (16ms rAF + 256 char batch) |
| `packages/laipe-ts` (TS SDK, 3 protocols) | ✅ v0.2 | v0.3 — extension API: add custom protocol via `StreamSource` impl |
| `packages/laipe-vue` (Vue 3 components) | ✅ v0.2 | v0.3 — vitest + vue-test-utils, more slot examples, default `message-actions` |
| `laipe-app` (Tauri 2 starter) | ✅ v0.2 | v0.3 — CI release builds, mobile signing docs |

## v0.1 — "the protocol layer exists" (released)

**Goal**: prove the 3-protocol plumbing works, get eyes on the API surface,
ship the first external user (PlotCraft).

- [x] `laipe-core` types — `ChatMessage`, `StreamEvent`, `ApiFormat`, `EffortLevel`, `ChatErrorKind` (8-way), `ToolDefinition` / `ToolCallPartial`
- [x] `laipe-streaming::sse` — shared SSE byte parser supporting `data:` and `event:` frames
- [x] `laipe-streaming::openai_chat` — POST `/v1/chat/completions` with `data: [DONE]` terminator
- [x] `laipe-streaming::openai_responses` — POST `/v1/responses` with `response.*` event vocabulary
- [x] `laipe-streaming::anthropic` — POST `/v1/messages` with `x-api-key` auth, system → top-level, `input_schema` tool shape
- [x] `laipe-tokio::CancelHandle` + `run_to_completion`
- [x] 25 unit tests passing (`cargo test --workspace` green)
- [x] 0 warnings, 0 errors on `cargo check --workspace`
- [x] Docs: README, CHANGELOG, VISION, ROADMAP, ARCHITECTURE, CONTRIBUTING, EXAMPLES
- [x] Docs: `docs/PROTOCOLS.md`, `docs/STREAMING.md`, `docs/TOOL_CALLING.md`

**Not in v0.1** (deliberately deferred): frontend packages, Tauri integration, PlotCraft cutover, GLM `reasoning_content`, Anthropic `cache_control`, multi-round tool calling, throttling helper.

## v0.2 — "the starter is shippable" (released)

**Goal**: a forking user can clone, configure, and ship a Tauri 2 desktop agent client in minutes. The full stack (Rust + TS + Vue + Tauri) is wired and the components are composable.

- [x] `packages/laipe-ts` — TS SDK: 3-protocol `dispatchStream`, types mirror, error class, `SseParser`
- [x] `packages/laipe-vue` — Vue 3 components split into 3 layers (primitives / composites / `AiChatPanel` batteries-included) with explicit slots and CSS-variable theming
- [x] `laipe-app` — Tauri 2 desktop starter (Vue 3 frontend + Rust backend using laipe-streaming); `#[tauri::command] chat` + `cancel`; events `chat:chunk` / `chat:done` / `chat:error` / `chat:tool_calls` / `chat:cancelled`
- [x] `StreamSource` abstraction — pluggable streaming source (tauri / fetch / mock) so the same components work in Tauri, browser-only, and tests
- [x] `bun run gates` — single command runs fmt + clippy + test + typecheck + build across Rust and TS
- [x] Documentation: README / VISION / AGENTS / ROADMAP / CHANGELOG updated for the new stack-locked starter model; `EXAMPLES.md` removed (one app, lives in `laipe-app/README.md`)

**Not in v0.2** (deferred to v0.3+): tests for the Vue components, mobile build verification, more slot examples, the extension API docs.

## v0.3 — "production-quality starter"

**Goal**: laipe is a starter you can actually ship to real users without feeling like it's a hackathon demo.

- [ ] `packages/laipe-vue` — vitest + vue-test-utils test suite (primitives + composites + `AiChatPanel` snapshot)
- [ ] `laipe-streaming` — multi-round tool calling (assistant `tool_calls` → tool result echo → re-request)
- [ ] `laipe-streaming` — GLM `reasoning_content` pass-through for Zhipu-compatible providers
- [ ] `laipe-tokio` — `run_to_completion_throttled` (16ms rAF + 256-char batch, opt-in)
- [ ] `packages/laipe-vue` — default `message-actions` slot (copy / regenerate / edit) and `message` slot examples in the docs
- [ ] `laipe-app` — `tauri-plugin-stronghold` integration for OS-keyring API key storage (replace the `localStorage` fallback)
- [ ] `laipe-app` — CI release builds (GitHub Actions) producing `.msi` / `.dmg` / `.AppImage` artifacts
- [ ] 50+ unit tests + 1 integration test (real OpenAI round-trip, requires `OPENAI_API_KEY` in CI secret)

**Not in v0.3** (deferred): backend-agnostic agent loop (`laipe-agent` crate), multi-modal tool calls (image inputs), tool result caching, `laipe-react` ports package.

## v0.4+ — "ecosystem"

- v0.4 — `laipe-agent` crate (optional): a minimal ReAct-style loop on top of laipe-core. Apps that want a "drop-in agent" can pull this in; apps that want their own loop can stay on laipe-core
- v0.5 — Anthropic `cache_control` + prompt caching
- v0.5 — OpenAI `reasoning_effort` / Anthropic `thinking` / Gemini thinking configs
- v0.6 — Tool result caching + dedup
- v0.6 — Mobile signing docs + verified iOS / Android builds
- v0.7 — Multi-modal (image / audio inputs)
- v0.8 — Gemini / Mistral / Cohere / Groq providers
- v0.9 — First-pass API stability review
- **v1.0** — API freeze, semver guarantee, `missing_docs = "deny"` enforced, all public types have `///` doc comments

## Non-goals (will not ship)

- **Replacing `reqwest`** — laipe-streaming is built on `reqwest`. If you need a different HTTP stack, laipe isn't for you (yet)
- **Agent orchestration as a hard dependency** — apps compose their own loops on top. laipe never calls the LLM for you
- **A separate web stack** — Tauri is the stack. If you want a pure-browser deploy, fork laipe-app and replace Tauri with your own backend
- **A web UI as a separate product** — laipe-vue is part of the starter, not a publishable design system
- **Pricing / cost tracking** — laipe has no opinion on cost. Use Langfuse / Helicone / OpenLLMetry alongside

## Cadence

- **Patch releases (v0.2.1, v0.2.2 …)** — bug fixes, doc fixes, anything that doesn't change the public API
- **Minor releases (v0.3, v0.4 …)** — new features, new protocols, anything that adds API surface
- **Major releases** — API breaking changes, API freeze announcements

Until v1.0 the public API is **not** stable. Every v0.x minor release may rename / re-shape / split public types.
