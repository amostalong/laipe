# AGENTS.md

**laipe** — *agent + llm + pipe* — an **agent client starter** (Rust + Vue + Tauri 2). A set of composable components you assemble to build your own LLM-powered agent desktop app.

Unifies OpenAI Chat Completions, OpenAI Responses, and Anthropic Messages behind a single `StreamEvent` shape. The stack is **fixed** (rust + vue + tauri) — what you customize is the **app's feature composition** (which Vue components you wire up, which slots you fill, which stream source you use).

See [`README.md`](../README.md) and [`VISION.md`](VISION.md) for the full pitch.

## Setup commands

- **MSRV:** Rust 1.70 (edition 2021). The workspace `rust-version` is the floor.
- **Node:** 20+ (used by Vite for the Vue build).
- Install deps:     `bun install && cargo build`
- Start dev:        `bun run dev:app`  (or `run-laipe-app.bat` on Windows)
- Build (release):  `cargo build --workspace --release` + `bun run build:app-fe`
- Test:             `cargo test --workspace`  *(must be green before push; 25 tests in `laipe-streaming`)*
- Lint:             `cargo clippy --workspace --all-targets -- -D warnings`
- Format:           `cargo fmt`
- Run all gates:    `bun run gates`  (fmt + clippy + test + typecheck + build)

## Project layout

```
crates/                            Rust protocol components (libraries)
├── laipe-core/                    Protocol-agnostic types. Zero HTTP/async deps.
├── laipe-streaming/               3-protocol SSE: openai_chat / openai_responses / anthropic.
└── laipe-tokio/                   CancelHandle + run_to_completion runtime glue.

packages/                          TypeScript component layer (libraries)
├── laipe-ts/                      TS SDK: types + SseParser + dispatchStream. No UI.
└── laipe-vue/                     Vue 3 components — primitives + composites + AiChatPanel.
    └── src/
        ├── streams.ts                 StreamSource interface + tauriStream / fetchStream / mockStream
        ├── composables/               useChat, useConfig, useConversations
        └── components/
            ├── primitives/            MessageBubble, MessageInput, EmptyState, IconButton
            ├── composites/            ChatView, Sidebar, SettingsModal
            └── AiChatPanel.vue        One-line drop-in

laipe-app/                         The starter app (Tauri 2 desktop)
├── src/                           Vue 3 frontend (showcases custom composition)
│   └── App.vue                    Uses ChatView + Sidebar + SettingsModal + useChat(tauriStream)
├── src-tauri/                     Rust backend (Tauri 2 commands)
│   └── src/lib.rs                 #[tauri::command] chat + cancel, calls laipe_streaming
└── tauri.conf.json                Tauri config (windows, bundle, dev URL)

docs/                              Design + spec docs
└── PROTOCOLS.md, STREAMING.md, TOOL_CALLING.md
```

## Code style

- **rustfmt** (`rustfmt.toml`): max width 100, 4-space tabs, Unix newlines, reorder imports + modules.
- **clippy** (`clippy.toml`): stricter than default — `cognitive-complexity-threshold = 25`, `too-many-arguments-threshold = 7`, `type-complexity-threshold = 250`.
- **Workspace lints** (`Cargo.toml` `[workspace.lints]`): `unsafe_code = "forbid"`, `unused_must_use = "deny"`, `missing_docs = "allow"` (until v1.0), all `clippy::all = "warn"`.
- All public traits use `#[async_trait]` — do **not** introduce native AFIT mid-v0.
- **TypeScript**: strict mode, `noUncheckedIndexedAccess: true`. `vue-tsc --noEmit` for typecheck.
- Run `bun run gates` before committing.

## Pluggability — global design principle

**All design decisions should treat components as pluggable.** A pluggable component has a stable, documented interface, accepts its dependencies through props/context, and can be replaced by another implementation without modifying its consumers. Apply this to every new module and every change to existing modules.

### What this means concretely

**Do**:
- Define an **interface** (trait, type, or interface) for any module that talks to the outside world. Implement the interface. Consumers depend on the interface, not the implementation.
- Inject dependencies through **constructor args, function args, or composable params** — not through module-level singletons or globals.
- Accept a **storage adapter / transport / driver** when the module persists state, makes network calls, or reads from disk. Default to the simplest adapter; allow the consumer to swap.
- Expose **slots, props, and events** on UI components. Use the slot pattern for structural variation, props for data, events for output.
- Register pluggable items in a **typed registry** (e.g. `StreamSource`, the tool catalog, the model catalog) so consumers can add, list, and pick.
- Document the **extension surface** in a comment at the top of the module: "to replace X, implement Y and pass it to Z."

**Don't**:
- Hardcode a backend inside a UI component (e.g. a component that imports `tauriStream` directly).
- Reach for `localStorage` / `fetch` / `invoke` from inside a primitive. Let the consumer provide the storage / transport.
- Use a singleton state without a way to reset or replace it.
- Couple a composable to a specific `StreamSource` instance. Take it as a param.
- Bake tool / model / provider logic into a single match. When a list grows past 2-3, switch to a registry.

### Plug-in points already in the framework

| Layer | Interface | Default impl | How to extend |
|---|---|---|---|
| `StreamSource` (TS) | `streams.ts: StreamSource` | `tauriStream` / `fetchStream` / `mockStream` | Implement the interface; pass to `useChat(source)` |
| Protocol dispatcher (Rust) | `StreamChatDispatch` | `openai_chat` / `openai_responses` / `anthropic` | Implement `StreamChat`; register in `pick()` |
| Tool execution (Rust) | `execute_tool(name, args)` in `lib.rs` | `get_current_time`, `echo` | Add a `match` arm in `execute_tool` + a schema in `laipe-app/src/tools.ts` |
| Config storage (TS) | `ConfigStorage` (new) | `localStorage` | Pass a different storage to `useConfig(storage)` |
| Console transport (TS) | `ConsoleEntry` push (singleton) | Tauri events + `console.log` hook | Replace `console.ts` with a custom transport for browser-only / HTTP logging |

### When to introduce a new plug-in point

When you find yourself reaching for a 3rd hardcoded implementation of the same thing — that's the signal. Add an interface, take it as a param, and keep the default behavior unchanged. Examples that would warrant this in v0.2+:
- Multiple LLM clients with different auth flows → `LlmAuth` interface.
- Multiple conversation backends (localStorage / SQLite / cloud) → `ConversationStorage` interface.
- Multiple message renderers (markdown / code / canvas) → `MessageRenderer` interface.

For the starter, **don't over-pluggify**. Each new interface is a thing consumers must learn. The bar is: *would the next person extending this need a different implementation, and is that work non-trivial?* If yes, add the seam. If no, hardcode it.

## LLM friendliness — global design principle

**The project is built to be developed and extended with LLM assistance.** Every file, function, and module should be straightforward for a code-generating LLM to read, modify, and extend. Apply this to every new file and every change to existing files.

### What this means concretely

**Do**:
- Put a **file-header comment** at the top of every non-trivial file: *what it is, why it exists, how to extend it, who depends on it*. Three to ten lines. Same shape in every file (`//!` in Rust, `//` in TS) so the LLM recognizes the pattern.
- Document every **public symbol** with a doc comment (`///` in Rust, `/** */` or `//` above in TS). The doc answers: *what does it do, what does it return, what's the typical caller, what can go wrong*.
- Use **stable, predictable naming**. Pick one verb for one concept (`pick` to select a streamer, `dispatch` to start a stream, `execute_tool` to run a tool — never mix with `select`/`run`/`invoke` for the same idea). One name, one meaning, across both Rust and TS.
- Keep a **single source of truth per concept**. The Rust `ProviderConfig` and the TS `ProviderConfig` are mirrors — document the mirror in the TS header. Don't define `ToolDefinition` in two places.
- **Show, don't describe**. Every doc comment has a code example. Every "How do I…" section in `EXTENDING.md` has a runnable snippet.
- Keep the **file layout stable**. New files go where existing files of the same role live. Don't reorganize without a migration note.
- Use **explicit types / signatures** everywhere. No `any`, no `as unknown as X`, no implicit conversions. An LLM should be able to read the type and know the shape.
- Make **error messages actionable**. `unknown tool: get_current_time (registered: get_current_time, echo)` is better than `unknown tool`.

**Don't**:
- **No clever metaprogramming**. No `Proxy`, no dynamic class generation, no Rust macros that hide logic. The LLM reads the source — keep the source readable.
- **No scattered copies of the same type / constant**. If the LLM sees `MAX_AGENT_TURNS` in two files, it doesn't know which to trust.
- **No 1000-line files**. If a file is hard for a human to scan, an LLM is worse. Split by responsibility.
- **No silent failures**. Every `catch {}` should log or at least be commented. Every `?` should bubble to a place that explains.
- **No undocumented public APIs**. If it's exported, it's part of the contract; the LLM treats it as one. Document or hide.
- **No magic strings for things that should be enums**. Use the existing `ApiFormat`, `EffortLevel`, `ChatRole` enums. Don't pass `"openai_chat"` as a string literal scattered across files.

### Pointers for an LLM picking up the codebase

When you're (an LLM) extending laipe, the navigation order is:

1. **[`README.md`](../README.md)** — pitch, status, what laipe is and isn't.
2. **[`VISION.md`](VISION.md)** — scope, non-goals, target audience.
3. **[`AGENTS.md`](AGENTS.md)** *(this file)* — global design principles (pluggability, LLM friendliness, code style, testing).
4. **[`ARCHITECTURE.md`](ARCHITECTURE.md)** — crate boundaries, streaming pipeline, anti-stutter, **pluggability seams map**.
5. **[`EXTENDING.md`](EXTENDING.md)** — fork-and-extend guide with two worked examples (plot-writer, finboard) and a **pluggability reference table**.
6. **[`API.md`](API.md)** — single source of truth for the public API surface. Start here when looking for "where is X defined / what calls Y".
7. **The file you're changing** — every file has a header comment explaining its role.

**To add a feature** (a tool, a UI component, a persistence layer), the answer is almost always the same shape: (1) add the schema/interface, (2) register it in the right place, (3) add a UI hook, (4) write a test. The exact "right place" is in the [Pluggability seams map](ARCHITECTURE.md#pluggability--where-the-seams-are) and the [Pluggability table](EXTENDING.md#pluggability).

**To fix a bug** — find the symbol in [`API.md`](API.md), read its doc comment, follow the call graph from there. The 25 tests in `laipe-streaming` are the floor for what to add to verify your fix.

## Testing instructions

- Unit tests: `cargo test --workspace` (Rust) + `bun run typecheck:ts` (TypeScript).
- The 25 existing tests in `laipe-streaming` are the **floor** for new public API. Add a test for every new behavior in the same crate.
- No vitest / vue-test-utils yet. v0.4+ work.
- All tests must pass before opening a PR — `bun run gates` is the de-facto gate (no CI yet, see `CONTRIBUTING.md`).

## PR & commit conventions

- Branch from `master`; never push to it directly. Branch naming: `fix/<one-word>` or `feat/<one-word>`.
- **Conventional Commits** (lightly):
  - Types: `feat` / `fix` / `refactor` / `docs` / `test` / `chore` / `perf`
  - Subject: under 70 chars, no period, present tense (`add`, not `added`).
  - Body: explain the *why*, not the *what*.
- One logical change per commit. Don't bundle "fix typo" with "rewrite parser".
- For multi-line messages, write to a file and use `git commit -F` (PowerShell 5.1: single-quote the subject, escape inner quotes — see `CONTRIBUTING.md`).
- No CI until v0.5.

## Security

- **Never commit secrets.** API keys (`OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, etc.) must be read at runtime from env vars or stored in OS keyring (Tauri 2 supports `tauri-plugin-stronghold`).
- In `laipe-app`, keys live in Rust process memory + `localStorage` for the demo. Production should use the OS keyring.
- `.env`, `.env.local`, `*.local` are in `.gitignore`. Don't override.
- The workspace `Cargo.lock` is **pinned** — do not run `cargo update` unless explicitly fixing a security advisory.
- `bun.lock` is also pinned — `bun install` syncs from `package.json` without updating transitive deps.
- New third-party dependencies need a strong reason: the 11 deps in workspace `Cargo.toml` are the floor; new deps multiply the audit surface.

## Pointers

- Position / scope / "what laipe is and isn't" → [`VISION.md`](VISION.md)
- Crate boundaries, streaming pipeline, anti-stutter tricks, tool-calling flow → [`ARCHITECTURE.md`](ARCHITECTURE.md)
- v0.1 → v1.0 plan, what's deliberately deferred → [`ROADMAP.md`](ROADMAP.md)
- Per-version what changed → [`CHANGELOG.md`](CHANGELOG.md)
- Pre-community contribution ground rules → [`CONTRIBUTING.md`](CONTRIBUTING.md)
- 3-protocol comparison + when to pick which → [`docs/PROTOCOLS.md`](docs/PROTOCOLS.md)
