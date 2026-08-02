# Changelog

All notable changes to laipe are documented here. Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Done in v0.1
- `laipe-core::types` — `ApiFormat`, `ChatMessage`, `ChatRole`, `ChatStatus`, `ProviderConfig`, `StreamEvent` (with `Error` variant), `EffortLevel`, `AssistantToolCall`, `AssistantToolCallFunction`
- `laipe-core::error` — `ChatErrorKind` (8-way: Network/Auth/ModelNotFound/BadRequest/RateLimit/ServerError/StreamProtocol/Unknown) + `ChatErrorDiag`
- `laipe-core::tool` — `ToolDefinition`, `ToolFunction`, `ToolCallInfo`, `ToolCallPartial`, `ToolResult`
- `laipe-streaming::sse` — shared SSE byte parser supporting both `data:` and `event:` shapes, plus `: ping` heartbeats
- `laipe-streaming::openai_chat` — full minimal implementation (POST /v1/chat/completions, `data: [DONE]` terminator, 16ms rAF + mpsc channel)
- `laipe-streaming::openai_responses` — full minimal implementation (POST /v1/responses, `response.*` event vocabulary, tool call state tracked by `output_index`)
- `laipe-streaming::anthropic` — full minimal implementation (POST /v1/messages, `x-api-key` auth, system → top-level, tools → flat `[{name, description, input_schema}]` shape, content_block_start/delta/stop)
- `laipe-tokio::CancelHandle` + `run_to_completion`
- `laipe-streaming::classify_upstream_error` + `map_reqwest_error` shared helpers
- `examples/vanilla-rust` one-bin demo (uses `OPENAI_API_KEY` env var)
- **25 unit tests passing** across 4 test modules (sse, openai_chat, openai_responses, anthropic)
- README + LICENSE (MIT) + .gitignore + rustfmt + clippy + CHANGELOG + 3 docs/ files (PROTOCOLS.md, STREAMING.md, TOOL_CALLING.md)

### Out of scope (deferred to v0.2+)
- PlotCraft `src-tauri/src/llm/config.rs` (789 lines) — PlotCraft-specific, not part of the generic agent framework
- PlotCraft `src/lib/llm.ts` (254 lines) + `types/chat.ts` (160 lines) — frontend, deferred
- PlotCraft `useStreamReducer` (258 lines) + `ai-tools.ts` + `components/ai/*` — frontend
- PlotCraft starter files, sessions persistence, settings UI, 7-step concept design — PlotCraft-specific
- GLM reasoning_content, Anthropic cache_control, Responses early-version compatibility — niche features
- Effort / thinking / extended reasoning controls
- 16ms rAF throttling at the library level (consumers opt in via `laipe-tokio::run_to_completion_throttled` when they need it)

## [0.1.0] - 2026-08-02

### Added
- Initial skeleton release — types nailed, crates wired, `cargo check` green. Streaming implementations are stubs.

## [0.1.1] - 2026-08-02

### Added
- Full minimal implementations of all three streaming protocols (OpenAI Chat / OpenAI Responses / Anthropic Messages)
- Shared SSE byte parser (supports both `data:` and `event:` shapes)
- 25 unit tests across sse, openai_chat, openai_responses, anthropic
- `StreamError::Upstream` carries `ChatErrorKind` for player-facing copy
- `StreamEvent::Error` variant for mid-stream errors
- `classify_upstream_error` + `map_reqwest_error` shared helpers
- README updated with status, quick start, architecture diagram
- CHANGELOG tracking v0.1 → v0.1.1 progress

