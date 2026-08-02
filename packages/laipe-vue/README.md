# laipe-vue

Vue 3 components for [`laipe`](../../) — built on top of [`laipe-ts`](../laipe-ts). Drop-in for **Tauri 2 webviews** (the laipe stack default), Vite dev servers, Nuxt 3, or any Vue 3 host.

This is the v0.2 deliverable: a composable component set with three explicit layers (primitives / composites / batteries-included) and a pluggable `StreamSource` so the same components work in Tauri, browser-only, and tests.

## Install

```bash
bun add laipe-vue laipe-ts vue
```

## Three layers, pick what you need

```
┌─────────────────────────────────────────────────────────────────┐
│  Batteries-included                                               │
│  AiChatPanel  —  one-line drop-in chat UI                         │
├─────────────────────────────────────────────────────────────────┤
│  Composites (compose primitives)                                 │
│  ChatView       —  message list + input                            │
│  Sidebar        —  multi-conversation list                         │
│  SettingsModal  —  provider config form                          │
├─────────────────────────────────────────────────────────────────┤
│  Primitives (no state, pure presentation)                         │
│  MessageBubble  —  single chat message                            │
│  ToolCallCard   —  one tool call (name + args + result)           │
│  MessageInput   —  input row + send/stop                          │
│  EmptyState     —  onboarding state with sample prompts           │
│  IconButton     —  small icon-only button                          │
├─────────────────────────────────────────────────────────────────┤
│  Composables + Streams                                            │
│  useChat, useConfig, useConversations                             │
│  tauriStream, fetchStream, mockStream, defaultStreamSource       │
└─────────────────────────────────────────────────────────────────┘
```

## Usage

### 1. Quick path — `AiChatPanel` (one-liner)

```vue
<script setup lang="ts">
import { AiChatPanel } from "laipe-vue";
import { useConfig } from "laipe-vue";

const { config } = useConfig();
</script>

<template>
  <AiChatPanel :config="config" @error="(m) => console.error(m)" />
</template>
```

`AiChatPanel` owns state, persistence (`useConfig` + `useConversations`), and streaming (`useChat(defaultStreamSource())`). Use it when you want a working chat with zero composition.

### 2. Custom composition — primitives + composables

```vue
<script setup lang="ts">
import {
  ChatView, Sidebar, SettingsModal,
  useConfig, useConversations, useChat,
  tauriStream,
} from "laipe-vue";
import type { ChatMessage, ProviderConfig } from "laipe-ts";
import { computed, ref } from "vue";

const { config } = useConfig();
const { conversations, current, setMessages, create, select, remove } = useConversations();
const { status, send, cancel } = useChat(tauriStream);

const messages = computed<ChatMessage[]>(() => current.value?.messages ?? []);
const settingsOpen = ref(false);

async function handleSend(text: string) {
  if (!config.value.api_key) return;
  const next = [...messages.value, { role: "user" as const, content: text }];
  setMessages(next);
  await send(config.value, next);
}
</script>

<template>
  <div class="app">
    <Sidebar
      :conversations="conversations"
      :current-id="current?.id ?? null"
      @select="select"
      @create="create"
      @remove="remove"
    />
    <ChatView
      :messages="messages"
      :status="status"
      @send="handleSend"
      @cancel="cancel"
    />
    <SettingsModal v-model:open="settingsOpen" v-model="config" />
  </div>
</template>
```

This is the pattern used in [`laipe-app/src/App.vue`](../../laipe-app/src/App.vue). You own the layout — wrap the components in whatever HTML you need.

### 3. Deep custom — primitives only

```vue
<script setup lang="ts">
import { MessageBubble, MessageInput, EmptyState } from "laipe-vue/primitives";
import { useChat, mockStream } from "laipe-vue";
import type { ChatMessage } from "laipe-ts";
import { ref } from "vue";

const messages = ref<ChatMessage[]>([]);
const { status, send, cancel } = useChat(mockStream);  // mock for offline dev

async function handleSend(text: string) {
  if (!text) return;
  messages.value = [...messages.value, { role: "user", content: text }];
  await send({ endpoint: "", api_key: "", model: "", api_format: "openai_chat" }, messages.value);
}
</script>

<template>
  <div class="chat">
    <EmptyState v-if="messages.length === 0" />
    <template v-else>
      <MessageBubble
        v-for="(m, i) in messages"
        :key="i"
        :message="m"
        :streaming="i === messages.length - 1 && status === 'streaming'"
      />
      <MessageInput v-model="text" :disabled="status === 'streaming'" @send="handleSend" @cancel="cancel" />
    </template>
  </div>
</template>
```

Use only the pieces you want. Every primitive is a presentational component with no state and no side effects.

## Stream sources

The `StreamSource` interface lets you swap where chat events come from without touching the components:

```ts
import { useChat, tauriStream, fetchStream, mockStream, defaultStreamSource } from "laipe-vue";

// Tauri (production default — calls the Rust backend via IPC)
const { send, cancel } = useChat(tauriStream);

// Browser-direct (dev only — needs CORS-permissive endpoint)
const { send, cancel } = useChat(fetchStream);

// Mock (offline UI dev, tests)
const { send, cancel } = useChat(mockStream);

// Auto-detect (Tauri in production, fetch in browser)
const { send, cancel } = useChat(defaultStreamSource());
```

Implement the `StreamSource` interface for your own backend:

```ts
import type { StreamSource, ChatMessage, ProviderConfig, StreamEvent } from "laipe-ts";

const myWebSocketStream: StreamSource = {
  async *send(config, messages, options) {
    const ws = new WebSocket("wss://my-backend/chat");
    // ... wire events to your backend
    yield { type: "text", delta: "hello" };
    yield { type: "done" };
  }
};
```

## Tool calling

`useChat` takes an optional second argument — a list of `ToolDefinition`s to make the LLM tool-aware. When set, the assistant's tool calls stream into the placeholder message's `tool_calls` array and are rendered by `MessageBubble` via `ToolCallCard`:

```ts
import { useChat, tauriStream } from "laipe-vue";
import type { ToolDefinition } from "laipe-ts";

const TOOLS: ToolDefinition[] = [
  {
    type: "function",
    function: {
      name: "get_current_time",
      description: "Return the current UTC time.",
      parameters: { type: "object", properties: {}, required: [] },
    },
  },
];

const { send, cancel } = useChat(tauriStream, TOOLS);
```

During streaming, each `message.tool_calls[i]` accumulates `id`, `name`, and `arguments` (a raw JSON string that may be partial). `MessageBubble` renders one `ToolCallCard` per call. Override the rendering with the `tool-calls` slot:

```vue
<ChatView :messages="messages" :status="status" @send="onSend" @cancel="onCancel">
  <template #message="{ message }">
    <MessageBubble :message="message">
      <template #tool-calls="{ calls }">
        <details v-for="c in calls" :key="c.id">
          <summary>{{ c.function.name }}</summary>
          <pre>{{ c.function.arguments }}</pre>
        </details>
      </template>
    </MessageBubble>
  </template>
</ChatView>
```

`AiChatPanel` exposes the same prop directly: `<AiChatPanel :tools="TOOLS" />`.

Tool execution lives on the **Rust side** of Tauri — see [`EXTENDING.md`](../../EXTENDING.md#layer-3-tool-execution) and the agent loop in `laipe-app/src-tauri/src/lib.rs`.

## Extension points

Every composite exposes slots. The `ChatView` has the most:

```vue
<ChatView
  :messages="messages"
  :status="status"
  @send="onSend"
  @cancel="onCancel"
>
  <template #header>             <!-- top banner / model info --> </template>
  <template #empty>              <!-- replaces default EmptyState --> </template>
  <template #before-messages>    <!-- banner above message list --> </template>
  <template #after-messages>     <!-- footer below message list --> </template>
  <template #message="{ message, index, streaming }">
    <!-- fully replace how a single message is rendered -->
    <MyCustomBubble :msg="message" :streaming="streaming" />
  </template>
  <template #message-actions="{ message, index }">
    <!-- action buttons inside each message (copy, regenerate, edit) -->
    <button @click="copy(message)">📋</button>
  </template>
  <template #tool-calls="{ calls }">
    <!-- inside MessageBubble, replace per-call ToolCallCard rendering -->
    <MyToolCall :call="calls[0]" />
  </template>
  <template #input-before>       <!-- buttons before the input textarea --> </template>
  <template #input-after>        <!-- buttons after the input textarea --> </template>
</ChatView>
```

`MessageBubble`, `Sidebar`, and `SettingsModal` all expose their own slots — see the individual component files for details.

## Theming

Every component uses CSS variables with sensible defaults. Override in your root CSS:

```css
:root {
  --laipe-bg: #1a1a1a;            /* page background */
  --laipe-bg-elevated: #2a2a2a;    /* modal / sidebar background */
  --laipe-bg-sidebar: #222;       /* sidebar background */
  --laipe-text: #f0f0f0;          /* primary text */
  --laipe-text-secondary: #aaa;   /* secondary text */
  --laipe-text-muted: #777;       /* muted / placeholder text */
  --laipe-border: #333;           /* default border */
  --laipe-border-strong: #444;    /* focused border */
  --laipe-accent: #4a9eff;        /* primary brand / button color */
  --laipe-accent-hover: #2a7edf;  /* accent hover */
  --laipe-error: #ff5544;         /* error red */
  --laipe-radius: 8px;            /* default border radius */
}
```

Components use `var(--laipe-xxx, default)` so missing variables fall back to the built-in defaults.

## Persistence

`useConfig` and `useConversations` use `localStorage` by default. For production, replace with your own storage:

```ts
// (v0.3+ — for now, fork the composables or use a Pinia plugin)
```

For Tauri 2, the recommended production path is `tauri-plugin-stronghold` (OS keyring) for API keys, plus a Tauri command for persisting conversation state to disk.

## Browser caveats

`fetchStream` calls LLM APIs directly from the browser, which requires the upstream to allow CORS for your origin. Most providers don't.

- **For Tauri 2** (recommended): use `tauriStream`. The Rust backend makes the API call; the webview only sees abstracted events. No CORS.
- **For development** (browser-only): use a CORS proxy or a same-origin dev server.
- **For production web deploys**: always proxy through your own backend.

The `anthropic-dangerous-direct-browser-access: true` header is sent by `fetchStream` so the demo works against `api.anthropic.com` from a browser. Anthropic explicitly warns against this in production.

## License

MIT — same as the rest of laipe.
