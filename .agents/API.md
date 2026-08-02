# API Reference

The **single source of truth** for the laipe public API surface. If a symbol is exported from a crate / package's root, it's documented here. If you find something exported but not listed, treat it as undocumented internal API — file an issue.

The TS and Rust surfaces are **mirrors** of each other by design. Where a type exists in both, it's listed once with both names and a pointer to the canonical (Rust) source.

> **Where to start**: see the [navigation order in `AGENTS.md`](AGENTS.md#pointers-for-an-llm-picking-up-the-codebase). This doc is the "where is X defined" map once you know what you're looking for.

---

## `laipe-core` (Rust) — `crates/laipe-core/`

Protocol-agnostic types. Zero HTTP / async deps. The **canonical source of truth** for the type system; TS mirrors everything here.

```toml
[dependencies]
laipe-core = { path = "crates/laipe-core" }
```

| Symbol | Where | Purpose |
|---|---|---|
| `ApiFormat` | `types.rs` | `OpenAiChat` / `OpenAiResponses` / `Anthropic` — which wire protocol to speak. |
| `EffortLevel` | `types.rs` | `Low` / `Medium` / `High` — reasoning effort for supported models. |
| `ChatRole` | `types.rs` | `System` / `User` / `Assistant` / `Tool` — message role. |
| `ChatStatus` | `types.rs` | `Idle` / `Streaming` / `Error` / `Cancelled` — high-level chat state. |
| `ChatMessage` | `types.rs` | One message in a conversation; `tool_call_id` / `tool_calls` for tool flows. |
| `ProviderConfig` | `types.rs` | Per-run config: endpoint, key, model, format, effort, tools. |
| `AssistantToolCall` | `types.rs` | One tool call in an assistant message (id + name + accumulating args). |
| `AssistantToolCallFunction` | `types.rs` | The `function` part of a tool call. |
| `StreamEvent` | `types.rs` | `Text` / `ToolCalls` / `Done` / `Error` — what streaming yields. |
| `ToolDefinition` | `tool.rs` | Schema for a function the LLM may call. |
| `ToolType` | `tool.rs` | `Function` (only one for now). |
| `ToolFunction` | `tool.rs` | `name` + `description` + JSON Schema `parameters`. |
| `ToolCallInfo` | `tool.rs` | Final, JSON-decoded tool call (after stream complete). |
| `ToolCallPartial` | `tool.rs` | A streaming tool-call fragment (index, optional id/name, raw args delta). |
| `ToolResult` | `tool.rs` | What the tool execution returns (tool_call_id + JSON content). |
| `ChatErrorKind` | `error.rs` | Categorical error type for UI display + retry logic. |
| `ChatError` | `error.rs` | Error wrapper with kind + diagnostic context. |
| `ChatErrorDiag` | `error.rs` | Optional diagnostic: status, body, x-request-id, stage. |

---

## `laipe-streaming` (Rust) — `crates/laipe-streaming/`

3-protocol SSE producers. Consumers get an `mpsc::Receiver<StreamEvent>` and translate it to whatever wire format they need (Tauri events, web SSE, file).

```toml
[dependencies]
laipe-streaming = { path = "crates/laipe-streaming" }
```

| Symbol | Where | Purpose |
|---|---|---|
| `StreamChat` (trait) | `lib.rs` | The protocol-implementor trait. `run(cfg, messages, tools) -> Receiver<StreamEvent>`. |
| `StreamChatDispatch` (trait) | `lib.rs` | Type-erased wrapper for `StreamChat` — same call site, no generics at the consumer. |
| `pick(fmt: ApiFormat) -> &'static dyn StreamChatDispatch` | `lib.rs` | The single dispatch point. Add a new protocol by implementing `StreamChat` and arm in `pick`. |
| `OpenAiChatStreamer` | `openai_chat.rs` | OpenAI Chat Completions protocol. |
| `OpenAiResponsesStreamer` | `openai_responses.rs` | OpenAI Responses protocol. |
| `AnthropicStreamer` | `anthropic.rs` | Anthropic Messages protocol. |
| `StreamError` | `lib.rs` | Pre-stream errors (connect / TLS / timeout / 4xx / 5xx). |
| `StreamResult<T>` | `lib.rs` | `Result<T, StreamError>` alias. |
| `classify_upstream_error(status, body)` | `lib.rs` | Map HTTP status → `ChatErrorKind`. |
| `map_reqwest_error(e)` | `lib.rs` | Best-effort `reqwest::Error` → `StreamError`. |
| `SseParser` | `sse.rs` | Stateful SSE byte → frame parser. |
| `throttle` (module) | `throttle.rs` | Optional 16ms rAF + 256-char batching for the emit side. |

**Tests**: 25 unit tests in `laipe-streaming` (build body, parse chunk, track tool calls, etc.). These are the floor for any new public API in this crate.

---

## `laipe-tokio` (Rust) — `crates/laipe-tokio/`

Runtime glue: `CancelHandle` + `run_to_completion` helper for driving a stream consumer to completion on a tokio task.

```toml
[dependencies]
laipe-tokio = { path = "crates/laipe-tokio" }
```

| Symbol | Where | Purpose |
|---|---|---|
| `CancelHandle` | `cancel.rs` | Clone-able cancellation token. `cancel()` kills the in-flight stream consumer. |
| `run_to_completion` | `run.rs` | Drive an async stream to completion, propagating cancellation. |

---

## `laipe-ts` (TypeScript) — `packages/laipe-ts/`

Browser/Node-native mirror of `laipe-core` + SSE parser + protocol dispatch. Use directly when you don't want a Tauri backend (e.g. pure-web deploys, tests).

```bash
bun add laipe-ts
```

### Types (mirror of `laipe-core`)

| TS name | Rust source | Notes |
|---|---|---|
| `ApiFormat` | `laipe_core::ApiFormat` | string literal union: `"openai_chat"` / `"openai_responses"` / `"anthropic"` |
| `EffortLevel` | `laipe_core::EffortLevel` | `"low"` / `"medium"` / `"high"` |
| `ChatRole` | `laipe_core::ChatRole` | `"system"` / `"user"` / `"assistant"` / `"tool"` |
| `ChatStatus` | `laipe_core::ChatStatus` | `"idle"` / `"streaming"` / `"error"` / `"cancelled"` |
| `ChatMessage` | `laipe_core::ChatMessage` | `role` / `content` / `tool_call_id?` / `tool_calls?` |
| `ProviderConfig` | `laipe_core::ProviderConfig` | `endpoint` / `api_key` / `model` / `api_format` / optional `effort` / `max_tokens` / `temperature` / `tools` |
| `AssistantToolCall` | `laipe_core::AssistantToolCall` | `id` / `type: "function"` / `function: { name, arguments }` |
| `StreamEvent` | `laipe_core::StreamEvent` | `{ type: "text", delta }` / `{ type: "tool_calls", partials }` / `{ type: "done" }` / `{ type: "error", kind, message }` |
| `ToolDefinition` | `laipe_core::ToolDefinition` | `{ type: "function", function: { name, description, parameters } }` |
| `ToolFunction` | `laipe_core::ToolFunction` | `name` / `description` / `parameters: unknown` (JSON Schema) |
| `ToolCallInfo` | `laipe_core::ToolCallInfo` | Final, JSON-decoded tool call. |
| `ToolCallPartial` | `laipe_core::ToolCallPartial` | `{ index, id?, name?, arguments_delta }` |
| `ToolResult` | `laipe_core::ToolResult` | `{ tool_call_id, content }` |
| `ChatErrorKind` | `laipe_core::ChatErrorKind` | `"network"` / `"auth"` / `"model_not_found"` / `"bad_request"` / `"rate_limit"` / `"server_error"` / `"stream_protocol"` / `"unknown"` |
| `ChatErrorDiag` | `laipe_core::ChatErrorDiag` | Optional diagnostic context. |

### Runtime

| Symbol | Where | Purpose |
|---|---|---|
| `dispatchStream(config, messages, tools?, options?)` | `dispatch.ts` | The single streaming entry point. Yields `StreamEvent`s. Mirrors Rust `pick(fmt).dispatch(...)`. |
| `DispatchOptions` | `dispatch.ts` | `{ signal?: AbortSignal }`. |
| `SseParser` | `sse.rs` | Stateful SSE byte → frame parser (Rust parity). |
| `SseFrame` | `sse.rs` | One parsed SSE frame (event + data + id). |

### Errors

| Symbol | Where | Purpose |
|---|---|---|
| `ChatError` (class) | `errors.ts` | Runtime error wrapper with `kind` + `message` + `diag?`. |
| (kinds) | `ChatErrorKind` (above) | Same categorical kinds as Rust. |

---

## `laipe-vue` (TypeScript) — `packages/laipe-vue/`

Vue 3 components. The UI side. Layered as **primitives** (no state) → **composites** (compose primitives) → **batteries-included** (`AiChatPanel`).

```bash
bun add laipe-vue laipe-ts vue
```

### Components

| Symbol | Layer | File | Purpose |
|---|---|---|---|
| `AiChatPanel` | batteries-included | `AiChatPanel.vue` | One-line drop-in chat UI; owns `useConfig` / `useConversations` / `useChat` internally. |
| `ChatView` | composite | `composites/ChatView.vue` | Message list + input. Slots for `header`, `empty`, `message`, `message-actions`, `before-messages`, `after-messages`, `input-before`, `input-after`. |
| `Sidebar` | composite | `composites/Sidebar.vue` | Multi-conversation list. |
| `SettingsModal` | composite | `composites/SettingsModal.vue` | ProviderConfig form (Connection + Advanced). Slots: `model`, `extra`, `footer`. |
| `ConsolePanel` | composite | `composites/ConsolePanel.vue` | Debug log viewer. Reads from `useConsoleEntries()`. |
| `MessageBubble` | primitive | `primitives/MessageBubble.vue` | Single chat message. Slots: `default`, `actions`, `header`, `tool-calls`. |
| `MessageInput` | primitive | `primitives/MessageInput.vue` | Input row with send/stop. Slots: `before`, `after`. |
| `EmptyState` | primitive | `primitives/EmptyState.vue` | Onboarding state with sample prompts. |
| `IconButton` | primitive | `primitives/IconButton.vue` | Small icon-only button. |
| `ToolCallCard` | primitive | `primitives/ToolCallCard.vue` | One tool call (name + status + args + result). |

### Composables

| Symbol | File | Purpose |
|---|---|---|
| `useChat(source?, tools?)` | `useChat.ts` | Generic streaming composable. `source` defaults to `defaultStreamSource()`. `tools` can be a static array OR a getter `() => ToolDefinition[]` (for reactive tool lists). |
| `useConfig()` | `useConfig.ts` | Reactive `ProviderConfig` + `AgentSettings` singleton. Storage is swappable via `setConfigStorage`. |
| `useConversations()` | `useConversations.ts` | Reactive conversation list singleton. Persists to localStorage by default. |

### Streams (pluggable)

| Symbol | File | Purpose |
|---|---|---|
| `StreamSource` (interface) | `streams.ts` | `send(config, messages, tools?, options?) -> AsyncGenerator<StreamEvent>`. The pluggable interface for "where do chat events come from". |
| `tauriStream` | `streams.ts` | Production default: invokes Rust `chat` command, listens to Tauri events. |
| `fetchStream` | `streams.ts` | Browser fallback: calls `laipe-ts`'s `dispatchStream` directly (CORS-limited). |
| `mockStream` | `streams.ts` | Echoes the last user message; for tests / offline dev. |
| `defaultStreamSource()` | `streams.ts` | Auto-detect: Tauri in production, fetch in browser-only. |

### Storage (pluggable)

| Symbol | File | Purpose |
|---|---|---|
| `ConfigStorage` (interface) | `useConfig.ts` | `loadProviderConfig` / `saveProviderConfig` / `loadAgentSettings` / `saveAgentSettings`. |
| `localStorageConfig` | `useConfig.ts` | Default impl: synchronous localStorage. |
| `setConfigStorage(s)` | `useConfig.ts` | Replace the storage backend at runtime. |
| `whenConfigReady()` | `useConfig.ts` | Promise that resolves when the first load completes. |

### Console (pluggable singleton)

| Symbol | File | Purpose |
|---|---|---|
| `ConsoleEntry` | `console.ts` | One log line: `id` / `level` (`info`/`warn`/`error`) / `source` (`backend`/`frontend`) / `module` / `message` / `timestamp_ms`. |
| `ConsoleLevel` | `console.ts` | `"info"` / `"warn"` / `"error"`. |
| `ConsoleSource` | `console.ts` | `"backend"` / `"frontend"`. |
| `useConsoleEntries()` | `console.ts` | Reactive ref to the entries list. |
| `initConsole()` | `console.ts` | Pull snapshot + subscribe to backend events. Idempotent. |
| `clearConsole()` | `console.ts` | Clear all entries (frontend + backend). |
| `refreshConsole()` | `console.ts` | Force re-snapshot. |
| `installConsoleHook()` | `console.ts` | Hook `console.log` / `console.warn` / `console.error` so frontend logs land in the panel. Idempotent. |

### Types

| Symbol | File | Notes |
|---|---|---|
| `ChatStatus` | `composables` | Local `"idle" / "streaming"` (subset of the laipe-ts `ChatStatus`). |
| `Conversation` | `useConversations.ts` | `{ id, title, messages, createdAt }`. |
| `ConfigStorage` | `useConfig.ts` | Storage interface. |
| `AgentSettings` | `useConfig.ts` | Per-agent settings (currently just `enabledTools`). |

---

## `laipe-app` — `laipe-app/`

The starter app. **Not a library** — this is the demo that wires the framework together. Customization happens here, not in the libraries.

| Symbol | File | Purpose |
|---|---|---|
| `TOOLS` | `src/tools.ts` | The tool schema list passed to the LLM. Add a new tool: add an entry here + a `match` arm in `lib.rs::execute_tool`. |
| `MODEL_CATALOG` | `src/modelCatalog.ts` | Curated model list (id, name, supported effort). Used by `ModelSelector`. |
| `modelsForFormat(format)` | `src/modelCatalog.ts` | Models available for a given `ApiFormat`. |
| `findModel(id)` | `src/modelCatalog.ts` | Look up a model by id. |
| `cleanupModelId(id, maxLen?)` | `src/modelCatalog.ts` | Strip `vendor/` prefix + truncate for UI. |
| `ModelSelector` | `src/components/ModelSelector.vue` | Dropdown: curated list + custom + effort. |
| `ToolsSettings` | `src/components/ToolsSettings.vue` | Per-tool enable toggles. |

---

## Tauri commands (Rust) — `laipe-app/src-tauri/src/`

These are the IPC commands the Vue frontend can `invoke()`. The capability for invoking them lives in `laipe-app/src-tauri/capabilities/default.json`.

| Command | Args | Returns | Purpose |
|---|---|---|---|
| `chat` | `{ cfg, messages, tools, app, state }` | `()` | Stream a chat with the agent loop (up to `MAX_AGENT_TURNS`). |
| `cancel` | `{ state }` | `()` | Cancel the in-flight chat (idempotent). |
| `get_console_entries` | `{ state }` | `Vec<ConsoleEntry>` | Snapshot the in-memory console buffer. |
| `clear_console` | `{ state }` | `()` | Clear the in-memory console buffer. |

## Tauri events (Rust → TS)

| Event | Payload | Purpose |
|---|---|---|
| `chat:start` | `()` | Stream opened. |
| `chat:chunk` | `string` (text delta) | One text fragment. |
| `chat:tool_calls` | `ToolCallPartial[]` | One batch of tool-call partials (id / name / accumulating args). |
| `chat:done` | `()` | Stream finished cleanly. |
| `chat:error` | `{ kind: ChatErrorKind, message: string }` | Mid-stream error. |
| `chat:cancelled` | `()` | The `cancel` command was called. |
| `console:entry` | `ConsoleEntry` | One new console log line. |

---

## See also

- [`AGENTS.md`](AGENTS.md#pluggability--global-design-principle) — global design principles (pluggability, LLM friendliness, code style)
- [`ARCHITECTURE.md`](ARCHITECTURE.md) — crate boundaries, streaming pipeline, **pluggability seams map**
- [`EXTENDING.md`](EXTENDING.md) — fork-and-extend guide with two worked examples
- [`docs/PROTOCOLS.md`](docs/PROTOCOLS.md) — which protocol to use when
- [`docs/TOOL_CALLING.md`](docs/TOOL_CALLING.md) — cross-protocol tool schema translation
- [`docs/STREAMING.md`](docs/STREAMING.md) — the SSE pipeline
