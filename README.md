# laipe

> **a**gent + **l**lm + **p**ipe — a **starter** for building LLM-powered agent desktop clients.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org)
[![Status: v0.2](https://img.shields.io/badge/status-v0.2-yellow.svg)](../.agents/ROADMAP.md)

**`laipe` is not a framework. It's a starter.** A set of composable components you assemble to build your own agent client — a Cursor, a Cline, a Claude.ai, a PlotCraft. The stack is fixed: **Rust + Vue 3 + Tauri 2**. What you customize is which components you use and how you compose them.

3 protocols, one shape. **OpenAI Chat Completions**, **OpenAI Responses**, and **Anthropic Messages** all surface as the same `StreamEvent` enum. On the Rust side it's `tokio::sync::mpsc::Receiver<StreamEvent>`. On the TS side it's `AsyncGenerator<StreamEvent>`. Same events, same mental model, every runtime.

---

## Why laipe

| You want to… | Use laipe if… | Use something else if… |
|---|---|---|
| Build a Tauri 2 desktop app that talks to LLMs | ✅ This is the sweet spot | — |
| Avoid 3 different vendor SDKs in your deps | ✅ | — |
| Get a working multi-conversation chat in a real desktop window in 5 minutes | ✅ | — |
| Customize the UI / layout / features freely | ✅ All components are open and replaceable | — |
| Write your own SSE plumbing from scratch | ❌ | — |
| Use a different stack (Electron, React Native, Flutter, web-only) | ❌ Stack is fixed at Rust+Vue+Tauri | Pick the stack-native SDK |
| Drop-in agent with planning loop | ❌ Compose on top | [Rig](https://github.com/0xplaydust/rig), [LangChain](https://github.com/langchain-ai/langchain) |
| Server-side web framework (axum, actix) | ❌ Use laipe-core types but bring your own HTTP | Use [axum](https://github.com/tokio-rs/axum) directly |

---

## Quick start

```bash
git clone https://github.com/amostalong/laipe.git
cd laipe

# Install Rust + Node + bun + Tauri CLI (one-time)
#   rustup target add x86_64-pc-windows-msvc
#   cargo install tauri-cli --version "^2.0" --locked
#   bun --version  # ≥ 1.2

bun install

# Launch the desktop app (compiles Tauri + Vite, opens a native window)
run-laipe-app.bat   # Windows
# or:
bun run dev:app
```

Click **Settings** in the top right, paste your API key, and you're chatting. The key lives in Rust process memory + `localStorage` (change to OS keyring for production — see [laipe-app README](laipe-app/README.md)).

First build takes ~5 minutes (Tauri's dep tree is large); subsequent builds are seconds.

---

## What's in the box

| Path | Type | Status | What it is |
|---|---|---|---|
| `crates/laipe-core` | Rust lib | ✅ stable | Protocol-agnostic types. Zero HTTP/async deps. |
| `crates/laipe-streaming` | Rust lib | ✅ stable | 3-protocol SSE: `openai_chat` / `openai_responses` / `anthropic` + shared `sse::SseParser`. 25 unit tests. |
| `crates/laipe-tokio` | Rust lib | ✅ stable | `CancelHandle` + `run_to_completion` runtime glue. |
| `packages/laipe-ts` | TS lib | ✅ v0.2 | TS SDK: types + `SseParser` + `dispatchStream` (3 protocols). No UI. |
| `packages/laipe-vue` | Vue lib | ✅ v0.2 | Vue 3 components — primitives + composites + `AiChatPanel` (batteries-included). |
| `laipe-app` | Tauri 2 app | ✅ v0.2 | The starter app. Vue 3 frontend + Rust backend. Native window, single .exe, no browser. |

**Stack is fixed at Rust + Vue 3 + Tauri 2.** Customization happens at the *component composition* level (which slots you fill, which stream source you use, what theme you set), not at the *infrastructure* level.

---

## Architecture

```
laipe-app (Tauri 2 desktop window, OS webview)
│
├── Vue 3 frontend (laipe-vue components)
│     │
│     │  invoke('chat', { cfg, messages })
│     │  listen('chat:chunk', 'chat:done', 'chat:error', ...)
│     │
│     ▼
├── Tauri IPC
│
├── Rust backend (laipe-app/src-tauri)
│     │
│     │  #[tauri::command] async fn chat(cfg, messages)
│     │  laipe_streaming::pick(format).dispatch(cfg, messages, None)
│     │  laipe_tokio::CancelHandle (per-stream, for cancel button)
│     │
│     ▼
└── mpsc::Receiver<StreamEvent>
       │
       │  StreamEvent::Text(delta) | ToolCalls(parts) | Done | Error{kind, message}
       │
       ▼
  3 protocols (openai_chat / openai_responses / anthropic) → OpenAI / Anthropic / etc.
```

Both the Rust side (`mpsc::Receiver<StreamEvent>`) and the TS side (`AsyncGenerator<StreamEvent>`) yield the same events. If you want to swap the streaming source (Tauri → direct fetch, or Tauri → WebSocket, or Tauri → mock for tests), inject a different `StreamSource` — see [laipe-vue README](packages/laipe-vue/README.md).

See [ARCHITECTURE.md](../.agents/ARCHITECTURE.md) for the full writeup, including the 4 anti-stutter countermeasures and the data flow for tool calling.

---

## How to use it

### Quick path: one-liner

```vue
<script setup>
import { AiChatPanel } from "laipe-vue";
import { useConfig } from "laipe-vue";

const { config } = useConfig();
</script>

<template>
  <AiChatPanel :config="config" @error="(m) => console.error(m)" />
</template>
```

That's it. State, streaming, settings — all handled.

### Custom composition: full control

```vue
<script setup>
import {
  ChatView, Sidebar, SettingsModal,
  useConfig, useConversations, useChat,
  tauriStream,
} from "laipe-vue";

const { config } = useConfig();
const { conversations, current, setMessages, create, select, remove } = useConversations();
const { status, send, cancel } = useChat(tauriStream);

const messages = computed(() => current.value?.messages ?? []);

async function handleSend(text) {
  const next = [...messages.value, { role: "user", content: text }];
  setMessages(next);
  await send(config.value, next);
}
</script>

<template>
  <div class="app">
    <Sidebar :conversations="conversations" :current-id="..." @select="select" @create="create" @remove="remove" />
    <ChatView :messages="messages" :status="status" @send="handleSend" @cancel="cancel" />
    <SettingsModal v-model:open="settingsOpen" v-model="config" />
  </div>
</template>
```

[laipe-app/src/App.vue](laipe-app/src/App.vue) is exactly this pattern, end-to-end.

### Deep custom: primitives only

```vue
<script setup>
import { MessageBubble, MessageInput, EmptyState } from "laipe-vue/primitives";
</script>

<template>
  <div class="chat">
    <div v-for="(m, i) in messages" :key="i">
      <MessageBubble :message="m" :streaming="i === messages.length - 1" />
    </div>
    <MessageInput v-model="text" @send="onSend" @cancel="onCancel" />
  </div>
</template>
```

Every component is replaceable. The library gives you the pieces; you assemble the layout.

---

## Customization

- **Theme**: every component uses CSS variables (`--laipe-bg`, `--laipe-text`, `--laipe-accent`, etc.). Override in your root CSS to re-theme.
- **Slots**: every composite exposes slots for `header`, `footer`, `message`, `message-actions`, `input-before`, `input-after`, etc. Inject your own markup without forking the component.
- **Stream source**: swap `tauriStream` for `fetchStream` (browser-direct), `mockStream` (testing), or your own implementation of the `StreamSource` interface.
- **Storage**: `useConfig` and `useConversations` use `localStorage` by default. Replace with IndexedDB / server sync by overriding the storage helper.
- **Add a protocol**: implement the 3-method `StreamSource` interface and inject it. No other component needs to change.

---

## Documentation

All LLM-facing knowledge (project conventions, API reference, architecture, fork-and-extend guide, vision, roadmap, changelog, contributing) lives in **[`.agents/`](.agents/README.md)** — a unified folder AI coding agents should read first.

| File | What's in it |
|---|---|
| [.agents/README.md](.agents/README.md) | **Start here.** Index of the unified LLM knowledge folder. |
| [.agents/AGENTS.md](.agents/AGENTS.md) | Project conventions: pluggability + LLM-friendliness + code style + testing + security |
| [.agents/API.md](.agents/API.md) | Single source of truth for the public API surface (every exported symbol, with file path). Start here for "where is X defined / what calls Y". |
| [.agents/ARCHITECTURE.md](.agents/ARCHITECTURE.md) | Crate boundaries, streaming pipeline, anti-stutter tricks, tool-calling data flow, **pluggability seams map** |
| [.agents/EXTENDING.md](.agents/EXTENDING.md) | Fork-and-extend guide: 10-step walkthrough + 2 worked examples (plot-writer, finboard) + pluggability reference + LLM-assisted development pointers |
| [.agents/VISION.md](.agents/VISION.md) | One-line positioning, what laipe is and isn't, design principles, target users, origin story |
| [.agents/ROADMAP.md](.agents/ROADMAP.md) | v0.1 → v1.0 plan, what's deliberately deferred |
| [.agents/CHANGELOG.md](.agents/CHANGELOG.md) | Per-version what changed |
| [.agents/CONTRIBUTING.md](.agents/CONTRIBUTING.md) | Pre-community status, ground rules, commit message style, what to work on |
| [laipe-app/README.md](laipe-app/README.md) | The starter app — install, run, customize, build, ship to all 5 platforms |
| [packages/laipe-ts/README.md](packages/laipe-ts/README.md) | TS SDK: 3-protocol dispatch, Cancellation, mid-stream errors |
| [packages/laipe-vue/README.md](packages/laipe-vue/README.md) | Vue 3 components: 3 layers (primitives / composites / batteries-included), slots, theming |
| [.agents/docs/PROTOCOLS.md](.agents/docs/PROTOCOLS.md) | 3-protocol comparison table + when to pick which |
| [.agents/docs/STREAMING.md](.agents/docs/STREAMING.md) | 4 anti-stutter countermeasures + StreamEvent flow |
| [.agents/docs/TOOL_CALLING.md](.agents/docs/TOOL_CALLING.md) | Tool schema cross-protocol translation table + 3 built-in patterns |

---

## Where this came from

`laipe` is the **agent-stripped** core of [PlotCraft](https://github.com/amostalong/plotcraft), an AI-screenwriter desktop tool. PlotCraft's chat / streaming / tool calling layers were 4000+ lines of carefully-iterated Rust + TS code; `laipe` is that same code with the RPG-screenwriter business logic removed.

If you want to see what a full agent client looks like on top of `laipe`, [PlotCraft](https://github.com/amostalong/plotcraft) is exactly that kind of thing — and the lessons from running it against OpenAI / Anthropic / DeepSeek / GLM are baked into laipe's defaults.

## License

MIT. See [LICENSE](LICENSE).
