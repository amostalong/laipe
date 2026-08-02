# Diagnostics — LLM-debuggable error reports

laipe ships a built-in **diagnostic layer** that makes errors inspectable
by LLM assistants. When something goes wrong (auth fail, rate limit,
network drop, SSE protocol violation), the user can hand their LLM a
single self-contained `.md` file and get a useful answer in one turn.

## Why this exists

The project is built to be **developed and extended with LLM assistance**
(see `AGENTS.md` — "LLM friendliness" global principle). That principle
applied to the runtime error path: when the app fails, an LLM should be
able to debug it without the user copy-pasting a 200-message thread and
hoping the right context survives.

The 4 anti-stutter countermeasures (in `ARCHITECTURE.md`) are about
**preventing** errors. This doc is about **debugging** them when they
happen anyway.

## The two layers

| Layer | What it is | Lives in | Lifetime |
|---|---|---|---|
| **In-memory console** | `ConsoleEntry` with structured context fields (kind, conv_id, turn, request_digest, response_digest, cause) | Tauri-managed `ConsoleState` | In-memory, max 1000 entries, lost on restart |
| **On-disk report** | Single self-contained `.md` file per error, plus the on-disk recording dir that backs it | Tauri-managed `DiagnosticsState` → `FileRecorder` | Persistent until user deletes |

The console is for **humans** (live UI feedback). The report is for
**LLM assistants** (paste a file, get a debug suggestion).

Both are populated by the same chat command; the recorder fires once
per chat turn regardless of outcome.

## How a chat turn flows

```
chat command invoked
  │
  ├─ DiagnosticsState builds RecordingContext { id, conv_id, turn, model, endpoint, format }
  │
  ├─ pick(format).dispatch(cfg, messages, tools, recorder, ctx)
  │     │
  │     ├─ recorder.record_request(ctx, redact(body))   ← before HTTP POST
  │     ├─ recorder.record_response_chunk(ctx, &b)     ← per network chunk
  │     └─ recorder.record_completion(ctx, outcome)    ← once, on Done/Error/Cancelled
  │
  ├─ StreamEvents yielded to UI
  │
  ├─ on Error:
  │     ├─ console.push_with_diag(...)   ← in-memory, structured
  │     └─ if auto_snapshot on: snapshot_error(...)   ← writes .md + INDEX.jsonl line
  │
  └─ emit chat:done / chat:error
```

## File layout

```
<app_log_dir>/
├── README-FOR-LLM.md        ← generated once; the LLM reads this first
├── INDEX.jsonl              ← one line per saved report; grep / jq this
├── recordings/
│   └── <rec-id>/
│       ├── request.json     ← exact request bytes (auth redacted)
│       ├── response.bin     ← raw response bytes, concatenated
│       └── meta.json        ← ctx + outcome (one JSON object)
└── reports/
    └── <ts>-<rec-id>.md     ← self-contained .md report for one error
```

`<app_log_dir>` is Tauri 2's `app_log_dir()`:
- Windows: `%LOCALAPPDATA%\dev.laipe.app\logs`
- macOS: `~/Library/Logs/dev.laipe.app`
- Linux: `~/.local/share/dev.laipe.app/logs`

## What's in a `.md` report

YAML frontmatter (ts, kind, model, conv_id, turn, endpoint) + sections:

- **Error** — the human message + the underlying cause string
- **Request** — the full request body, auth-redacted
- **Response** — the raw response bytes (truncated to the byte cap)
- **Likely causes** — the per-`ChatErrorKind` debug recipe from
  `laipe_core::ChatErrorKind::to_debug_hint()`
- **Conversation context** — the messages that led to the error

The report is one file per error. Bundling a full session into one
`.md` would blow the LLM's context window. One error = one file = one
LLM conversation.

## Pluggability (global design principle)

| Interface | Default impl | How to swap |
|---|---|---|
| `DiagnosticRecorder` (laipe-streaming) | `FileRecorder` (writes to disk) | Implement the trait; pass `Some(your_arc)` to `pick(fmt).dispatch(cfg, msgs, tools, recorder, ctx)`. |
| `DiagnosticConfig` (laipe-app) | `auto_snapshot=false, record_successful_rounds=false, max_report_bytes=5MiB` | Tauri command `set_diagnostic_mode` or the Settings-modal `DiagnosticsSettings` component. |

The trait is the seam. `NullRecorder` (the implicit no-op) is the
zero-cost option for consumers that don't want on-disk artifacts.

## What's NOT in scope

- **No session replay.** Each `.md` is one error, not a full timeline.
- **No telemetry / Sentry.** All data is local. Nothing is sent off-device.
- **No automatic redaction beyond auth headers.** The report is a **dev
  artifact** — users should review it before sharing. The auth redaction
  is best-effort (Bearer, x-api-key, JSON `api_key` field); a key
  pasted into a free-form system prompt will not be scrubbed.
- **No cloud sync.** Reports live in `app_log_dir()` only.

## For LLM assistants (the README-FOR-LLM.md content)

The README at `<app_log_dir>/README-FOR-LLM.md` is auto-generated on
first launch. It contains:

- The directory layout above.
- A per-`ChatErrorKind` debug recipe (8 sections: Network / Auth /
  ModelNotFound / BadRequest / RateLimit / ServerError / StreamProtocol /
  Unknown).
- Reading instructions: read `.md` first, cross-reference `INDEX.jsonl`
  for patterns, read raw `response.bin` for full upstream behavior.

When the user pastes a `.md` report (or the file path), start with the
"Error" + "Likely causes" sections. The recipe is hand-written for
LLM consumption — it tells you what to check first, in priority order.

## Settings UI

The `DiagnosticsSettings` component (in `laipe-app/src/components/`) is
the user-facing toggles. Three controls:

1. **Auto-snapshot every error** — writes a `.md` for every failed turn.
2. **Record every chat round** — saves request + response to disk for
   every turn, success or failure.
3. **Max bytes per report** — response cap (default 5 MiB).

The first two are off by default. Disk usage is bounded by `max_report_bytes`
per recording + manual cleanup of the `recordings/` and `reports/` dirs.

## Files touched

| File | Change |
|---|---|
| `crates/laipe-core/src/diagnostics.rs` | New — `ErrorContext` struct |
| `crates/laipe-core/src/error.rs` | Added `ChatErrorKind::as_str` / `label` / `to_debug_hint` / `ALL` |
| `crates/laipe-streaming/src/recorder.rs` | New — `DiagnosticRecorder` trait + `NullRecorder` + `FileRecorder` + `redact_request_bytes` |
| `crates/laipe-streaming/src/lib.rs` | `StreamChat::run` and `StreamChatDispatch::dispatch` gained `recorder: Arc<dyn DiagnosticRecorder>` and `&RecordingContext` params |
| `crates/laipe-streaming/src/{openai_chat,openai_responses,anthropic}.rs` | Wire `recorder.record_request` / `record_response_chunk` / `record_completion` at the right points |
| `laipe-app/src-tauri/src/console.rs` | `ConsoleEntry` gained 6 optional diagnostic fields + `ConsoleDiag` builder + `get_console_entry_by_id` command |
| `laipe-app/src-tauri/src/diagnostics.rs` | New — `DiagnosticsState` (Tauri-managed, swappable inner) + `DiagnosticConfig` + `snapshot_error` + `find_recording_for` + `write_readme_for_llm` |
| `laipe-app/src-tauri/src/lib.rs` | `chat` command takes `conversation_id`; holds recorder; auto-snapshots on error; new commands `get_diagnostic_mode` / `set_diagnostic_mode` / `dump_error_report` |
| `packages/laipe-vue/src/console.ts` | `ConsoleEntry` + `ChatErrorKind` + `DiagnosticConfig` types; `saveReport` / `getDiagnosticConfig` / `setDiagnosticConfig` APIs |
| `packages/laipe-vue/src/streams.ts` | `StreamSource.send` options gained `conversationId` |
| `packages/laipe-vue/src/composables/useChat.ts` | `send` takes optional `conversationId` |
| `packages/laipe-vue/src/components/composites/ConsolePanel.vue` | "Save report" action on each error row; transient banner showing the saved path |
| `laipe-app/src/components/DiagnosticsSettings.vue` | New — Settings toggles |
| `laipe-app/src/App.vue` | Wires `DiagnosticsSettings` into Settings; passes `currentId` to `send` |

## Versioning

`README-FOR-LLM.md` carries a `schema_version` line. When the report
schema changes in a way that requires an LLM to re-read the README,
bump `README_SCHEMA_VERSION` in `laipe-app/src-tauri/src/diagnostics.rs`
— the file regenerates on next app start.
