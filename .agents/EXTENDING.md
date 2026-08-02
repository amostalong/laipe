# Extending laipe — from starter to a real agent

This is the **fork-and-extend guide** for `laipe-app`. It walks you through turning the starter into a production agent, with two worked examples (a fiction-writing agent and a financial-dashboard agent) showing the same patterns applied to two very different domains.

If you only have 5 minutes, read [The 4-layer model](#the-4-layer-model) and [Step 1](#step-1-pick-your-fork-anchor). If you have 30, read the whole thing and the [Plot-writer walkthrough](#worked-example-1-plot-writer-agent) or the [Finboard walkthrough](#worked-example-2-finboard-agent) — whichever is closer to your domain.

---

## The 4-layer model

`laipe` is built on the principle that **the agent shape is yours; the protocol plumbing is ours**. Concretely, the codebase is split into four layers, each of which you either *use as-is* or *replace wholesale*. Mixing-and-matching is the whole point.

| # | Layer | What it is | What you do |
|---|---|---|---|
| 0 | **Protocol** (`laipe-core`, `laipe-streaming`) | OpenAI Chat / Responses / Anthropic wire formats, SSE parsing, tool schema translation, cancel handles. | **Don't touch.** Lives in `crates/`. You only fork this if you're adding a new LLM provider (and even then, the extension point is `pick(fmt)` — just implement `StreamChat`). |
| 1 | **Component kit** (`laipe-vue`) | Vue 3 primitives (MessageBubble, ToolCallCard, MessageInput, EmptyState, IconButton), composites (ChatView, Sidebar, SettingsModal), batteries-included (AiChatPanel), and a `StreamSource` interface that swaps between Tauri IPC, browser fetch, and a mock. | **Compose.** Import what you want, replace what you don't, use the slots to override anything in between. |
| 2 | **App shell** (`laipe-app/src/App.vue`) | The wiring: which `StreamSource` to use, which tools to expose, what state lives in `useConversations` and `useConfig`, the layout chrome. | **Customize freely.** This is *your* app's spine. |
| 3 | **Tool execution** (`laipe-app/src-tauri/src/lib.rs`) | The agent loop. Calls `laipe_streaming::pick(fmt).dispatch(...)`, drains the stream into Tauri events, and — when the LLM emits `tool_calls` — runs them in Rust, appends the results, and re-dispatches. Up to `MAX_AGENT_TURNS`. | **Add tools here.** This is where *your* agent's value lives. |

> **Mental model**: layer 0 is the network driver, layer 1 is the UI kit, layer 2 is the app glue, layer 3 is your product. Forks normally touch only 2 and 3.

### What stays locked

- **The stack**: Rust + Vue 3 + Tauri 2. You don't swap Tauri for Electron, Vue for React, or Rust for Go. (If you want a different stack, fork the whole repo — but then you're not extending laipe, you're forking it.)
- **The wire format**: 3-protocol SSE (OpenAI Chat / OpenAI Responses / Anthropic). Add a new protocol in layer 0 only.
- **The `StreamEvent` shape**: `text | tool_calls | done | error`. Consumers can rely on this.
- **The `ChatMessage` shape**: `role / content / tool_call_id? / tool_calls?`. The Rust ↔ TS serde is locked.

### What you customize

- The tools your agent exposes (layer 3 + `src/tools.ts`).
- The state shape (add fields to `Conversation` or `ProviderConfig` in `useConversations` / `useConfig`).
- The UI layout (slot overrides, custom primitives, your own App.vue chrome).
- The persistence backend (replace `localStorage` with Tauri commands writing to `app_data_dir()`).
- The Rust-side capabilities (add `tauri-plugin-fs`, `tauri-plugin-shell`, `tauri-plugin-stronghold` as your tools need them).

---

## Step 1: Pick your fork anchor

Before you write a line of code, answer these three questions:

1. **What does the LLM get to do?** This is your tool list. Aim for 3-8 tools, each with a clear single responsibility. More than ~10 tools and the LLM starts hallucinating which one to call.
2. **What state lives between turns?** Conversation history (built-in), but also: project files, watchlists, scratchpads, knowledge bases, user preferences. Decide what gets persisted and where.
3. **What runs in the webview vs in Rust?** Rule of thumb: anything that touches the network, filesystem, or native APIs goes in Rust. UI state, formatting, and `localStorage` go in TS.

If you can write down a one-paragraph answer to all three, you're ready for step 2.

---

## Step 2: Add the tool schemas (TypeScript)

For each tool, you need a `ToolDefinition`. Put them in a single `src/tools.ts` so the LLM sees a coherent set:

```ts
// src/tools.ts
import type { ToolDefinition } from "laipe-ts";

export const TOOLS: ToolDefinition[] = [
  {
    type: "function",
    function: {
      name: "get_quote",
      description: "Get the current price and daily change for a ticker symbol.",
      parameters: {
        type: "object",
        properties: {
          symbol: { type: "string", description: "Ticker, e.g. 'AAPL' or 'BTC-USD'." },
        },
        required: ["symbol"],
      },
    },
  },
  // ... more tools
];
```

**Tips:**

- The `description` is *the most important field*. The LLM reads it to decide when to call. If the description is vague, the tool gets called at the wrong time.
- `parameters` follows [JSON Schema](https://json-schema.org/). The OpenAI Chat format requires `type: "object"` at the root. laipe flattens this for Anthropic / Responses automatically.
- Keep names `snake_case` — they end up in Rust function names and JSON keys.

## Step 3: Add the tool implementations (Rust)

In `src-tauri/src/lib.rs`, find the `execute_tool` function and add a new arm to the match:

```rust
fn execute_tool(name: &str, args_json: &str) -> String {
    match name {
        "get_quote" => {
            // Parse args
            let args: serde_json::Value = serde_json::from_str(args_json)
                .unwrap_or_else(|_| serde_json::json!({}));
            let symbol = args["symbol"].as_str().unwrap_or("");

            // Do the work — could be reqwest::get, a database query,
            // a local file read, anything.
            let price = fetch_price_sync(symbol);  // your impl

            serde_json::json!({
                "symbol": symbol,
                "price": price,
            })
            .to_string()
        }
        "get_current_time" => { /* ... existing ... */ }
        "echo" => { /* ... existing ... */ }
        _ => serde_json::json!({ "error": format!("unknown tool: {name}") }).to_string(),
    }
}
```

**Tips:**

- Return JSON. The whole content of a `role: tool` message is a string, but the convention is JSON. laipe doesn't parse it for you — the LLM does.
- If a tool needs to be async (network, DB), make `execute_tool` async and `tokio::spawn_blocking` anything that blocks. The agent loop already runs inside an `async` context.
- For tools that need to remember state between calls (e.g. a watchlist), use `tauri::State` or pass a shared `Arc<Mutex<...>>` into `execute_tool`. See the [Finboard walkthrough](#worked-example-2-finboard-agent).

## Step 4: Wire the tools into App.vue

```ts
// src/App.vue
import { useChat, tauriStream } from "laipe-vue";
import { TOOLS } from "./tools";

const { status, send, cancel } = useChat(tauriStream, TOOLS);
```

That's it for the agent loop — `useChat` forwards `tools` to `tauriStream`, which sends them to the Rust `chat` command, which passes them to `laipe_streaming::pick(fmt).dispatch(cfg, messages, tools)`.

## Step 5: Run the agent loop (Rust)

`src-tauri/src/lib.rs` already has the agent loop. Two settings to think about:

- **`MAX_AGENT_TURNS`**: how many LLM → tool → LLM cycles per user turn. Default is 4. Increase if your agent legitimately chains many tools; decrease if you want snappier responses. There's no good automatic answer.
- **Cancellation**: the `cancel` command kills the in-flight request mid-stream. If your tools are long-running (file scan, network), make them check the `CancelHandle` too — pass it into `execute_tool` and `tokio::select!` on it.

## Step 6: Render tool calls in the UI

`MessageBubble` already renders `message.tool_calls[i]` via `ToolCallCard` by default. If the default card isn't enough, override the slot:

```vue
<ChatView :messages="messages" :status="status" @send="onSend" @cancel="onCancel">
  <template #message="{ message }">
    <MessageBubble :message="message">
      <template #tool-calls="{ calls }">
        <QuoteCard v-for="c in calls" :key="c.id" :symbol="JSON.parse(c.function.arguments).symbol" />
      </template>
    </MessageBubble>
  </template>
</ChatView>
```

The `tool-calls` slot receives the `AssistantToolCall[]` array; you decide how to render it.

## Step 7: Persist state

The starter ships with `useConfig` / `useConversations` persisting to `localStorage` via a swappable `ConfigStorage` adapter. For production, write a Tauri-backed adapter and call `setConfigStorage(...)` once at app startup:

```ts
// laipe-app/src/storage.ts
import { invoke } from "@tauri-apps/api/core";
import type { ConfigStorage, AgentSettings, ProviderConfig } from "laipe-vue";

export const tauriConfigStorage: ConfigStorage = {
  async loadProviderConfig() {
    try {
      return await invoke<ProviderConfig | null>("load_config");
    } catch {
      return null;
    }
  },
  async saveProviderConfig(c) {
    await invoke("save_config", { cfg: c });
  },
  async loadAgentSettings() {
    try {
      return await invoke<AgentSettings | null>("load_agent_settings");
    } catch {
      return null;
    }
  },
  async saveAgentSettings(s) {
    await invoke("save_agent_settings", { settings: s });
  },
};
```

```ts
// laipe-app/src/main.ts
import { setConfigStorage, whenConfigReady, installConsoleHook, initConsole } from "laipe-vue";
import { tauriConfigStorage } from "./storage";

setConfigStorage(tauriConfigStorage);
await whenConfigReady();          // optional — wait for the first load before mounting
installConsoleHook();
initConsole();
createApp(App).mount("#app");
```

The Rust side adds the matching `load_config` / `save_config` commands (see [Step 7 of the original pattern](#step-7-persist-state-legacy) for the canonical implementation).

For API keys, use `tauri-plugin-stronghold` (OS keyring). Never commit a key.

## Step 8: Mobile (optional)

Tauri 2 compiles the same codebase to iOS / Android. The only changes:

- `tauri.conf.json`: add `bundle.iOS` and `bundle.android` sections.
- Tooling: `rustup target add aarch64-apple-ios` (iOS) and `aarch64-linux-android` (Android), plus the matching platform toolchain (Xcode / Android SDK + NDK).
- The webview is the same — but on iOS, the keyboard, safe-area, and viewport units behave slightly differently. Add `@capacitor/status-bar`-style logic if you need it.

For mobile, you also usually want to lock the LLM endpoint to HTTPS-only and ensure your tool implementations don't block the UI thread.

## Step 9: Ship

- `cargo tauri build -p laipe-app` → MSI / DMG / AppImage.
- For mobile, sign + upload via Transporter (iOS) or Play Console (Android).
- For auto-update, add `tauri-plugin-updater`.

## Step 10: Iterate

The starter is intentionally tiny. Add things in this order:

1. ✅ Tool calling (you're here)
2. Per-conversation system prompts
3. Multi-conversation search (`useConversations` adds a `tags: string[]` field)
4. Per-tool permissions (some tools need user confirmation before running)
5. Streaming tool results (tool emits `chat:tool_progress` events while running)
6. Background sub-agents (`tokio::spawn` to run a long task; emit progress events)
7. RAG: tools that search a local index, inject the top-k into the prompt
8. Voice I/O (Web Speech API on the frontend; Whisper / TTS on the backend)

---

## Worked example 1: Plot-writer agent

**Domain**: a fiction-writing agent. The user says "write a mystery set in 1920s Shanghai" and the agent calls tools to register characters, locations, scenes, and plot beats as it composes the story. The Rust backend writes everything to `app_data_dir()/plots/{title}.json`.

### Step 1 — The tool list

Five tools, each with a single responsibility:

```ts
// src/tools.ts
import type { ToolDefinition } from "laipe-ts";

export const TOOLS: ToolDefinition[] = [
  {
    type: "function",
    function: {
      name: "create_plot",
      description: "Start a new plot. Call this first, before any other tool, when the user wants to write something new.",
      parameters: {
        type: "object",
        properties: {
          title: { type: "string" },
          genre: { type: "string", description: "e.g. 'mystery', 'romance', 'sci-fi'" },
          setting: { type: "string", description: "Time and place, e.g. '1920s Shanghai'" },
        },
        required: ["title", "genre", "setting"],
      },
    },
  },
  {
    type: "function",
    function: {
      name: "add_character",
      description: "Add a character to the current plot.",
      parameters: {
        type: "object",
        properties: {
          name: { type: "string" },
          role: { type: "string", description: "e.g. 'protagonist', 'antagonist', 'witness'" },
          description: { type: "string", description: "1-2 sentence character sketch" },
        },
        required: ["name", "role", "description"],
      },
    },
  },
  {
    type: "function",
    function: {
      name: "add_scene",
      description: "Add a scene to the current plot. Scenes should advance the story; aim for 3-8 per plot.",
      parameters: {
        type: "object",
        properties: {
          title: { type: "string" },
          setting: { type: "string", description: "Where/when the scene takes place" },
          characters: { type: "array", items: { type: "string" }, description: "Character names present" },
          summary: { type: "string", description: "1-2 sentence plot summary of what happens" },
        },
        required: ["title", "setting", "summary"],
      },
    },
  },
  {
    type: "function",
    function: {
      name: "summarize_plot",
      description: "Get the current state of the plot: title, characters, scenes so far. Use this when you need to recall details before continuing.",
      parameters: { type: "object", properties: {}, required: [] },
    },
  },
  {
    type: "function",
    function: {
      name: "list_plots",
      description: "List all plots the user has written.",
      parameters: { type: "object", properties: {}, required: [] },
    },
  },
];
```

### Step 2 — The Rust state

```rust
// src-tauri/src/lib.rs (additions)
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Default)]
struct AppState {
    cancel: Arc<Mutex<Option<CancelHandle>>>,
    // One Plot per conversation. Keyed by conversation id (you pass
    // this in as part of the chat command, or via a separate setter).
    plots: Arc<RwLock<HashMap<String, Plot>>>,
}

#[derive(Default, Clone, Serialize, Deserialize)]
struct Plot {
    title: String,
    genre: String,
    setting: String,
    characters: Vec<Character>,
    scenes: Vec<Scene>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Character { name: String, role: String, description: String }

#[derive(Clone, Serialize, Deserialize)]
struct Scene { title: String, setting: String, characters: Vec<String>, summary: String }
```

### Step 3 — The tool implementations

```rust
fn execute_tool(
    name: &str,
    args_json: &str,
    state: &AppState,
    conv_id: &str,
    app: &AppHandle,
) -> String {
    let args: serde_json::Value = serde_json::from_str(args_json).unwrap_or_else(|_| json!({}));
    let mut plots = state.plots.blocking_write();  // brief sync hold is fine for small plots

    match name {
        "create_plot" => {
            let plot = Plot {
                title: args["title"].as_str().unwrap_or("").to_string(),
                genre: args["genre"].as_str().unwrap_or("").to_string(),
                setting: args["setting"].as_str().unwrap_or("").to_string(),
                ..Default::default()
            };
            plots.insert(conv_id.to_string(), plot.clone());
            // Persist to disk
            let _ = save_plot(app, conv_id, &plot);
            json!({ "ok": true, "title": plot.title }).to_string()
        }
        "add_character" => {
            let Some(plot) = plots.get_mut(conv_id) else {
                return json!({ "error": "no active plot" }).to_string();
            };
            plot.characters.push(Character {
                name: args["name"].as_str().unwrap_or("").to_string(),
                role: args["role"].as_str().unwrap_or("").to_string(),
                description: args["description"].as_str().unwrap_or("").to_string(),
            });
            let _ = save_plot(app, conv_id, plot);
            json!({ "ok": true, "total_characters": plot.characters.len() }).to_string()
        }
        "add_scene" => { /* same shape */ todo!() }
        "summarize_plot" => {
            let Some(plot) = plots.get(conv_id) else {
                return json!({ "error": "no active plot" }).to_string();
            };
            serde_json::to_string(plot).unwrap_or_default()
        }
        "list_plots" => {
            let names: Vec<&str> = plots.values().map(|p| p.title.as_str()).collect();
            json!({ "plots": names }).to_string()
        }
        _ => json!({ "error": format!("unknown tool: {name}") }).to_string(),
    }
}

fn save_plot(app: &AppHandle, conv_id: &str, plot: &Plot) -> Result<(), String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("plots");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{conv_id}.json"));
    std::fs::write(&path, serde_json::to_string_pretty(plot).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
```

(Note: the `blocking_write` is fine for tiny in-memory state. For larger plots, switch to `tokio::sync::RwLock` and `await`. And for the disk write, use the `std::fs::write` + `spawn_blocking` pattern from [the PlotCraft battle-test](https://github.com/amostalong/plotcraft) — `tokio::fs::write` + immediate `rename` is racy on Windows.)

### Step 4 — System prompt

The LLM needs a strong persona. Add it via `useConfig` (so the user can edit it) or hard-code it in `App.vue`:

```ts
// App.vue (or a composable)
const SYSTEM_PROMPT = `You are a fiction-writing assistant. When the user wants to start a new story, call create_plot first. Then progressively add characters and scenes, narrating the choices you make. After every 2-3 tool calls, write a short narrative passage that brings the story to life. Use the summarize_plot tool to recall details if you lose track.`;
```

In `useChat`'s call site, prepend the system message to the conversation before calling `send`:

```ts
const next = [
  { role: "system" as const, content: SYSTEM_PROMPT },
  ...messages.value,
  { role: "user" as const, content: text },
];
await send(config.value, next);
```

### Step 5 — Conversation id as tool scope

To make the `conv_id` work, add it to the `chat` command:

```rust
#[tauri::command]
async fn chat(
    cfg: ProviderConfig,
    messages: Vec<ChatMessage>,
    tools: Option<Vec<ToolDefinition>>,
    conv_id: String,                              // ← new
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // pass &conv_id to execute_tool via the agent loop
}
```

In `useChat` (or your App.vue), pass the active conversation's id when sending.

### What you get

A 200-line Tauri app where the user can say "write a murder mystery set on a 1920s ocean liner" and watch the LLM:

1. Call `create_plot` (with the user-confirmed title).
2. Call `add_character` 3-4 times for the suspects.
3. Call `add_scene` 5-6 times as the story unfolds.
4. Intersperse narrative prose between tool calls.

Every tool call shows up as a `ToolCallCard` in the chat, with a button to expand and see the args. The plot file persists to disk so a `cargo tauri dev` reload doesn't lose work.

---

## Worked example 2: Finboard agent

**Domain**: a financial dashboard. The LLM can pull live quotes, manage a watchlist, and answer questions like "what's happening with my tech stocks today?" by calling tools that hit a stock API and a local SQLite store.

### Different shape from the plot-writer

The interesting bit here is **mixing real-time external data with persistent local state**:

- `get_quote` is a pure read — calls a stock API in Rust, returns the result, no persistence.
- `add_to_watchlist` mutates per-user state — appends to a SQLite table.
- `summarize_watchlist` joins the two — pulls current quotes for every ticker in the watchlist.

This shows the pattern of `reqwest` (external) + `rusqlite` (local) + `tauri::State` (shared) all living in `execute_tool`.

### The tool list (TS)

```ts
// src/tools.ts
export const TOOLS: ToolDefinition[] = [
  {
    type: "function",
    function: {
      name: "get_quote",
      description: "Get the current price and daily change for a ticker symbol (e.g. 'AAPL', 'BTC-USD').",
      parameters: {
        type: "object",
        properties: { symbol: { type: "string" } },
        required: ["symbol"],
      },
    },
  },
  {
    type: "function",
    function: {
      name: "add_to_watchlist",
      description: "Add a ticker to the user's watchlist.",
      parameters: {
        type: "object",
        properties: {
          symbol: { type: "string" },
          note: { type: "string", description: "Optional note, e.g. 'bought at 180'" },
        },
        required: ["symbol"],
      },
    },
  },
  {
    type: "function",
    function: {
      name: "remove_from_watchlist",
      description: "Remove a ticker from the user's watchlist.",
      parameters: {
        type: "object",
        properties: { symbol: { type: "string" } },
        required: ["symbol"],
      },
    },
  },
  {
    type: "function",
    function: {
      name: "summarize_watchlist",
      description: "Get the current watchlist with live prices for each ticker. Use this when the user asks 'how are my stocks doing' or anything watchlist-wide.",
      parameters: { type: "object", properties: {}, required: [] },
    },
  },
];
```

### The Rust state + DB

```rust
// src-tauri/src/lib.rs
use rusqlite::{params, Connection};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Default)]
struct AppState {
    cancel: Arc<Mutex<Option<CancelHandle>>>,
    db: Arc<Mutex<Connection>>,  // SQLite, opened in setup()
}

// Called from tauri Builder.setup
fn open_db(app: &AppHandle) -> Result<Connection, String> {
    let path = app.path().app_data_dir().map_err(|e| e.to_string())?.join("finboard.db");
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    let conn = Connection::open(&path).map_err(|e| e.to_string())?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS watchlist (
            symbol TEXT PRIMARY KEY,
            note TEXT,
            added_at INTEGER NOT NULL
        )",
    ).map_err(|e| e.to_string())?;
    Ok(conn)
}
```

### The tool implementations (async this time)

```rust
async fn execute_tool(
    name: &str,
    args_json: &str,
    state: &AppState,
) -> String {
    let args: serde_json::Value = serde_json::from_str(args_json).unwrap_or_else(|_| json!({}));

    match name {
        "get_quote" => {
            let symbol = args["symbol"].as_str().unwrap_or("");
            match fetch_quote(symbol).await {
                Ok(q) => json!({ "symbol": symbol, "price": q.price, "change_pct": q.change_pct }).to_string(),
                Err(e) => json!({ "error": e.to_string() }).to_string(),
            }
        }
        "add_to_watchlist" => {
            let symbol = args["symbol"].as_str().unwrap_or("").to_uppercase();
            let note = args["note"].as_str().unwrap_or("");
            let conn = state.db.lock().await;
            let res = conn.execute(
                "INSERT OR REPLACE INTO watchlist (symbol, note, added_at) VALUES (?, ?, ?)",
                params![symbol, note, chrono::Utc::now().timestamp()],
            );
            match res {
                Ok(_) => json!({ "ok": true, "symbol": symbol }).to_string(),
                Err(e) => json!({ "error": e.to_string() }).to_string(),
            }
        }
        "remove_from_watchlist" => { /* same shape, DELETE */ todo!() }
        "summarize_watchlist" => {
            let conn = state.db.lock().await;
            let mut stmt = conn.prepare("SELECT symbol, note FROM watchlist ORDER BY added_at DESC").unwrap();
            let rows: Vec<(String, String)> = stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .unwrap()
                .filter_map(|r| r.ok())
                .collect();
            drop(stmt);  // release the lock before the awaits below

            // Now fetch quotes concurrently
            let quotes: Vec<_> = futures::future::join_all(
                rows.iter().map(|(sym, _)| async move {
                    let q = fetch_quote(sym).await.ok();
                    json!({ "symbol": sym, "quote": q })
                })
            ).await;

            json!({ "watchlist": quotes }).to_string()
        }
        _ => json!({ "error": format!("unknown tool: {name}") }).to_string(),
    }
}

async fn fetch_quote(symbol: &str) -> Result<Quote, String> {
    // Hit your favorite API. The exact URL doesn't matter for the pattern.
    let url = format!("https://your-quote-api.com/v1/quote/{symbol}");
    let resp = reqwest::get(&url).await.map_err(|e| e.to_string())?;
    let q: Quote = resp.json().await.map_err(|e| e.to_string())?;
    Ok(q)
}

#[derive(Deserialize)]
struct Quote { price: f64, change_pct: f64 }
```

### Wiring the agent loop for async tools

The starter's `execute_tool` is sync. When your tools need to be async (network, DB), change the function signature and the agent loop:

```rust
// In the agent loop in `chat()`:
let result_json = execute_tool(
    part.name.as_deref().unwrap_or(""),
    &part.arguments_delta,
    &state,                              // pass state in
).await;
```

`tokio::spawn` for fire-and-forget, `tokio::join!` for parallel calls, `tokio::time::timeout` for tools that might hang. The agent loop is just a `for` loop — everything you can do with `tokio`, you can do here.

### What you get

The user can now say:

- *"What are my stocks doing today?"* → LLM calls `summarize_watchlist`, gets a JSON of every watchlist ticker with live prices, narrates the highlights.
- *"Add NVDA with note 'AI bet'"* → LLM calls `add_to_watchlist`, gets `{ "ok": true }`, confirms.
- *"How is AAPL doing compared to yesterday?"* → LLM calls `get_quote("AAPL")`, then narrates.

Every tool call shows up as a `ToolCallCard` in the UI. The watchlist persists in SQLite across restarts.

---

## Patterns reference

Common questions, with the 30-second answer.

## Pluggability

Every component in laipe is designed to be **swappable, replaceable, and composable**. The full principle lives in [`AGENTS.md`](AGENTS.md#pluggability--global-design-principle); the short version:

| What | Interface | Default | How to extend |
|---|---|---|---|
| Chat transport | `StreamSource` (TS) | `tauriStream` | Implement the interface; pass to `useChat(source, ...)` |
| Protocol dispatcher | `StreamChat` (Rust) | `openai_chat` / `openai_responses` / `anthropic` | Implement the trait; register in `pick()` |
| Tool execution | `execute_tool(name, args)` match (Rust) | 2 sample tools | Add a `match` arm in `lib.rs` + a schema in `src/tools.ts` |
| Config storage | `ConfigStorage` (TS) | `localStorage` | Implement the interface; call `setConfigStorage(s)` |
| Console transport | `useConsoleEntries()` singleton | Tauri events + `console.log` hook | Replace `console.ts` with your own bus |
| UI composition | `<slot>` (Vue) | Layout in `App.vue` | Override slots on `ChatView` / `MessageBubble` / `SettingsModal` |

When you find yourself reaching for a 3rd hardcoded implementation of the same thing — that's the signal. Add an interface, take it as a param, keep the default behavior unchanged. Don't over-pluggify: every new interface is something consumers must learn.

### How do I add a system prompt?

Prepend `{ role: "system", content: "..." }` to the messages array before calling `useChat.send(...)`. Or add a `systemPrompt` field to `Conversation` in `useConversations` and inject it in `App.vue`.

### How do I gate a tool behind user confirmation?

In `execute_tool`, instead of running immediately, emit a `chat:tool_needs_approval` event with the call details, return a JSON like `{"status": "pending_approval"}`, and wait for a `chat:tool_approved` event from the frontend before actually running. The frontend shows a confirmation card in `MessageBubble`'s `tool-calls` slot; clicking "Approve" fires the approval event.

### How do I stream tool progress?

Add a new `StreamEvent` variant: `ToolProgress { id, message }`. Emit `chat:tool_progress` Tauri events from inside `execute_tool` (via `app.emit`). The frontend's `tauriStream` listens and forwards as a new event type. `MessageBubble`'s `tool-calls` slot receives the call list and can render a progress bar.

### How do I run tools in parallel?

`futures::future::join_all` over the tool calls in the agent loop. Each call still gets its own `role: tool` message; the LLM sees them in order.

### How do I add long-running background tools?

`tokio::spawn` inside `execute_tool` for tasks that outlive the request (e.g. indexing a folder). Emit progress events. Store the result in shared state; let the next user prompt trigger a "this finished" continuation if appropriate.

### How do I sub-agent?

Treat a sub-agent as a tool. The tool's `execute_tool` calls `laipe_streaming::pick(fmt).dispatch(...)` recursively (or in a fresh `mpsc::channel`) with a *different* system prompt and a *different* tool set. Stream the sub-agent's output back to the parent via a `chat:tool_progress` event so the user sees what the sub-agent is doing.

### How do I do RAG?

Add an `indexed_search` tool. The Rust side chunks documents, embeds them (or uses BM25), and returns the top-k passages. The LLM then cites them in its response. For more sophistication, inject the top-k passages into the *system prompt* automatically on every turn (no tool call needed).

### How do I handle errors gracefully?

`execute_tool` should return JSON with an `error` field, not panic. The agent loop continues; the LLM sees the error and can retry, ask the user, or give up. Reserve Rust `?` / `Result::Err` for the case where the whole agent loop should bail.

### How do I avoid the agent looping forever?

`MAX_AGENT_TURNS` is the floor. Add a per-tool call counter (max N calls per tool per turn), and a token budget (max total prompt+completion tokens per turn) if you're cost-sensitive.

### How do I make tools work in browser-only mode?

`fetchStream` uses `laipe-ts`'s `dispatchStream`, which already accepts `tools`. The browser path doesn't execute tools, though — it just shows the tool calls to the user. For a real browser-only agent you'd need a JS-side `execute_tool` that calls HTTP APIs directly. Most production deployments stick to Tauri.

---

## Pointers for LLM-assisted development

The codebase is deliberately structured to be readable by an LLM. If you're an LLM picking this up, the navigation order is:

1. **Read [`API.md`](API.md) first.** Single source of truth for the public API surface. Every exported symbol is listed with its file path.
2. **Read the file header comment** of the file you're changing. Every non-trivial file starts with a `//!` (Rust) or `//` (TS) block explaining *what it is, why it exists, how to extend it, who depends on it*.
3. **Read the doc comment on the symbol** you're touching. The format is consistent: *what it does, what it returns, the typical caller, what can go wrong*.
4. **Find the seams in the [Pluggability table](#pluggability).** Almost every "how do I add X" question has the same answer: implement an interface, register it, expose a UI hook.
5. **Check the worked examples** in this document. The [Plot-writer](#worked-example-1-plot-writer-agent) and [Finboard](#worked-example-2-finboard-agent) walks show the same shape: schema → Rust impl → wire → UI.

### Prompt patterns that work well

- "Add a tool called `get_weather` that takes a `city: string` and returns the current weather. Follow the pattern in `laipe-app/src/tools.ts` and `laipe-app/src-tauri/src/lib.rs::execute_tool`."
- "Add a new pluggable storage backend that uses Tauri commands to persist config to `app_data_dir()/config.json`. Implement the `ConfigStorage` interface and call `setConfigStorage(...)` in `main.ts`."
- "Add a new stream source that proxies through a local WebSocket server. Implement the `StreamSource` interface in `packages/laipe-vue/src/streams.ts` and pass it to `useChat(myStream, TOOLS)` in `App.vue`."
- "Refactor `execute_tool` from a `match` to a `HashMap<String, ToolFn>` registry so new tools can be added without editing `lib.rs`."

### What to give the LLM as context

- The current file's full content.
- The relevant section of [`API.md`](API.md) (one table is usually enough).
- The header comment of any imported module.
- The error message + the smallest possible reproduction.

### What NOT to ask the LLM

- "Implement a new LLM provider from scratch." The right path is `StreamChat` + `pick()`, but the protocol details are non-trivial; the LLM should reference the existing 3 implementations.
- "Optimize the streaming pipeline." The anti-stutter countermeasures are battle-tested; LLM "optimizations" usually regress them.
- "Replace the localStorage storage with a database." Implement `ConfigStorage` — don't reach into `useConfig.ts` internals.

---

## Anti-patterns

Things that *look* like good ideas but bite later.

- **Putting business logic in the Vue components.** If `App.vue` knows how to compute a stock price, the rule is in the wrong layer. Push it down into a tool.
- **Making tools call other tools synchronously.** This is how you get 8-second pauses and a frozen UI. Spawn a task, return a job id, let the next turn check on it.
- **Using the same tool list for every conversation.** Different tasks need different tools. Per-conversation tool lists is a v0.3+ feature; for now, fork the `TOOLS` constant or accept a `conversation.toolList: string[]` field.
- **Storing API keys in `localStorage`.** Use `tauri-plugin-stronghold`. The starter is a demo, not a production template.
- **Re-implementing the agent loop from scratch.** The one in `laipe-app/src-tauri/src/lib.rs` already handles tool-call accumulation, `Done` vs `Error` events, cancel mid-stream, and `MAX_AGENT_TURNS`. Extend it (add a new event type, a new dispatch path), don't replace it.

---

## See also

- [`README.md`](../README.md) — top-level pitch
- [`VISION.md`](VISION.md) — what laipe is and isn't
- [`docs/TOOL_CALLING.md`](docs/TOOL_CALLING.md) — cross-protocol tool schema translation
- [`docs/STREAMING.md`](docs/STREAMING.md) — the SSE pipeline
- [`docs/PROTOCOLS.md`](docs/PROTOCOLS.md) — picking OpenAI Chat / Responses / Anthropic
- [`laipe-app/src-tauri/src/lib.rs`](laipe-app/src-tauri/src/lib.rs) — the agent loop, line by line
- [`packages/laipe-vue/README.md`](../packages/laipe-vue/README.md) — every component, every slot
- [`ARCHITECTURE.md`](ARCHITECTURE.md) — crate boundaries, anti-stutter, design notes
