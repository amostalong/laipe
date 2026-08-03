<script setup lang="ts">
// MainView — the chat view (the "main" route).
//
// v0.2+ extracted from App.vue when Settings became its own route
// (`/settings`). The conversation Sidebar stays in App.vue so it
// can be conditionally hidden on non-main routes.
//
// Composition:
//   - ChatView from laipe-vue (the message list + input)
//   - Topbar with model label + Delete-all button
//   - Empty-state card when there's no active conversation
//
// v0.2.1+: removed the in-topbar "Settings" button. The global
// TabsBar at the app root is the single entry point for Settings;
// the chat topbar stays focused on chat actions (clear all).
//
// All chat state (`useConfig`, `useConversations`, `useChat`) lives
// here, NOT in App.vue — when navigating to /settings and back, the
// chat state is preserved because the component is kept alive (the
// router caches the view by default; we use keep-alive below).

import { computed, onMounted, ref } from "vue";
import type { ChatMessage, ProviderConfig, ToolDefinition } from "laipe-ts";
import {
  ChatView,
  Sidebar,
  useConfig,
  useConversations,
  useChat,
  tauriStream,
} from "laipe-vue";
import { TOOLS } from "../tools";
import { cleanupModelId, findModel } from "../modelCatalog";
import { useProviderConfig } from "../composables/useProviderConfig";

// v0.2+ multi-provider: provider config 走 useProviderConfig (PlotCraft 等价
// multi-provider UX), agent settings 走 laipe-vue useConfig (跟 provider 解耦).
// `cfg` 是 active provider 的 laipe ProviderConfig; 没 active provider 时 null.
const { config: cfg, agentSettings } = useProviderConfig()
const { conversations, currentId, current, create, select, remove, setMessages, clearAll } =
  useConversations()

/** Tools that are currently enabled in Settings (filtered from TOOLS). */
const enabledToolsList = computed<ToolDefinition[]>(() =>
  TOOLS.filter((t) => agentSettings.value.enabledTools[t.function.name] ?? true),
)

// `tauriStream` invokes the Rust `chat` command; the filtered tool list
// tells the LLM (and the Rust agent loop) which functions it may call.
const { status, send, cancel } = useChat(tauriStream, () => enabledToolsList.value);

const toast = ref<string | null>(null);
const sidebarOpen = ref(true);
let toastTimer: ReturnType<typeof setTimeout> | null = null;

const messages = computed<ChatMessage[]>(() => current.value?.messages ?? []);

/** Topbar model label: catalog name if found, else cleaned+truncated id. */
const modelDisplay = computed<string>(() => {
  const c = cfg.value;
  if (!c) return "(no active provider)";
  const m = findModel(c.model);
  if (m) return m.name;
  return cleanupModelId(c.model || "");
});

onMounted(() => {
  if (!cfg.value?.api_key) {
    setTimeout(
      () => showToast("Add your API key in Settings to start chatting."),
      800,
    );
  }
});

function showToast(msg: string, duration = 4000): void {
  toast.value = msg;
  if (toastTimer) clearTimeout(toastTimer);
  toastTimer = setTimeout(() => {
    toast.value = null;
    toastTimer = null;
  }, duration);
}

async function handleSend(text: string): Promise<void> {
  const c = cfg.value;
  if (!c) {
    showToast("No active provider. Open Settings to configure one.");
    return;
  }
  if (!c.api_key) {
    showToast("No API key configured. Open Settings to add one.");
    return;
  }
  const next: ChatMessage[] = [
    ...messages.value,
    { role: "user", content: text },
  ];
  setMessages(next);
  // Propagate the active conversation id so the diagnostic recorder
  // can group saved error reports by conversation. `currentId` may be
  // null for the very first send before `useConversations` has
  // assigned one; pass undefined to let the backend default.
  await send(c, next, currentId.value ?? undefined);
}

function handleCancel(): void {
  cancel();
}

function handleNewChat(): void {
  create();
}

function handleSelect(id: string): void {
  select(id);
}

function handleRemove(id: string): void {
  remove(id);
}

function handleClearAll(): void {
  if (!confirm("Delete all conversations? This cannot be undone.")) return;
  clearAll();
}
</script>

<template>
  <div class="main-view">
    <header class="topbar">
      <button
        v-if="!sidebarOpen"
        class="btn-burger"
        title="Open sidebar"
        @click="sidebarOpen = true"
      >
        <svg viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
          <path d="M2 3.5A.5.5 0 0 1 2.5 3h11a.5.5 0 0 1 0 1h-11a.5.5 0 0 1-.5-.5Zm0 4A.5.5 0 0 1 2.5 7h11a.5.5 0 0 1 0 1h-11a.5.5 0 0 1-.5-.5Zm0 4a.5.5 0 0 1 .5-.5h7a.5.5 0 0 1 0 1h-7a.5.5 0 0 1-.5-.5Z" />
        </svg>
      </button>
      <div class="title-area">
        <span class="logo">▰</span>
        <h1>{{ current?.title || "laipe" }}</h1>
        <span v-if="cfg?.api_key" class="model-tag">
          {{ modelDisplay }} · {{ cfg?.api_format }}{{ cfg?.effort ? ` · ${cfg.effort}` : "" }}
        </span>
      </div>
      <div class="actions">
        <button
          v-if="conversations.length > 0"
          class="btn-icon"
          title="Delete all conversations"
          @click="handleClearAll"
        >
          <svg viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
            <path d="M5.5 5.5A.5.5 0 0 1 6 5h4a.5.5 0 0 1 0 1H6a.5.5 0 0 1-.5-.5ZM2.5 3.5A.5.5 0 0 1 3 3h10a.5.5 0 0 1 0 1H3a.5.5 0 0 1-.5-.5ZM3.118 5h9.764l-.804 8.066A2 2 0 0 1 10.092 15H5.908a2 2 0 0 1-1.986-1.934L3.118 5Z" />
          </svg>
        </button>
      </div>
    </header>

    <ChatView
      v-if="current"
      :messages="messages"
      :status="status"
      @send="handleSend"
      @cancel="handleCancel"
      @update="(m: ChatMessage[]) => setMessages(m)"
    />
    <div v-else class="no-conversation">
      <div class="setup-card">
        <h2>Welcome to laipe</h2>
        <p>A desktop chat app built on <strong>laipe</strong> + Tauri 2.</p>
        <p class="muted">
          Multi-conversation, settings page, streaming via Tauri IPC,
          localStorage persistence. Single .exe, no browser required.
        </p>
        <button class="btn-primary" @click="handleNewChat">Start your first chat</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.main-view {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-width: 0;
  background: #fafafa;
}
.topbar {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 10px 20px;
  border-bottom: 1px solid #e5e5e7;
  background: #ffffff;
  flex-shrink: 0;
  min-height: 52px;
}
.btn-burger {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: #1d1d1f;
  flex-shrink: 0;
  cursor: pointer;
}
.btn-burger svg { width: 16px; height: 16px; }
.btn-burger:hover { background: rgba(0, 0, 0, 0.06); }
.title-area {
  display: flex;
  align-items: baseline;
  gap: 10px;
  flex: 1;
  min-width: 0;
}
.title-area .logo {
  color: #007aff;
  font-size: 1.2em;
}
.title-area h1 {
  margin: 0;
  font-size: 1.05em;
  font-weight: 600;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.model-tag {
  font-size: 0.75em;
  color: #a1a1a6;
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.actions {
  display: flex;
  gap: 6px;
  align-items: center;
}
.btn-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: #6e6e73;
  cursor: pointer;
}
.btn-icon svg { width: 16px; height: 16px; }
.btn-icon:hover { background: rgba(0, 0, 0, 0.06); color: #ff3b30; }

.no-conversation {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
}
.setup-card {
  max-width: 460px;
  padding: 32px;
  background: #ffffff;
  border: 1px solid #e5e5e7;
  border-radius: 12px;
  text-align: center;
}
.setup-card h2 {
  margin: 0 0 12px 0;
  font-size: 1.3em;
}
.setup-card p {
  margin: 0 0 12px 0;
  color: #6e6e73;
  line-height: 1.6;
}
.setup-card p.muted { color: #a1a1a6; font-size: 0.85em; }
.btn-primary {
  margin-top: 12px;
  padding: 10px 20px;
  border: none;
  border-radius: 8px;
  background: #007aff;
  color: white;
  font-size: 0.95em;
  font-weight: 500;
  cursor: pointer;
  font-family: inherit;
}
.btn-primary:hover { background: #0066d6; }
@media (max-width: 720px) { .model-tag { display: none; } }
</style>
