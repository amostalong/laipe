# Roadmap

This is the public roadmap for `laipe`. Versions roughly follow semver. The
project is currently in **v0.1.1** — protocol layer skeleton with 25 unit
tests, no production usage yet.

## Status snapshot

| Component | v0.1.1 (now) | Next milestone |
|---|---|---|
| `laipe-core` (types, errors, tool schema) | ✅ stable | v0.2 — add `Config` schema for settings |
| `laipe-streaming` (3 protocols) | ✅ minimal impl + 25 tests | v0.2 — multi-round tool calling, GLM `reasoning_content` |
| `laipe-tokio` (cancel + run helper) | ✅ minimal | v0.2 — `run_to_completion_throttled` (16ms rAF + 256 char batch) |
| `examples/vanilla-rust` | ✅ runs OpenAI Chat | v0.1.2 — Anthropic + tool call example |
| `examples/tauri-minimal` | ❌ | v0.2 — first Tauri 2 demo |
| `packages/laipe-ts` (fetch SSE + types) | ❌ | v0.2 — frontend mirror of laipe-core |
| `packages/laipe-vue` (chat panel) | ❌ | v0.3 — Vue 3 components |
| `packages/laipe-react` (chat panel) | ❌ | v0.3+ — React port |
| `examples/electron-minimal` | ❌ | v0.3 — alternative to Tauri |
| `examples/vanilla-web` (HTML + fetch) | ❌ | v0.2 — pure-web SSE consumer |

## v0.1 — "the protocol layer exists" (current)

**Goal**: prove the 3-protocol plumbing works, get eyes on the API surface,
land the first external user (PlotCraft).

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
- [x] LICENSE (MIT)
- [x] Initial commit + project skeleton

**Not in v0.1** (deliberately deferred to v0.2+): frontend packages, Tauri integration, PlotCraft cutover, GLM reasoning_content, Anthropic cache_control, multi-round tool calling, throttling helper, more example apps.

## v0.2 — "PlotCraft can switch to it"

**Goal**: prove `laipe` works inside a real Tauri 2 desktop app (PlotCraft), prove a Tauri minimal example runs from scratch, and add the frontend mirror so non-Rust projects can use laipe too.

- [ ] `examples/tauri-minimal` — Tauri 2 desktop app with 1 chat tab + 1 example tab, end-to-end laipe → Tauri command → SSE
- [ ] `laipe-tokio::run_to_completion_throttled` — 16ms rAF + 256-char batch, opt-in helper for the consumer side
- [ ] `laipe-streaming` v0.2 — multi-round tool calling (assistant `tool_calls` on next request, tool result echoes)
- [ ] `laipe-streaming` v0.2 — GLM `reasoning_content` field pass-through (for `provider = "zhipu"`)
- [ ] `laipe-core::Config` — settings schema (provider list, model catalog, key storage hints)
- [ ] `packages/laipe-ts` — fetch SSE mirror, 8-error mapper, tool schema TS types
- [ ] `examples/vanilla-web` — pure HTML+JS consumer (no Tauri)
- [ ] PlotCraft `src-tauri/Cargo.toml` cutover: add `laipe = { path = "../Laipe" }`, remove duplicated `llm/` module
- [ ] PlotCraft `src/` cutover: swap `import` to `laipe-ts` mirror
- [ ] 50+ unit tests + 1 integration test (real OpenAI round-trip, requires `OPENAI_API_KEY` in CI secret)

**Not in v0.2** (deferred to v0.3+): `laipe-vue` components, `laipe-react` port, agent orchestration patterns, streaming reasoning summaries.

## v0.3 — "frontend first-class"

**Goal**: laipe is usable from a Vue or React project without writing your own SSE handler.

- [ ] `packages/laipe-vue` — `AiChatPanel`, `AltCard`, `AskFreeTextInput` Vue 3 components
- [ ] `packages/laipe-react` — same shape, React port
- [ ] `useStreamReducer` composable (Vue) / `useStreamReducer` hook (React) — PlotCraft's 12-field / 10-mutation state machine, ported
- [ ] `laipe-ts::resolveEnabledTools` — filter tool list by user settings before sending on the wire
- [ ] `examples/electron-minimal` — alternative to Tauri for Electron-based desktop apps
- [ ] CI: GitHub Actions running `cargo test --workspace` + `bun test` on every PR

**Not in v0.3** (deferred): backend-agnostic agent loop (`laipe-agent` crate), multi-modal tool calls (image inputs), tool result caching.

## v0.4+ — "ecosystem"

- v0.4 — `laipe-agent` crate (optional): a minimal ReAct-style loop on top of laipe-core. Apps that want a "drop-in agent" can pull this in; apps that want their own loop can stay on laipe-core.
- v0.5 — Anthropic `cache_control` + prompt caching
- v0.5 — OpenAI `reasoning_effort` / Anthropic `thinking` / Gemini thinking configs
- v0.6 — Tool result caching + dedup
- v0.7 — Multi-modal (image / audio inputs)
- v0.8 — Gemini / Mistral / Cohere / Groq providers
- v0.9 — First-pass API stability review
- **v1.0** — API freeze, semver guarantee, `missing_docs = "deny"` enforced, all public types have `///` doc comments

## Non-goals (will not ship)

- **Replacing `reqwest`** — laipe is built on `reqwest`. If you need a different HTTP stack, laipe isn't for you (yet).
- **Agent orchestration as a hard dependency** — apps compose their own loops on top. laipe never calls the LLM for you.
- **Session / project / settings persistence** — these are app-level concerns. `laipe-core::Config` will *describe* the schema; storage is your job.
- **A web UI** — laipe is a library. UI lives in `packages/laipe-vue` etc.
- **Pricing / cost tracking** — laipe has no opinion on cost. Use Langfuse / Helicone / OpenLLMetry alongside.

## Cadence

- **Patch releases (v0.1.2, v0.1.3 …)** — bug fixes, doc fixes, anything that doesn't change the public API
- **Minor releases (v0.2, v0.3 …)** — new features, new protocols, anything that adds API surface
- **Major releases** — API breaking changes, API freeze announcements

Until v1.0 the public API is **not** stable. Every v0.x minor release may rename / re-shape / split public types.
