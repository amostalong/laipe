<script setup lang="ts">
// MainView — the chat view (the "main" route).
//
// v0.2+ 设计:
// - ChatView from laipe-vue (message list + input)
// - Topbar: model display via ModelEffortSelector (chat 端选 model + effort)
// - 错误反馈: 8 分类玩家文案 + retry (镜像 PlotCraft SessionView 错误条)
//
// v0.2+ tool approval: 走 laipe-vue useToolApprovals + 现有 useChat 架构.
//
// v0.2+ multi-provider: provider config 走 useProviderConfig, agent settings
// 走 laipe-vue useConfig (跟 provider 解耦).

import { computed, onMounted, ref, watch } from "vue";
import type {
  AssistantToolCall,
  ChatMessage,
  ProviderConfig,
  ToolDefinition,
  ChatErrorKind,
} from "laipe-ts";
import {
  ChatView,
  MessageBubble,
  Sidebar,
  ToolCallCard,
  useConfig,
  useConversations,
  useChat,
  tauriStream,
} from "laipe-vue";
import { TOOLS } from "../tools";
import { useProviderConfig } from "../composables/useProviderConfig";
import { useToolApprovals } from "../composables/useToolApprovals";
import { getErrorMessage, type PlayerErrorMessage } from "../lib/error-messages";
import type { EffortLevel } from "../lib/settings";
import ModelEffortSelector from "../components/chat/ModelEffortSelector.vue";

const { config: cfg, agentSettings, providers, setActive } = useProviderConfig();
const { conversations, currentId, current, create, select, remove, setMessages, clearAll } =
  useConversations()
const { agentSettings: _agentSettings2, reset: _resetAgent } = useConfig()

/** Chat 端 state: 当前选中的 model id + effort
 *  - 默认从 active provider 的 defaultModel 开始
 *  - 用户在 ModelEffortSelector 切 → 写这里 + useProviderConfig.setActive
 */
const selectedModel = ref<string>(cfg.value?.model ?? "");
const selectedEffort = ref<EffortLevel>("none");

// 跟随 active provider 切 → 同步 selectedModel
watch(
  () => cfg.value?.model,
  (m) => {
    if (m != null) selectedModel.value = m;
  },
);

const unconfiguredProviderCount = computed(
  () =>
    providers.value.filter((p) => {
      if (!p.enabled) return false;
      const effective = p.defaultModel?.trim() || p.models?.[0]?.id?.trim() || "";
      return effective === "";
    }).length,
);

const effortSupported = computed(() => selectedModel.value.trim().length > 0);

/** 玩家在 chat 端选 model → 反查 provider, 切 active connection */
function onSelectModel(id: string) {
  const cp = providers.value.find((p) => {
    if (!p.enabled) return false;
    const effective = p.defaultModel?.trim() || p.models?.[0]?.id?.trim() || "";
    return effective === id;
  });
  if (cp) {
    // v0.2+ Laipe: useProviderConfig.setActive 切 active provider
    setActive(cp.id);
  }
  selectedModel.value = id;
}

function onSelectEffort(level: EffortLevel) {
  selectedEffort.value = level;
}

/** Tools that are currently enabled in Settings (filtered from TOOLS). */
const enabledToolsList = computed<ToolDefinition[]>(() =>
  TOOLS.filter((t) => agentSettings.value.enabledTools[t.function.name] ?? true),
)

const { status, lastError, lastErrorKind, send, cancel, clearError } = useChat(
  tauriStream,
  () => enabledToolsList.value,
  () => agentSettings.value.toolPermissions,
);

const approvals = useToolApprovals();

const toast = ref<string | null>(null);
const sidebarOpen = ref(true);
let toastTimer: ReturnType<typeof setTimeout> | null = null;

const messages = computed<ChatMessage[]>(() => current.value?.messages ?? []);

/** v0.2+ 错误条 player 文案 (PlotCraft 镜像) */
const errorMessage = computed<PlayerErrorMessage | null>(() => {
  if (!lastError.value) return null;
  return getErrorMessage(lastErrorKind.value as ChatErrorKind | null, lastError.value);
});

/** Retry: 删最后一条 assistant 错误消息 + 重发最后一条 user message */
async function onRetry() {
  if (!errorMessage.value?.canRetry) return;
  const lastUser = [...messages.value].reverse().find((m) => m.role === "user");
  if (!lastUser) return;
  // strip trailing error assistant messages
  const next: ChatMessage[] = [];
  for (const m of messages.value) {
    if (m.role === "assistant" && m.content.startsWith("[") && m.content.includes("]")) {
      // skip error-tagged assistant message
      continue;
    }
    next.push(m);
  }
  setMessages(next);
  clearError();
  const c = cfg.value;
  if (!c) return;
  await send(c, next, currentId.value ?? undefined);
}

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
  if (!selectedModel.value.trim()) {
    showToast("No model selected. Pick one in the model selector below.");
    return;
  }
  // 把 selectedModel / effort 应用到 effective config (不写回 useProviderConfig,
  // 跟 PlotCraft 一致 — chat 端切 model 写回, effort 是 per-run 的)
  const eff: ProviderConfig = {
    ...c,
    model: selectedModel.value,
    effort: selectedEffort.value === "none" ? undefined : selectedEffort.value,
  };
  const next: ChatMessage[] = [
    ...messages.value,
    { role: "user", content: text },
  ];
  setMessages(next);
  await send(eff, next, currentId.value ?? undefined);
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
  if (confirm("Delete all conversations? This cannot be undone.")) clearAll();
}

function handleApprove(call: AssistantToolCall): void {
  if (!call.id) return;
  void approvals.approve(call.id);
}

function handleDeny(call: AssistantToolCall): void {
  if (!call.id) return;
  void approvals.deny(call.id);
}

onMounted(() => {
  if (!cfg.value?.api_key) {
    setTimeout(
      () => showToast("Add your API key in Settings to start chatting."),
      800,
    );
  }
});

watch(
  () => messages.value,
  (msgs) => {
    const last = msgs[msgs.length - 1];
    if (!last || last.role !== "assistant") return;
    for (const call of last.tool_calls ?? []) {
      if (!call.id) continue;
      approvals.track(call);
    }
  },
  { deep: true },
);
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
    >
      <template #message="{ message, streaming }">
        <MessageBubble :message="message" :streaming="streaming">
          <template #tool-calls="{ calls }">
            <ToolCallCard
              v-for="call in calls"
              :key="call.id"
              :call="call"
              :pending="streaming"
              :on-approve="() => handleApprove(call)"
              :on-deny="() => handleDeny(call)"
            />
          </template>
        </MessageBubble>
      </template>
    </ChatView>
    <div v-else class="no-conversation">
      <div class="setup-card">
        <h2>Welcome to laipe</h2>
        <p>A desktop chat app built on <strong>laipe</strong> + Tauri 2.</p>
        <p class="muted">
          Multi-provider (PlotCraft 1:1 mirror), settings page, streaming
          via Tauri IPC, localStorage persistence. Single .exe, no browser
          required.
        </p>
        <button class="btn-primary" @click="handleNewChat">Start your first chat</button>
      </div>
    </div>

    <!-- v0.2+ chat composer 顶部错误条 + ModelEffortSelector (PlotCraft 镜像) -->
    <div v-if="current" class="composer-area">
      <!-- 错误条 -->
      <div v-if="errorMessage" class="error-bar" :class="{ retryable: errorMessage.canRetry }">
        <div class="error-bar-main">
          <div class="error-bar-title">{{ errorMessage.title }}</div>
          <div class="error-bar-desc">{{ errorMessage.description }}</div>
          <div class="error-bar-hint">{{ errorMessage.hint }}</div>
        </div>
        <div class="error-bar-actions">
          <button
            v-if="errorMessage.canRetry"
            type="button"
            class="btn-retry"
            @click="onRetry"
          >
            ↻ Retry
          </button>
          <button
            type="button"
            class="btn-dismiss"
            title="Dismiss"
            @click="clearError"
          >
            ✕
          </button>
        </div>
      </div>

      <!-- composer: model selector + 临时 input 显示 (real input 是 ChatView 的) -->
      <div class="model-row">
        <ModelEffortSelector
          :selected-id="selectedModel"
          :effort="selectedEffort"
          :effort-supported="effortSupported"
          :unconfigured-provider-count="unconfiguredProviderCount"
          :disabled="status === 'streaming'"
          @select-model="onSelectModel"
          @select-effort="onSelectEffort"
        />
        <span v-if="!selectedModel.trim()" class="model-hint">
          ⚠ 没选 model —— 点上面 selector
        </span>
      </div>
    </div>

    <!-- toast (top center) -->
    <div v-if="toast" class="toast">{{ toast }}</div>
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

/* v0.2+ composer area (model selector + error bar) */
.composer-area {
  flex-shrink: 0;
  border-top: 1px solid #e5e5e7;
  background: #ffffff;
}
.error-bar {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  padding: 10px 16px;
  background: rgba(255, 59, 48, 0.06);
  border-bottom: 1px solid rgba(255, 59, 48, 0.2);
}
.error-bar.retryable {
  background: rgba(255, 149, 0, 0.08);
  border-bottom-color: rgba(255, 149, 0, 0.3);
}
.error-bar-main {
  flex: 1;
  min-width: 0;
}
.error-bar-title {
  font-size: 0.85em;
  font-weight: 600;
  color: #ff3b30;
  margin-bottom: 2px;
}
.error-bar.retryable .error-bar-title {
  color: #ff9500;
}
.error-bar-desc {
  font-size: 0.78em;
  color: #6e6e73;
  margin-bottom: 2px;
  line-height: 1.4;
}
.error-bar-hint {
  font-size: 0.72em;
  color: #6e6e73;
  font-style: italic;
  line-height: 1.4;
}
.error-bar-actions {
  display: flex;
  gap: 4px;
  align-items: center;
  flex-shrink: 0;
}
.btn-retry,
.btn-dismiss {
  padding: 4px 10px;
  font-size: 0.78em;
  border-radius: 4px;
  cursor: pointer;
  font-family: inherit;
  border: 1px solid #d2d2d7;
}
.btn-retry {
  background: #007aff;
  border-color: #007aff;
  color: white;
}
.btn-retry:hover {
  opacity: 0.85;
}
.btn-dismiss {
  background: transparent;
  color: #6e6e73;
}
.btn-dismiss:hover {
  background: rgba(0, 0, 0, 0.06);
}

.model-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 16px;
}
.model-hint {
  font-size: 0.75em;
  color: #ff9500;
  font-style: italic;
}

.toast {
  position: fixed;
  top: 20px;
  left: 50%;
  transform: translateX(-50%);
  padding: 8px 16px;
  background: rgba(0, 0, 0, 0.85);
  color: white;
  border-radius: 6px;
  font-size: 0.85em;
  z-index: 200;
}
</style>
