<script setup lang="ts">
// ProviderEditModal — PlotCraft-style provider edit/add modal
//
// 设计参照 PlotCraft/src/components/settings/ProviderEditModal.vue
// (但 FinaBoard 简化版: 不做 2 阶段 pick → config, 不接 Tauri catalog,
//  不做 enable/disable checkbox (在 panel card 上已经做了), id 在 edit
//  模式锁住).
//
// Test connection 走 laipe-ts testProvider (跨 laipe-ts / laipe-streaming
// 共享的 1:1 实现, 3 协议 URL + auth + body + 响应解析).

import { computed, ref, watch } from "vue";
import type { ApiFormat, TestProviderResult } from "laipe-ts";
import { testProvider } from "laipe-ts";
import type { CustomProvider } from "../../composables/useProviderConfig";

const props = defineProps<{
  /** 当前编辑的 provider (null = 关闭) */
  provider: CustomProvider | null;
  /** true = 新建, false = 编辑 */
  isNew: boolean;
  /** 已有 id 集合, 新模式的唯一性校验 */
  existingIds: string[];
}>();

const emit = defineEmits<{
  close: [];
  save: [provider: CustomProvider];
}>();

// === Draft state (modal 期间独立) ===
const draftId = ref("");
const draftName = ref("");
const draftEndpoint = ref("");
const draftApiKey = ref("");
const draftApiFormat = ref<ApiFormat>("openai_chat");
const draftEnabled = ref(true);
const draftEffort = ref<string | null>(null);
const draftMaxTokens = ref<number | null>(null);
const draftTemperature = ref<number | null>(null);
const draftModel = ref("");

const localError = ref<string | null>(null);
const saving = ref(false);

// Test connection state
const testRunning = ref(false);
const testResult = ref<TestProviderResult | null>(null);

function resetDrafts(): void {
  draftId.value = "";
  draftName.value = "";
  draftEndpoint.value = "";
  draftApiKey.value = "";
  draftApiFormat.value = "openai_chat";
  draftEnabled.value = true;
  draftEffort.value = null;
  draftMaxTokens.value = null;
  draftTemperature.value = null;
  draftModel.value = "";
  localError.value = null;
  testRunning.value = false;
  testResult.value = null;
}

function populateFromProvider(p: CustomProvider): void {
  draftId.value = p.id;
  draftName.value = p.name;
  draftEndpoint.value = p.endpoint;
  draftApiKey.value = p.api_key;
  draftApiFormat.value = p.api_format as ApiFormat;
  draftEnabled.value = p.enabled;
  draftEffort.value = p.effort;
  draftMaxTokens.value = p.max_tokens;
  draftTemperature.value = p.temperature;
  draftModel.value = p.model;
}

// modal 每次打开 (provider 从 null 变 object) 时初始化 draft
watch(
  () => props.provider,
  (p) => {
    if (!p) return;
    saving.value = false;
    resetDrafts();
    if (!props.isNew) populateFromProvider(p);
  },
  { immediate: true },
);

const dialogTitle = computed(() => {
  if (props.isNew) return "Add provider";
  return `Edit "${props.provider?.name ?? ""}"`;
});

// max_tokens / temperature 字符串 <-> number
const maxTokensStr = computed<string>({
  get: () => (draftMaxTokens.value != null ? String(draftMaxTokens.value) : ""),
  set: (v) => {
    const t = v.trim();
    if (t === "") draftMaxTokens.value = null;
    else {
      const n = Number(t);
      draftMaxTokens.value = Number.isFinite(n) && n > 0 ? Math.floor(n) : null;
    }
  },
});
const temperatureStr = computed<string>({
  get: () => (draftTemperature.value != null ? String(draftTemperature.value) : ""),
  set: (v) => {
    const t = v.trim();
    if (t === "") draftTemperature.value = null;
    else {
      const n = Number(t);
      draftTemperature.value = Number.isFinite(n) ? n : null;
    }
  },
});

function onFormatChange(fmt: ApiFormat): void {
  draftApiFormat.value = fmt;
  if (!draftEndpoint.value.trim()) {
    draftEndpoint.value =
      fmt === "anthropic"
        ? "https://api.anthropic.com"
        : "https://api.openai.com/v1";
  }
}

const apiFormatOptions: { value: ApiFormat; label: string }[] = [
  { value: "openai_chat", label: "OpenAI Chat Completions" },
  { value: "openai_responses", label: "OpenAI Responses" },
  { value: "anthropic", label: "Anthropic Messages" },
];

function validate(): boolean {
  if (!draftName.value.trim()) {
    localError.value = "Display name required";
    return false;
  }
  if (!draftEndpoint.value.trim() || !draftEndpoint.value.startsWith("http")) {
    localError.value = "Endpoint must start with http/https";
    return false;
  }
  if (props.isNew) {
    if (!draftId.value.trim()) {
      localError.value = "ID required";
      return false;
    }
    if (props.existingIds.includes(draftId.value.trim())) {
      localError.value = `ID "${draftId.value}" already exists`;
      return false;
    }
  }
  localError.value = null;
  return true;
}

function buildProvider(): CustomProvider {
  return {
    id: draftId.value.trim(),
    name: draftName.value.trim(),
    endpoint: draftEndpoint.value.trim(),
    api_key: draftApiKey.value,
    model: draftModel.value.trim(),
    api_format: draftApiFormat.value,
    enabled: draftEnabled.value,
    effort: draftEffort.value,
    max_tokens: draftMaxTokens.value,
    temperature: draftTemperature.value,
    default_model: draftModel.value.trim() || null,
    models: [],
  };
}

function onSave(): void {
  if (!validate()) return;
  saving.value = true;
  emit("save", buildProvider());
}

async function onTest(): Promise<void> {
  if (!draftEndpoint.value.trim() || !draftApiFormat.value || !draftModel.value.trim()) {
    testResult.value = {
      ok: false,
      status: null,
      error: "Endpoint + API Format + model all required",
      response: null,
      endpoint: draftEndpoint.value,
      model: draftModel.value,
      apiFormat: draftApiFormat.value,
    };
    return;
  }
  testRunning.value = true;
  testResult.value = null;
  localError.value = null;
  try {
    testResult.value = await testProvider({
      endpoint: draftEndpoint.value,
      apiKey: draftApiKey.value,
      apiFormat: draftApiFormat.value,
      model: draftModel.value,
    });
  } catch (e) {
    testResult.value = {
      ok: false,
      status: null,
      error: String(e),
      response: null,
      endpoint: draftEndpoint.value,
      model: draftModel.value,
      apiFormat: draftApiFormat.value,
    };
  } finally {
    testRunning.value = false;
  }
}

function onClose(): void {
  if (saving.value) return;
  emit("close");
}

function onKeydown(e: KeyboardEvent): void {
  if (e.key === "Escape" && !saving.value) onClose();
}
</script>

<template>
  <Teleport to="body">
    <Transition name="modal">
      <div
        v-if="provider"
        class="modal-overlay"
        @mousedown.self="onClose"
        @keydown="onKeydown"
        tabindex="-1"
      >
        <div class="dialog" role="dialog" aria-modal="true">
          <header class="dialog-header">
            <span class="dialog-title">{{ dialogTitle }}</span>
            <button class="close-btn" type="button" :disabled="saving" @click="onClose">
              <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <line x1="18" y1="6" x2="6" y2="18" />
                <line x1="6" y1="6" x2="18" y2="18" />
              </svg>
            </button>
          </header>

          <div class="dialog-body">
            <p v-if="localError" class="error">{{ localError }}</p>

            <div class="grid">
              <!-- Left: connection fields -->
              <div class="col">
                <label class="field">
                  <span class="label">Display Name <span class="required">*</span></span>
                  <input
                    v-model="draftName"
                    type="text"
                    :class="{ invalid: localError?.includes('Display name') }"
                    :disabled="saving"
                    placeholder="My OpenAI"
                  />
                </label>

                <label class="field">
                  <span class="label">ID (unique key) <span v-if="isNew" class="required">*</span></span>
                  <input
                    v-model="draftId"
                    type="text"
                    :class="{ invalid: localError?.includes('ID') }"
                    :disabled="saving || !isNew"
                    :placeholder="isNew ? 'openai-main' : ''"
                    spellcheck="false"
                  />
                  <small v-if="!isNew" class="field-hint">ID is locked in edit mode</small>
                </label>

                <label class="field">
                  <span class="label">Endpoint <span class="required">*</span></span>
                  <input
                    v-model="draftEndpoint"
                    type="url"
                    :class="{ invalid: localError?.includes('Endpoint') }"
                    :disabled="saving"
                    placeholder="https://api.openai.com/v1"
                    spellcheck="false"
                  />
                </label>

                <label class="field">
                  <span class="label">API Key</span>
                  <input
                    v-model="draftApiKey"
                    type="password"
                    :disabled="saving"
                    placeholder="sk-..."
                    autocomplete="off"
                    spellcheck="false"
                  />
                  <small class="field-hint">Stored in <code>localStorage</code> by default</small>
                </label>

                <label class="field">
                  <span class="label">API Format</span>
                  <select
                    :value="draftApiFormat"
                    :disabled="saving"
                    @change="(e) => onFormatChange((e.target as HTMLSelectElement).value as ApiFormat)"
                  >
                    <option v-for="opt in apiFormatOptions" :key="opt.value" :value="opt.value">
                      {{ opt.label }}
                    </option>
                  </select>
                </label>
              </div>

              <!-- Right: model + advanced -->
              <div class="col">
                <label class="field">
                  <span class="label">Model ID <span class="required">*</span></span>
                  <input
                    v-model="draftModel"
                    type="text"
                    :disabled="saving"
                    placeholder="gpt-4o-mini"
                    spellcheck="false"
                  />
                  <small class="field-hint">
                    Use any model id (e.g. via OpenRouter <code>vendor/model</code>).
                    Laipe appends <code>/chat/completions</code> or <code>/responses</code> or <code>/v1/messages</code> per format.
                  </small>
                </label>

                <details class="advanced-block">
                  <summary>Advanced</summary>
                  <label class="field">
                    <span class="label">Effort</span>
                    <select
                      :value="draftEffort ?? ''"
                      :disabled="saving"
                      @change="(e) => (draftEffort = (e.target as HTMLSelectElement).value || null)"
                    >
                      <option value="">(default)</option>
                      <option value="low">low</option>
                      <option value="medium">medium</option>
                      <option value="high">high</option>
                    </select>
                    <small class="field-hint">Reasoning effort / thinking level. Laipe sends it via the protocol's effort field.</small>
                  </label>
                  <label class="field">
                    <span class="label">Temperature</span>
                    <input v-model="temperatureStr" type="text" inputmode="decimal" placeholder="leave empty for upstream default" />
                  </label>
                  <label class="field">
                    <span class="label">Max Tokens</span>
                    <input v-model="maxTokensStr" type="text" inputmode="numeric" placeholder="leave empty for upstream default" />
                  </label>
                </details>
              </div>
            </div>
          </div>

          <!-- Status bar: local error + test result -->
          <div
            v-if="localError || testResult || testRunning"
            class="dialog-status"
          >
            <div v-if="localError" class="status-error">{{ localError }}</div>
            <div
              v-else-if="testResult || testRunning"
              class="test-result"
              :class="{ ok: testResult?.ok, fail: testResult && !testResult.ok, testing: testRunning }"
            >
              <svg v-if="testRunning" class="spin" viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <line x1="12" y1="2" x2="12" y2="6" />
                <line x1="12" y1="18" x2="12" y2="22" />
                <line x1="4.93" y1="4.93" x2="7.76" y2="7.76" />
                <line x1="16.24" y1="16.24" x2="19.07" y2="19.07" />
                <line x1="2" y1="12" x2="6" y2="12" />
                <line x1="18" y1="12" x2="22" y2="12" />
                <line x1="4.93" y1="19.07" x2="7.76" y2="16.24" />
                <line x1="16.24" y1="7.76" x2="19.07" y2="4.93" />
              </svg>
              <svg v-else-if="testResult?.ok" viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14" />
                <polyline points="22 4 12 14.01 9 11.01" />
              </svg>
              <svg v-else-if="testResult" viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <circle cx="12" cy="12" r="10" />
                <line x1="15" y1="9" x2="9" y2="15" />
                <line x1="9" y1="9" x2="15" y2="15" />
              </svg>
              <span v-if="testRunning">Testing...</span>
              <span v-else-if="testResult?.ok">
                Connected<span v-if="testResult.status"> (HTTP {{ testResult.status }})</span>
              </span>
              <span v-else-if="testResult">
                Failed<span v-if="testResult.status"> (HTTP {{ testResult.status }})</span>
              </span>
              <code v-if="testResult?.response" class="test-response">
                {{ testResult.response }}
              </code>
              <code v-if="testResult?.error" class="test-error">
                {{ testResult.error }}
              </code>
            </div>
          </div>

          <footer class="dialog-footer">
            <button type="button" class="btn-cancel" :disabled="saving" @click="onClose">Cancel</button>
            <button
              type="button"
              class="btn-test"
              :disabled="saving || testRunning"
              @click="onTest"
            >
              <svg v-if="testRunning" class="spin" viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2" />
              </svg>
              <svg v-else viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2" />
              </svg>
              <span>{{ testRunning ? "Testing..." : "Test" }}</span>
            </button>
            <button type="button" class="btn-save" :disabled="saving" @click="onSave">
              <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <path d="M19 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11l5 5v11a2 2 0 0 1-2 2z" />
                <polyline points="17 21 17 13 7 13 7 21" />
                <polyline points="7 3 7 8 15 8" />
              </svg>
              <span>{{ saving ? "Saving..." : "Save" }}</span>
            </button>
          </footer>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.modal-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
  padding: 20px;
}
.dialog {
  background: var(--laipe-bg-elevated, #ffffff);
  border: 1px solid var(--laipe-border, #d2d2d7);
  border-radius: 10px;
  max-width: 800px;
  width: 100%;
  max-height: 90vh;
  display: flex;
  flex-direction: column;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.18);
  overflow: hidden;
}
.dialog-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 20px;
  border-bottom: 1px solid var(--laipe-border, #e5e5e7);
  flex-shrink: 0;
}
.dialog-title {
  font-size: 0.95em;
  font-weight: 600;
  color: var(--laipe-text, #1d1d1f);
}
.close-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  background: transparent;
  border: 1px solid transparent;
  border-radius: 4px;
  color: var(--laipe-text-muted, #6e6e73);
  cursor: pointer;
}
.close-btn:hover:not(:disabled) {
  background: var(--laipe-bg, #f5f5f7);
  color: var(--laipe-text, #1d1d1f);
  border-color: var(--laipe-border, #e5e5e7);
}

.dialog-body {
  padding: 20px;
  overflow-y: auto;
  flex: 1;
}
.error {
  margin: 0 0 14px 0;
  padding: 8px 12px;
  background: rgba(255, 59, 48, 0.1);
  color: #ff3b30;
  border: 1px solid #ff3b30;
  border-radius: 4px;
  font-size: 0.82em;
}

.grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 20px;
}
.col {
  display: flex;
  flex-direction: column;
  gap: 14px;
}
.field {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.label {
  font-size: 0.72em;
  font-weight: 600;
  color: var(--laipe-text-muted, #6e6e73);
  text-transform: uppercase;
  letter-spacing: 0.3px;
}
.required {
  color: #ff3b30;
}
.field input,
.field select {
  padding: 8px 10px;
  border: 1px solid var(--laipe-border, #d2d2d7);
  border-radius: 6px;
  font-size: 0.85em;
  background: var(--laipe-bg, #ffffff);
  color: var(--laipe-text, #1d1d1f);
  font-family: inherit;
  outline: none;
}
.field input:focus,
.field select:focus {
  border-color: var(--laipe-accent, #007aff);
  box-shadow: 0 0 0 3px rgba(0, 122, 255, 0.15);
}
.field input:disabled,
.field select:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}
.field input.invalid {
  border-color: #ff3b30;
}
.field-hint {
  font-size: 0.7em;
  color: var(--laipe-text-muted, #a1a1a6);
  line-height: 1.4;
}
.field-hint code {
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  background: var(--laipe-bg-elevated, #f5f5f7);
  padding: 0 3px;
  border-radius: 2px;
  font-size: 0.95em;
}

.advanced-block {
  border-top: 1px solid var(--laipe-border, #e5e5e7);
  padding-top: 14px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.advanced-block summary {
  cursor: pointer;
  font-size: 0.78em;
  font-weight: 600;
  color: var(--laipe-text-muted, #6e6e73);
  user-select: none;
  list-style: none;
}
.advanced-block summary::-webkit-details-marker { display: none; }
.advanced-block summary::before {
  content: "▸";
  display: inline-block;
  margin-right: 6px;
  transition: transform 0.15s ease;
}
.advanced-block[open] summary::before {
  transform: rotate(90deg);
}

/* === Test status bar === */
.dialog-status {
  padding: 0 20px 12px;
  flex-shrink: 0;
}
.status-error {
  padding: 8px 12px;
  background: rgba(255, 59, 48, 0.1);
  color: #ff3b30;
  border: 1px solid #ff3b30;
  border-radius: 4px;
  font-size: 0.82em;
}
.test-result {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border-radius: 4px;
  font-size: 0.82em;
  flex-wrap: wrap;
}
.test-result.ok {
  background: rgba(52, 199, 89, 0.1);
  color: #34c759;
  border: 1px solid #34c759;
}
.test-result.fail {
  background: rgba(255, 59, 48, 0.1);
  color: #ff3b30;
  border: 1px solid #ff3b30;
}
.test-result.testing {
  background: rgba(0, 122, 255, 0.1);
  color: var(--laipe-accent, #007aff);
  border: 1px solid var(--laipe-accent, #007aff);
}
.test-response,
.test-error {
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  background: var(--laipe-bg, #fafafa);
  border: 1px solid var(--laipe-border, #e5e5e7);
  padding: 1px 5px;
  border-radius: 3px;
  font-size: 0.78em;
  word-break: break-all;
  color: var(--laipe-text, #1d1d1f);
  flex-basis: 100%;
}
.spin {
  animation: spin 0.8s linear infinite;
}
@keyframes spin {
  to { transform: rotate(360deg); }
}

.dialog-footer {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
  padding: 12px 20px;
  border-top: 1px solid var(--laipe-border, #e5e5e7);
  background: var(--laipe-bg, #fafafa);
  flex-shrink: 0;
}
.btn-cancel,
.btn-test,
.btn-save {
  padding: 7px 18px;
  border-radius: 6px;
  font-size: 0.85em;
  font-family: inherit;
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  gap: 5px;
}
.btn-cancel {
  background: transparent;
  border: 1px solid var(--laipe-border, #d2d2d7);
  color: var(--laipe-text, #1d1d1f);
}
.btn-cancel:hover:not(:disabled) {
  border-color: var(--laipe-accent, #007aff);
}
.btn-test {
  background: transparent;
  border: 1px solid var(--laipe-border, #d2d2d7);
  color: var(--laipe-text, #1d1d1f);
}
.btn-test:hover:not(:disabled) {
  border-color: var(--laipe-accent, #007aff);
  color: var(--laipe-accent, #007aff);
}
.btn-save {
  background: var(--laipe-accent, #007aff);
  border: 1px solid var(--laipe-accent, #007aff);
  color: white;
}
.btn-save:hover:not(:disabled) {
  opacity: 0.88;
}
.btn-save:disabled,
.btn-cancel:disabled,
.btn-test:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.modal-enter-active,
.modal-leave-active {
  transition: opacity 0.18s ease;
}
.modal-enter-active .dialog,
.modal-leave-active .dialog {
  transition: transform 0.18s ease;
}
.modal-enter-from, .modal-leave-to {
  opacity: 0;
}
.modal-enter-from .dialog, .modal-leave-to .dialog {
  transform: scale(0.96) translateY(-8px);
}
</style>
