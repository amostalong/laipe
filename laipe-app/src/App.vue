<script setup lang="ts">
// laipe-app — a complete Tauri 2 desktop chat app.
//
// This file is the **deep composition** example: instead of dropping in
// `AiChatPanel` as a one-liner, it composes the primitives directly to
// show how every part fits together. Use this as a reference for your
// own custom layout — replace the ChatView/Sidebar/SettingsModal here
// with your own primitives, or drop in <AiChatPanel /> for the quick
// path.
//
// Imports come from laipe-vue (the framework):
//   - composables: useConfig, useConversations, useChat
//   - composites:  ChatView, Sidebar, SettingsModal
//   - streams:     tauriStream (calls Rust backend via Tauri IPC)
//
// The Tauri Rust backend is in src-tauri/src/lib.rs.

import { ref, computed, onMounted } from "vue";
import type { ChatMessage, EffortLevel, ProviderConfig, ToolDefinition } from "laipe-ts";
import {
  ChatView,
  Sidebar,
  SettingsModal,
  ConsolePanel,
  useConfig,
  useConversations,
  useChat,
  tauriStream,
} from "laipe-vue";
import { TOOLS } from "./tools";
import { cleanupModelId, findModel } from "./modelCatalog";
import ModelSelector from "./components/ModelSelector.vue";
import ToolsSettings from "./components/ToolsSettings.vue";
import DiagnosticsSettings from "./components/DiagnosticsSettings.vue";

const { config, agentSettings } = useConfig();
const { conversations, currentId, current, create, select, remove, setMessages, clearAll } =
  useConversations();

/** Tools that are currently enabled in Settings (filtered from TOOLS). */
const enabledToolsList = computed<ToolDefinition[]>(() =>
  TOOLS.filter((t) => agentSettings.value.enabledTools[t.function.name] ?? true),
);

// `tauriStream` invokes the Rust `chat` command; the filtered tool list
// tells the LLM (and the Rust agent loop) which functions it may call.
// Pass a getter so toggling a tool in Settings takes effect on the next
// send without rebuilding the composable. The Rust side owns the tool
// implementations — see src-tauri/src/lib.rs.
const { status, send, cancel } = useChat(tauriStream, () => enabledToolsList.value);

const settingsOpen = ref(false);
const sidebarOpen = ref(true);
const toast = ref<string | null>(null);
let toastTimer: ReturnType<typeof setTimeout> | null = null;

const messages = computed<ChatMessage[]>(() => current.value?.messages ?? []);

/** Topbar model label: catalog name if found, else cleaned+truncated id. */
const modelDisplay = computed<string>(() => {
  const m = findModel(config.value.model);
  if (m) return m.name;
  return cleanupModelId(config.value.model || "");
});

onMounted(() => {
  if (!config.value.api_key) {
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

function updateEnabledTools(next: Record<string, boolean>): void {
  agentSettings.value.enabledTools = { ...next };
}

async function handleSend(text: string): Promise<void> {
  if (!config.value.api_key) {
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
  await send(config.value, next, currentId.value ?? undefined);
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
  <div class="app">
    <Sidebar
      :conversations="conversations"
      :current-id="currentId"
      :collapsed="!sidebarOpen"
      @select="handleSelect"
      @create="handleNewChat"
      @remove="handleRemove"
      @toggle="sidebarOpen = false"
    />

    <main class="main">
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
          <span v-if="config.api_key" class="model-tag">
            {{ modelDisplay }} · {{ config.api_format }}{{ config.effort ? ` · ${config.effort}` : "" }}
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
          <button class="btn-settings" @click="settingsOpen = true">
            <svg viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
              <path d="M9.405 1.05c-.413-1.4-2.397-1.4-2.81 0l-.1.34a1.464 1.464 0 0 1-2.105.872l-.31-.17c-1.283-.698-2.686.705-1.987 1.987l.169.311c.446.82.023 1.841-.872 2.105l-.34.1c-1.4.413-1.4 2.397 0 2.81l.34.1a1.464 1.464 0 0 1 .872 2.105l-.17.31c-.698 1.283.705 2.686 1.987 1.987l.311-.169a1.464 1.464 0 0 1 2.105.872l.1.34c.413 1.4 2.397 1.4 2.81 0l.1-.34a1.464 1.464 0 0 1 2.105-.872l.31.17c1.283.698 2.686-.705 1.987-1.987l-.169-.311a1.464 1.464 0 0 1 .872-2.105l.34-.1c1.4-.413 1.4-2.397 0-2.81l-.34-.1a1.464 1.464 0 0 1-.872-2.105l.17-.31c.698-1.283-.705-2.686-1.987-1.987l-.311.169a1.464 1.464 0 0 1-2.105-.872l-.1-.34zM8 10.93a2.929 2.929 0 1 1 0-5.858 2.929 2.929 0 0 1 0 5.858z" />
            </svg>
            <span>Settings</span>
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
            Multi-conversation, settings modal, streaming via Tauri IPC,
            localStorage persistence. Single .exe, no browser required.
          </p>
          <button class="btn-primary" @click="handleNewChat">Start your first chat</button>
        </div>
      </div>
    </main>

    <SettingsModal
      :open="settingsOpen"
      v-model="config"
      @close="settingsOpen = false"
    >
      <!-- Model selector (curated + custom + effort) -->
      <template #model>
        <section class="field-group">
          <span class="label">Model</span>
          <ModelSelector
            :model-id="config.model"
            :api-format="config.api_format"
            :effort="config.effort ?? null"
            @update:model-id="(id: string) => { config.model = id; }"
            @update:effort="(lv: EffortLevel | null) => { config.effort = lv ?? undefined; }"
          />
          <small class="help">
            Pick from the curated list, or "Custom…" to type any model id
            (e.g. via OpenRouter). <em>Effort</em> only appears for
            reasoning-capable models.
          </small>
        </section>
      </template>

      <!-- Tools + Console below Advanced -->
      <template #extra>
        <ToolsSettings
          :tools="TOOLS"
          :enabled-tools="agentSettings.enabledTools"
          @update:enabled-tools="updateEnabledTools"
        />

        <details class="console-block">
          <summary>
            Debug Console
            <span class="console-hint">· runtime logs (info / warn / error, backend / frontend)</span>
          </summary>
          <div class="console-wrap">
            <ConsolePanel />
          </div>
        </details>

        <DiagnosticsSettings />
      </template>
    </SettingsModal>

    <Transition name="toast">
      <div v-if="toast" class="toast" @click="toast = null">
        <span class="toast-msg">{{ toast }}</span>
        <span class="toast-dismiss">(click to dismiss)</span>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.app {
  display: flex;
  height: 100vh;
  background: #fafafa;
  position: relative;
}
.main {
  flex: 1;
  display: flex;
  flex-direction: column;
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
.btn-settings {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  border: 1px solid #d2d2d7;
  border-radius: 6px;
  background: #ffffff;
  color: #1d1d1f;
  font-size: 0.85em;
  font-weight: 500;
  cursor: pointer;
  font-family: inherit;
}
.btn-settings svg { width: 16px; height: 16px; }
.btn-settings:hover { background: #f0f0f0; }

/* === Settings modal slot content (model, console) === */
.field-group {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-bottom: 4px;
}
.field-group .label {
  font-size: 0.85em;
  font-weight: 500;
  color: var(--laipe-text, #1d1d1f);
}
.field-group .help {
  font-size: 0.78em;
  color: var(--laipe-text-muted, #a1a1a6);
  line-height: 1.5;
}
.field-group .help em {
  font-style: italic;
  color: var(--laipe-text-secondary, #6e6e73);
}
.console-block {
  border-top: 1px solid var(--laipe-border, #e5e5e7);
  padding-top: 12px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.console-block summary {
  cursor: pointer;
  font-size: 0.85em;
  font-weight: 500;
  color: var(--laipe-text-secondary, #6e6e73);
  user-select: none;
  list-style: none;
}
.console-block summary::-webkit-details-marker { display: none; }
.console-block summary::before {
  content: "▸";
  display: inline-block;
  margin-right: 6px;
  transition: transform 0.15s ease;
}
.console-block[open] summary::before { transform: rotate(90deg); }
.console-hint {
  font-weight: 400;
  color: var(--laipe-text-muted, #a1a1a6);
  font-size: 0.9em;
  margin-left: 4px;
}
.console-wrap {
  border: 1px solid var(--laipe-border, #e5e5e7);
  border-radius: 6px;
  padding: 10px;
  background: var(--laipe-bg, #fafafa);
}
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
.toast {
  position: fixed;
  bottom: 20px;
  right: 20px;
  background: #1d1d1f;
  color: white;
  padding: 12px 16px;
  border-radius: 8px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.08);
  max-width: 480px;
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 12px;
  font-size: 0.9em;
  z-index: 200;
}
.toast-msg { flex: 1; word-break: break-word; }
.toast-dismiss { opacity: 0.6; font-size: 0.85em; }
.toast-enter-active, .toast-leave-active { transition: all 0.2s ease; }
.toast-enter-from, .toast-leave-to { opacity: 0; transform: translateY(20px); }
@media (max-width: 720px) { .model-tag { display: none; } }
</style>
