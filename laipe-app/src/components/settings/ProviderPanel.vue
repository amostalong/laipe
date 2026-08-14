<script setup lang="ts">
// ProviderPanel — PlotCraft-equivalent multi-provider UI for laipe-app
//
// 设计参照 PlotCraft/src/components/settings/ProvidersPanel.vue:
// - Saved Providers section: header (title + count + Add button)
// - card 列表: 每条 provider 一个 card, 右上角 3 个 icon 按钮
//   - Power/PowerOff 切 enabled
//   - Pencil 打开 Edit modal
//   - Trash 弹 Delete confirm modal (不用 window.confirm)
// - 整张 card 点击 → setActive (切 active)
//
// v0.2 laipe-app 简化 (vs PlotCraft):
// - 不接 Tauri models.dev catalog fetch (useModelCatalog 提供, modal 内部用)
// - 不做 "Import from Locus" 跨 app 导入 (laipe-app 是 Vue+Vite 浏览器 demo, 没 Tauri FS access)
// - modal 2 阶段 (pick → config) — Pick stage 调 useModelCatalog 拉 catalog
// - 字段 camelCase (baseUrl/apiKey/defaultModel) 跟 PlotCraft / useProviderConfig 对齐

import { ref, computed, onMounted, onUnmounted } from "vue";
import { useProviderConfig, type CustomProvider } from "../../composables/useProviderConfig";
import ProviderEditModal from "./ProviderEditModal.vue";

const {
  providers,
  activeProviderId,
  setActive,
  add,
  remove,
  update,
  toggleEnabled,
} = useProviderConfig();

// === Add / Edit modal state (2-stage: pick → config) ===
const editingProvider = ref<CustomProvider | null>(null);
const isAdding = ref(false);
const existingIds = computed(() => providers.value.map((p) => p.id));

function startAdd() {
  isAdding.value = true;
  editingProvider.value = {
    id: "",
    name: "",
    baseUrl: "https://",
    apiKey: "",
    apiFormat: "openai_chat",
    enabled: true,
    models: [],
    defaultModel: "",
  };
}

function startEdit(p: CustomProvider) {
  isAdding.value = false;
  // 浅拷贝（modal 内会修改 draft，但 props.provider 是 readonly）
  editingProvider.value = { ...p };
}

function closeModal() {
  editingProvider.value = null;
  isAdding.value = false;
}

function onModalSave(newProvider: CustomProvider) {
  if (isAdding.value) {
    const created = add(newProvider);
    setActive(created.id);
  } else {
    const oldId = editingProvider.value?.id;
    if (oldId) update(oldId, newProvider);
  }
  closeModal();
}

// === Delete confirm modal ===
const confirmingDeleteId = ref<string | null>(null);
const confirmingDeleteTarget = computed<CustomProvider | null>(() => {
  const id = confirmingDeleteId.value;
  if (!id) return null;
  return providers.value.find((p) => p.id === id) ?? null;
});

function askDelete(id: string) {
  confirmingDeleteId.value = id;
}
function cancelDelete() {
  confirmingDeleteId.value = null;
}
function confirmDelete() {
  const id = confirmingDeleteId.value;
  if (!id) return;
  remove(id);
  confirmingDeleteId.value = null;
}

function formatLabel(fmt: string): string {
  switch (fmt) {
    case "openai_chat":
      return "OpenAI Chat Completions";
    case "openai_responses":
      return "OpenAI Responses";
    case "anthropic_messages":
      return "Anthropic Messages";
    default:
      return fmt;
  }
}

function shortEndpoint(ep: string): string {
  if (!ep) return "(no endpoint)";
  try {
    return new URL(ep).host;
  } catch {
    return ep.length > 32 ? ep.slice(0, 31) + "…" : ep;
  }
}

function maskKey(k: string): string {
  return k ? "••••" + k.slice(-4) : "(empty)";
}

// Esc 关 delete modal
function onKeydown(e: KeyboardEvent) {
  if (e.key === "Escape" && confirmingDeleteId.value) cancelDelete();
}
onMounted(() => document.addEventListener("keydown", onKeydown));
onUnmounted(() => document.removeEventListener("keydown", onKeydown));
</script>

<template>
  <div class="provider-panel">
    <header class="header">
      <div class="header-text">
        <h2>Providers</h2>
        <p class="hint">
          Saved LLM provider library. Active provider is what chat uses.
          Toggle <strong>enabled</strong> to show/hide in the chat selector;
          click a card to set as <strong>active</strong>.
        </p>
      </div>
      <button v-if="!editingProvider" class="add-btn" @click="startAdd">
        <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <line x1="12" y1="5" x2="12" y2="19" />
          <line x1="5" y1="12" x2="19" y2="12" />
        </svg>
        <span>Add provider</span>
      </button>
    </header>

    <div v-if="providers.length === 0 && !editingProvider" class="empty">
      No providers yet. Click <strong>Add provider</strong> to start
      (OpenAI / DeepSeek / Anthropic / Ollama all work).
    </div>

    <div v-else class="provider-list">
      <div
        v-for="p in providers"
        :key="p.id"
        class="provider-card"
        :class="{ disabled: !p.enabled, active: p.id === activeProviderId }"
        @click="setActive(p.id)"
      >
        <div class="card-header">
          <div class="card-title">
            <span
              class="active-marker"
              :class="{ on: p.id === activeProviderId }"
              :title="p.id === activeProviderId ? 'Active provider' : 'Click to set as active'"
            >
              <svg v-if="p.id === activeProviderId" viewBox="0 0 24 24" width="11" height="11" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <polyline points="20 6 9 17 4 12" />
              </svg>
            </span>
            <span class="provider-name" :title="`id: ${p.id}`">{{ p.name || "(untitled)" }}</span>
            <span v-if="!p.enabled" class="tag disabled-tag">disabled</span>
            <span v-else-if="p.id === activeProviderId" class="tag active-tag">active</span>
          </div>
          <div class="card-actions" @click.stop>
            <button
              @click="toggleEnabled(p.id)"
              class="icon-btn"
              :title="p.enabled ? 'Disable (hide from chat selector)' : 'Enable (show in chat selector)'"
            >
              <svg v-if="p.enabled" viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <path d="M18.36 6.64a9 9 0 1 1-12.73 0" />
                <line x1="12" y1="2" x2="12" y2="12" />
              </svg>
              <svg v-else viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <path d="M18.36 6.64a9 9 0 1 1-12.73 0" />
                <line x1="12" y1="2" x2="12" y2="12" />
                <line x1="3" y1="3" x2="21" y2="21" />
              </svg>
            </button>
            <button @click="startEdit(p)" class="icon-btn" title="Edit">
              <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <path d="M12 20h9" />
                <path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z" />
              </svg>
            </button>
            <button
              @click="askDelete(p.id)"
              class="icon-btn danger"
              title="Delete"
              :disabled="providers.length <= 1"
            >
              <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <polyline points="3 6 5 6 21 6" />
                <path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6" />
                <path d="M10 11v6" />
                <path d="M14 11v6" />
              </svg>
            </button>
          </div>
        </div>
        <div class="card-body">
          <div class="endpoint-badge">{{ shortEndpoint(p.baseUrl) }}</div>
          <div class="card-row">
            <span class="card-label">format:</span>
            <span class="card-val">{{ formatLabel(p.apiFormat) }}</span>
          </div>
          <div class="card-row">
            <span class="card-label">apiKey:</span>
            <code class="card-val">{{ maskKey(p.apiKey) }}</code>
          </div>
          <div class="card-row">
            <span class="card-label">model:</span>
            <code class="card-val">{{ p.defaultModel || p.models[0]?.id || "(no model)" }}</code>
          </div>
        </div>
      </div>
    </div>

    <!-- Edit / Add Modal (2 阶段: pick → config) -->
    <ProviderEditModal
      :provider="editingProvider"
      :is-new="isAdding"
      :existing-ids="existingIds"
      @close="closeModal"
      @save="onModalSave"
    />

    <!-- Delete confirm modal -->
    <Teleport to="body">
      <div
        v-if="confirmingDeleteTarget"
        class="confirm-overlay"
        @mousedown.self="cancelDelete"
      >
        <div class="confirm-modal" role="alertdialog" aria-modal="true">
          <div class="confirm-header">
            <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" />
              <line x1="12" y1="9" x2="12" y2="13" />
              <line x1="12" y1="17" x2="12.01" y2="17" />
            </svg>
            <span class="confirm-title">Delete provider?</span>
          </div>
          <div class="confirm-body">
            <p>
              Delete provider
              <code class="confirm-target-id">{{ confirmingDeleteTarget.id }}</code>
              <span v-if="confirmingDeleteTarget.name" class="confirm-target-name">
                ({{ confirmingDeleteTarget.name }})
              </span>
              ? This cannot be undone.
            </p>
            <p class="confirm-hint">Other providers and chat history are not affected.</p>
          </div>
          <div class="confirm-actions">
            <button type="button" class="confirm-cancel" @click="cancelDelete">Cancel</button>
            <button type="button" class="confirm-delete" @click="confirmDelete" autofocus>Delete</button>
          </div>
        </div>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.provider-panel {
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
}
.header-text h2 {
  margin: 0 0 4px 0;
  font-size: 1.1em;
  font-weight: 600;
  color: var(--laipe-text, #1d1d1f);
}
.hint {
  margin: 0;
  font-size: 0.78em;
  color: var(--laipe-text-muted, #6e6e73);
  line-height: 1.5;
  max-width: 600px;
}
.hint strong {
  color: var(--laipe-text, #1d1d1f);
  font-weight: 600;
}
.add-btn {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  background: var(--laipe-accent, #007aff);
  color: white;
  border: 1px solid var(--laipe-accent, #007aff);
  border-radius: 6px;
  cursor: pointer;
  font-size: 0.82em;
  font-family: inherit;
  white-space: nowrap;
}
.add-btn:hover {
  opacity: 0.88;
}

.empty {
  font-size: 0.85em;
  color: var(--laipe-text-muted, #6e6e73);
  font-style: italic;
  padding: 32px 20px;
  text-align: center;
  background: var(--laipe-bg-elevated, #ffffff);
  border: 1px dashed var(--laipe-border, #d2d2d7);
  border-radius: 8px;
}

.provider-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.provider-card {
  background: var(--laipe-bg-elevated, #ffffff);
  border: 1px solid var(--laipe-border, #e5e5e7);
  border-radius: 8px;
  padding: 12px 14px;
  cursor: pointer;
  transition: border-color 0.15s ease, background 0.15s ease;
}
.provider-card:hover {
  border-color: var(--laipe-accent, #007aff);
}
.provider-card.active {
  border-color: var(--laipe-accent, #007aff);
  background: rgba(0, 122, 255, 0.04);
}
.provider-card.disabled {
  opacity: 0.55;
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 10px;
  gap: 8px;
}
.card-title {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  min-width: 0;
}
.active-marker {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 16px;
  height: 16px;
  border: 1px solid var(--laipe-border, #d2d2d7);
  border-radius: 50%;
  background: var(--laipe-bg, #fafafa);
  color: var(--laipe-accent, #007aff);
  flex-shrink: 0;
  transition: all 0.15s ease;
}
.active-marker.on {
  background: var(--laipe-accent, #007aff);
  border-color: var(--laipe-accent, #007aff);
  color: white;
}
.provider-name {
  font-size: 0.92em;
  font-weight: 600;
  color: var(--laipe-text, #1d1d1f);
}
.tag {
  font-size: 0.62em;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.4px;
  padding: 1px 6px;
  border-radius: 3px;
}
.active-tag {
  color: var(--laipe-accent, #007aff);
  background: rgba(0, 122, 255, 0.12);
  border: 1px solid var(--laipe-accent, #007aff);
}
.disabled-tag {
  color: var(--laipe-text-muted, #a1a1a6);
  background: var(--laipe-bg, #fafafa);
  border: 1px solid var(--laipe-border, #d2d2d7);
  font-style: italic;
}

.card-actions {
  display: flex;
  gap: 4px;
  align-items: center;
  flex-shrink: 0;
}
.icon-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  background: transparent;
  color: var(--laipe-text-muted, #6e6e73);
  border: 1px solid transparent;
  border-radius: 4px;
  cursor: pointer;
  transition: all 0.15s ease;
}
.icon-btn:hover:not(:disabled) {
  background: var(--laipe-bg, #f5f5f7);
  color: var(--laipe-text, #1d1d1f);
  border-color: var(--laipe-border, #e5e5e7);
}
.icon-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}
.icon-btn.danger:hover:not(:disabled) {
  background: rgba(255, 59, 48, 0.12);
  color: #ff3b30;
  border-color: #ff3b30;
}

.card-body {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.endpoint-badge {
  display: inline-block;
  align-self: flex-start;
  padding: 2px 8px;
  background: var(--laipe-bg, #f5f5f7);
  border: 1px solid var(--laipe-border, #d2d2d7);
  border-radius: 3px;
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  font-size: 0.72em;
  color: var(--laipe-text, #1d1d1f);
  margin-bottom: 4px;
}
.card-row {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 0.78em;
}
.card-label {
  color: var(--laipe-text-muted, #a1a1a6);
  width: 56px;
  flex-shrink: 0;
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
}
.card-val {
  color: var(--laipe-text, #1d1d1f);
}
.card-val code,
code.card-val {
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  background: var(--laipe-bg, #fafafa);
  border: 1px solid var(--laipe-border, #e5e5e7);
  padding: 0 5px;
  border-radius: 3px;
  font-size: 0.92em;
}

/* === Delete confirm modal === */
.confirm-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}
.confirm-modal {
  background: var(--laipe-bg-elevated, #ffffff);
  border: 1px solid var(--laipe-border, #e5e5e7);
  border-radius: 10px;
  padding: 20px 22px;
  max-width: 440px;
  width: 90%;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.18);
}
.confirm-header {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 12px;
  color: #ff3b30;
}
.confirm-title {
  font-size: 1em;
  font-weight: 600;
  color: var(--laipe-text, #1d1d1f);
}
.confirm-body {
  font-size: 0.85em;
  line-height: 1.5;
  color: var(--laipe-text, #1d1d1f);
  margin-bottom: 16px;
}
.confirm-body p {
  margin: 0 0 8px 0;
}
.confirm-hint {
  font-size: 0.78em;
  color: var(--laipe-text-muted, #6e6e73);
}
.confirm-target-id {
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  background: var(--laipe-bg, #f5f5f7);
  border: 1px solid var(--laipe-border, #d2d2d7);
  padding: 1px 5px;
  border-radius: 3px;
  font-size: 0.85em;
  color: var(--laipe-text, #1d1d1f);
}
.confirm-target-name {
  color: var(--laipe-text-muted, #6e6e73);
}
.confirm-actions {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
}
.confirm-cancel,
.confirm-delete {
  padding: 6px 16px;
  border-radius: 4px;
  font-size: 0.82em;
  font-family: inherit;
  cursor: pointer;
}
.confirm-cancel {
  background: transparent;
  border: 1px solid var(--laipe-border, #d2d2d7);
  color: var(--laipe-text, #1d1d1f);
}
.confirm-cancel:hover {
  border-color: var(--laipe-accent, #007aff);
}
.confirm-delete {
  background: #ff3b30;
  border: 1px solid #ff3b30;
  color: white;
}
.confirm-delete:hover {
  opacity: 0.85;
}
</style>
