<script setup lang="ts">
// ProviderEditModal — PlotCraft-equivalent provider add/edit modal
//
// 设计参照 PlotCraft/src/components/settings/ProviderEditModal.vue:
// - 2 阶段流程 (v0.2+ 仿 Locus pick → config):
//   - stage='pick': ProviderCatalogStep (搜索框 + 手动添加卡 + catalog 列表)
//   - stage='config': 左 connection 字段, 右 model 列表 + ModelLibraryPanel
// - 中间状态条: local error + test result (连接成功 / 失败 + 错误码 + 模型返回片段)
// - Footer: 取消 / Test / 保存
// - 顶 ← 返回按钮 (仅 isNew=true + stage=config 时显示, 仿 Locus)
//
// v0.2 laipe-app 简化 (vs PlotCraft):
// - 字段 camelCase (baseUrl/apiKey/defaultModel) 跟 useProviderConfig 对齐
// - effort 6 levels (none/low/medium/high/xhigh/max) 跟 PlotCraft 对齐
// - 不做 enable/disable checkbox (在 panel card 上已经做了), id 在 edit 模式锁住
// - Test connection 走 laipe-ts testProvider (跨 laipe-ts / laipe-streaming
//   共享的 1:1 实现, 3 协议 URL + auth + body + 响应解析)

import { computed, ref, watch } from "vue";
import type { ApiFormat, TestProviderResult } from "laipe-ts";
import { testProvider } from "laipe-ts";
import type { CustomProvider, ProviderModel } from "../../composables/useProviderConfig";
import { API_FORMAT_LABELS, DEFAULT_API_FORMAT, EFFORT_LABELS, EFFORT_ORDER, DEFAULT_ENDPOINTS } from "../../lib/settings";
import type { CatalogModel, CatalogProvider } from "../../types/catalog";
import ProviderCatalogStep from "./ProviderCatalogStep.vue";
import ModelLibraryPanel from "./ModelLibraryPanel.vue";

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
const draftApiFormat = ref<ApiFormat>(DEFAULT_API_FORMAT);
const draftEnabled = ref(true);
const draftEffort = ref<string>("none");
const draftMaxTokens = ref<number | null>(null);
const draftTemperature = ref<number | null>(null);
const draftModels = ref<ProviderModel[]>([]);
const draftDefaultModel = ref("");

const localError = ref<string | null>(null);
const saving = ref(false);

// Test connection state
const testRunning = ref(false);
const testResult = ref<TestProviderResult | null>(null);

// 2 阶段: pick (isNew + start) → config (edit or after pick)
const stage = ref<"pick" | "config">("config");

// "手动添加 model" inline form state
const showManualForm = ref(false);
const manualId = ref("");
const manualName = ref("");
const manualError = ref<string | null>(null);

// === Init / reset ===
function resetDrafts(): void {
  draftId.value = "";
  draftName.value = "";
  draftEndpoint.value = "";
  draftApiKey.value = "";
  draftApiFormat.value = DEFAULT_API_FORMAT;
  draftEnabled.value = true;
  draftEffort.value = "none";
  draftMaxTokens.value = null;
  draftTemperature.value = null;
  draftModels.value = [];
  draftDefaultModel.value = "";
  localError.value = null;
  testRunning.value = false;
  testResult.value = null;
  showManualForm.value = false;
  manualId.value = "";
  manualName.value = "";
  manualError.value = null;
}

function populateFromProvider(p: CustomProvider): void {
  draftId.value = p.id;
  draftName.value = p.name;
  draftEndpoint.value = p.baseUrl;
  draftApiKey.value = p.apiKey;
  draftApiFormat.value = p.apiFormat;
  draftEnabled.value = p.enabled;
  draftModels.value = p.models ? p.models.map((m) => ({ ...m })) : [];
  draftDefaultModel.value = p.defaultModel;
  // v0.2 laipe-app 简化: 旧盘没 effort/max_tokens/temperature 字段, 默认 none/null
  draftEffort.value = "none";
  draftMaxTokens.value = null;
  draftTemperature.value = null;
}

watch(
  () => props.provider,
  (p) => {
    if (!p) return;
    saving.value = false;
    resetDrafts();
    if (props.isNew) {
      stage.value = "pick";
      resetDrafts();
    } else {
      stage.value = "config";
      populateFromProvider(p);
    }
  },
  { immediate: true },
);

const dialogTitle = computed(() => {
  if (props.isNew && stage.value === "pick") return "Add provider";
  if (props.isNew) return "Add provider · Configure";
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
    draftEndpoint.value = DEFAULT_ENDPOINTS[fmt];
  }
}

function onPickCatalog(payload: { provider: CatalogProvider; firstModel: CatalogModel }) {
  const { provider, firstModel } = payload;
  // draftId 用 provider + model id 拼 (小写 + dash)
  draftId.value = `${provider.id}-${firstModel.id}`.replace(/[^a-z0-9-]/g, "-");
  draftName.value = `${provider.name} / ${firstModel.name}`;
  draftEndpoint.value = provider.endpoint;
  draftApiKey.value = "";
  draftApiFormat.value = provider.suggested_api_format as ApiFormat;
  draftEnabled.value = true;
  draftModels.value = [{ id: firstModel.id, name: firstModel.name }];
  draftDefaultModel.value = firstModel.id;
  stage.value = "config";
}

function onPickManual() {
  resetDrafts();
  stage.value = "config";
}

function onBackToPick() {
  if (!props.isNew) return;
  stage.value = "pick";
}

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
  if (draftDefaultModel.value.trim()) {
    const exists = draftModels.value.some(
      (m) => m.id === draftDefaultModel.value.trim(),
    );
    if (!exists) {
      localError.value = `Default Model "${draftDefaultModel.value}" not in models list`;
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
    baseUrl: draftEndpoint.value.trim(),
    apiKey: draftApiKey.value,
    apiFormat: draftApiFormat.value,
    enabled: draftEnabled.value,
    models: draftModels.value.map((m) => ({
      id: m.id.trim(),
      name: m.name.trim() || m.id.trim(),
    })),
    defaultModel: draftDefaultModel.value.trim(),
  };
}

function onSave(): void {
  if (!validate()) return;
  saving.value = true;
  emit("save", buildProvider());
}

async function onTest(): Promise<void> {
  const testModel =
    draftDefaultModel.value.trim() || draftModels.value[0]?.id?.trim() || "";
  if (!draftEndpoint.value.trim() || !draftApiFormat.value || !testModel) {
    testResult.value = {
      ok: false,
      status: null,
      error: "Endpoint + API Format + 至少 1 个 model 三个都得填",
      response: null,
      endpoint: draftEndpoint.value,
      model: testModel,
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
      model: testModel,
    });
  } catch (e) {
    testResult.value = {
      ok: false,
      status: null,
      error: e instanceof Error ? e.message : String(e),
      response: null,
      endpoint: draftEndpoint.value,
      model: testModel,
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

// === Models 增删 (v0.2+ 镜像 PlotCraft multi-model 列表) ===

function openManualForm() {
  showManualForm.value = true;
  manualError.value = null;
  manualId.value = "";
  manualName.value = "";
}
function closeManualForm() {
  showManualForm.value = false;
  manualId.value = "";
  manualName.value = "";
  manualError.value = null;
}

function submitManualAdd() {
  const id = manualId.value.trim();
  const name = manualName.value.trim() || id;
  if (!id) {
    manualError.value = "Model id 不能为空";
    return;
  }
  if (draftModels.value.some((m) => m.id === id)) {
    manualError.value = `Model id "${id}" 已存在`;
    return;
  }
  draftModels.value = [...draftModels.value, { id, name }];
  if (!draftDefaultModel.value.trim()) {
    draftDefaultModel.value = id;
  }
  closeManualForm();
}

function addFromLibrary(payload: { model: CatalogModel; provider: CatalogProvider }) {
  const { model } = payload;
  if (draftModels.value.some((x) => x.id === model.id)) return;
  draftModels.value = [
    ...draftModels.value,
    { id: model.id, name: model.name || model.id },
  ];
  if (!draftDefaultModel.value.trim()) {
    draftDefaultModel.value = model.id;
  }
}

const existingModelIds = computed(() => draftModels.value.map((m) => m.id));

function removeModel(id: string) {
  draftModels.value = draftModels.value.filter((m) => m.id !== id);
  if (draftDefaultModel.value === id) {
    draftDefaultModel.value = draftModels.value[0]?.id ?? "";
  }
}

function setAsDefault(id: string) {
  draftDefaultModel.value = id;
}

const libraryExpanded = ref(true);
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
            <div class="dialog-header-lead">
              <button
                v-if="isNew && stage === 'config'"
                class="back-btn"
                type="button"
                :disabled="saving"
                title="返回 catalog 选"
                @click="onBackToPick"
              >
                ‹
              </button>
              <span class="dialog-title">{{ dialogTitle }}</span>
            </div>
            <button class="close-btn" type="button" :disabled="saving" @click="onClose">
              ✕
            </button>
          </header>

          <!-- Pick stage -->
          <ProviderCatalogStep
            v-if="isNew && stage === 'pick'"
            :disabled="saving"
            @pick-catalog="onPickCatalog"
            @pick-manual="onPickManual"
          />

          <!-- Config stage: 2 栏 (左 connection, 右 model) -->
          <div v-else-if="stage === 'config'" class="config-body">
            <p v-if="localError" class="error">{{ localError }}</p>

            <div class="grid">
              <!-- Left: connection fields -->
              <div class="col">
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
                  <span class="label">Endpoint <span class="required">*</span></span>
                  <textarea
                    :value="draftEndpoint"
                    :class="{ invalid: localError?.includes('Endpoint') }"
                    :disabled="saving"
                    rows="1"
                    spellcheck="false"
                    placeholder="https://api.openai.com/v1"
                    @input="(e) => (draftEndpoint = (e.target as HTMLTextAreaElement).value.replace(/\s+/g, ''))"
                    @keydown.enter.prevent
                  ></textarea>
                </label>

                <label class="field">
                  <span class="label">API Format</span>
                  <select
                    :value="draftApiFormat"
                    :disabled="saving"
                    @change="(e) => onFormatChange((e.target as HTMLSelectElement).value as ApiFormat)"
                  >
                    <option
                      v-for="(label, fmt) in API_FORMAT_LABELS"
                      :key="fmt"
                      :value="fmt"
                    >
                      {{ label }}
                    </option>
                  </select>
                </label>

                <details class="advanced-block">
                  <summary>Advanced</summary>
                  <label class="field">
                    <span class="label">Effort</span>
                    <select
                      :value="draftEffort"
                      :disabled="saving"
                      @change="(e) => (draftEffort = (e.target as HTMLSelectElement).value)"
                    >
                      <option
                        v-for="lvl in EFFORT_ORDER"
                        :key="lvl"
                        :value="lvl"
                      >
                        {{ EFFORT_LABELS[lvl] }}
                      </option>
                    </select>
                    <small class="field-hint">Reasoning effort / thinking level. 6 levels (none/low/medium/high/xhigh/max) — per-run override at chat time.</small>
                  </label>
                  <label class="field">
                    <span class="label">Max Tokens</span>
                    <input v-model="maxTokensStr" type="text" inputmode="numeric" :disabled="saving" placeholder="leave empty for upstream default" />
                  </label>
                  <label class="field">
                    <span class="label">Temperature</span>
                    <input v-model="temperatureStr" type="text" inputmode="decimal" :disabled="saving" placeholder="leave empty for upstream default" />
                  </label>
                </details>
              </div>

              <!-- Right: model list (v0.2+ 跟 PlotCraft 同款多 model) -->
              <div class="col">
                <div class="models-header">
                  <span class="models-title">Models</span>
                  <span v-if="draftModels.length > 0" class="models-count">
                    已加 {{ draftModels.length }} 个
                  </span>
                  <span v-else class="models-count empty">未添加</span>
                  <div class="models-actions">
                    <button type="button" class="add-model-btn" :disabled="saving" @click="openManualForm">
                      手动添加
                    </button>
                  </div>
                </div>

                <!-- Manual add form -->
                <div v-if="showManualForm" class="manual-form">
                  <p v-if="manualError" class="manual-error">{{ manualError }}</p>
                  <input
                    v-model="manualId"
                    type="text"
                    class="manual-input"
                    placeholder="model id (如 gpt-4o-mini)"
                    spellcheck="false"
                  />
                  <input
                    v-model="manualName"
                    type="text"
                    class="manual-input"
                    placeholder="display name (可选, 缺省 = id)"
                  />
                  <div class="manual-form-actions">
                    <button type="button" class="btn-cancel-sm" @click="closeManualForm">取消</button>
                    <button type="button" class="btn-save-sm" @click="submitManualAdd">添加</button>
                  </div>
                </div>

                <!-- Default model picker -->
                <label v-if="draftModels.length > 0" class="field">
                  <span class="label">Default Model</span>
                  <select
                    :value="draftDefaultModel"
                    :disabled="saving"
                    @change="(e) => (draftDefaultModel = (e.target as HTMLSelectElement).value)"
                  >
                    <option
                      v-for="m in draftModels"
                      :key="m.id"
                      :value="m.id"
                    >
                      {{ m.name }} ({{ m.id }})
                    </option>
                  </select>
                </label>

                <!-- Models list (cards) -->
                <div v-if="draftModels.length > 0" class="models-list">
                  <div
                    v-for="m in draftModels"
                    :key="m.id"
                    class="model-card"
                    :class="{ default: m.id === draftDefaultModel }"
                  >
                    <div class="model-card-info">
                      <div class="model-card-name">{{ m.name }}</div>
                      <code class="model-card-id">{{ m.id }}</code>
                    </div>
                    <div class="model-card-actions">
                      <button
                        v-if="m.id !== draftDefaultModel"
                        type="button"
                        class="model-card-btn"
                        :disabled="saving"
                        @click="setAsDefault(m.id)"
                      >
                        设默认
                      </button>
                      <span v-else class="model-card-default-tag">default</span>
                      <button
                        type="button"
                        class="model-card-btn danger"
                        :disabled="saving"
                        @click="removeModel(m.id)"
                      >
                        ✕
                      </button>
                    </div>
                  </div>
                </div>

                <!-- Model library (v0.2+ PlotCraft 同款) -->
                <ModelLibraryPanel
                  v-model:expanded="libraryExpanded"
                  :existing-model-ids="existingModelIds"
                  :disabled="saving"
                  @add-model="addFromLibrary"
                />
              </div>
            </div>
          </div>

          <!-- Status bar: local error + test result -->
          <div
            v-if="(localError && stage === 'config') || testResult || testRunning"
            class="dialog-status"
          >
            <div v-if="testResult || testRunning" class="test-result" :class="{ ok: testResult?.ok, fail: testResult && !testResult.ok, testing: testRunning }">
              <span v-if="testRunning">Testing...</span>
              <span v-else-if="testResult?.ok">
                ✓ Connected<span v-if="testResult.status"> (HTTP {{ testResult.status }})</span>
              </span>
              <span v-else-if="testResult">
                ✗ Failed<span v-if="testResult.status"> (HTTP {{ testResult.status }})</span>
              </span>
              <code v-if="testResult?.response" class="test-response">
                {{ testResult.response }}
              </code>
              <code v-if="testResult?.error" class="test-error">
                {{ testResult.error }}
              </code>
            </div>
          </div>

          <footer v-if="stage === 'config'" class="dialog-footer">
            <button type="button" class="btn-cancel" :disabled="saving" @click="onClose">Cancel</button>
            <button type="button" class="btn-test" :disabled="saving || testRunning" @click="onTest">
              {{ testRunning ? "Testing..." : "Test" }}
            </button>
            <button type="button" class="btn-save" :disabled="saving" @click="onSave">
              {{ saving ? "Saving..." : "Save" }}
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
  max-width: 1080px;
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
.dialog-header-lead {
  display: flex;
  align-items: center;
  gap: 10px;
}
.back-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  background: transparent;
  color: var(--laipe-text-muted, #6e6e73);
  border: 1px solid var(--laipe-border, #d2d2d7);
  border-radius: 5px;
  cursor: pointer;
  font-size: 16px;
}
.back-btn:hover:not(:disabled) {
  background: var(--laipe-bg, #f5f5f7);
  color: var(--laipe-text, #1d1d1f);
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
  font-size: 14px;
}
.close-btn:hover:not(:disabled) {
  background: var(--laipe-bg, #f5f5f7);
  color: var(--laipe-text, #1d1d1f);
  border-color: var(--laipe-border, #e5e5e7);
}

.config-body {
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
.field select,
.field textarea {
  padding: 8px 10px;
  border: 1px solid var(--laipe-border, #d2d2d7);
  border-radius: 6px;
  font-size: 0.85em;
  background: var(--laipe-bg, #ffffff);
  color: var(--laipe-text, #1d1d1f);
  font-family: inherit;
  outline: none;
  resize: vertical;
}
.field input:focus,
.field select:focus,
.field textarea:focus {
  border-color: var(--laipe-accent, #007aff);
  box-shadow: 0 0 0 3px rgba(0, 122, 255, 0.15);
}
.field input:disabled,
.field select:disabled,
.field textarea:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}
.field input.invalid,
.field textarea.invalid {
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
  border: 1px solid var(--laipe-border, #e5e5e7);
  border-radius: 6px;
  padding: 8px 12px;
  background: var(--laipe-bg, #fafafa);
}
.advanced-block summary {
  cursor: pointer;
  font-size: 0.78em;
  font-weight: 600;
  color: var(--laipe-text-muted, #6e6e73);
  user-select: none;
  padding: 2px 0;
}
.advanced-block[open] summary {
  margin-bottom: 8px;
}
.advanced-block .field {
  margin-top: 8px;
}

/* Models column */
.models-header {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}
.models-title {
  font-size: 0.72em;
  font-weight: 600;
  color: var(--laipe-text-muted, #6e6e73);
  text-transform: uppercase;
  letter-spacing: 0.3px;
}
.models-count {
  font-size: 0.7em;
  color: var(--laipe-text-muted, #6e6e73);
  padding: 1px 6px;
  background: var(--laipe-bg, #f5f5f7);
  border-radius: 3px;
}
.models-count.empty {
  font-style: italic;
}
.models-actions {
  margin-left: auto;
}
.add-model-btn {
  font-size: 0.78em;
  padding: 4px 10px;
  border: 1px solid var(--laipe-border, #d2d2d7);
  border-radius: 4px;
  background: var(--laipe-bg, #ffffff);
  color: var(--laipe-text, #1d1d1f);
  cursor: pointer;
  font-family: inherit;
}
.add-model-btn:hover:not(:disabled) {
  background: var(--laipe-bg, #f5f5f7);
  border-color: var(--laipe-accent, #007aff);
}

.manual-form {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 8px;
  background: var(--laipe-bg, #fafafa);
  border: 1px solid var(--laipe-border, #d2d2d7);
  border-radius: 6px;
}
.manual-error {
  margin: 0;
  font-size: 0.75em;
  color: #ff3b30;
}
.manual-input {
  padding: 5px 8px;
  border: 1px solid var(--laipe-border, #d2d2d7);
  border-radius: 4px;
  font-size: 0.8em;
  background: var(--laipe-bg, #ffffff);
  color: var(--laipe-text, #1d1d1f);
  font-family: inherit;
  outline: none;
}
.manual-input:focus {
  border-color: var(--laipe-accent, #007aff);
}
.manual-form-actions {
  display: flex;
  gap: 6px;
  justify-content: flex-end;
}
.btn-cancel-sm,
.btn-save-sm {
  padding: 3px 10px;
  font-size: 0.75em;
  border-radius: 4px;
  cursor: pointer;
  font-family: inherit;
}
.btn-cancel-sm {
  background: transparent;
  border: 1px solid var(--laipe-border, #d2d2d7);
  color: var(--laipe-text, #1d1d1f);
}
.btn-save-sm {
  background: var(--laipe-accent, #007aff);
  border: 1px solid var(--laipe-accent, #007aff);
  color: white;
}
.btn-cancel-sm:hover {
  background: var(--laipe-bg, #f5f5f7);
}

.models-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
  max-height: 200px;
  overflow-y: auto;
}
.model-card {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 8px;
  background: var(--laipe-bg, #ffffff);
  border: 1px solid var(--laipe-border, #d2d2d7);
  border-radius: 5px;
  gap: 8px;
}
.model-card.default {
  border-color: var(--laipe-accent, #007aff);
  background: rgba(0, 122, 255, 0.04);
}
.model-card-info {
  flex: 1;
  min-width: 0;
}
.model-card-name {
  font-size: 0.82em;
  font-weight: 600;
  color: var(--laipe-text, #1d1d1f);
}
.model-card-id {
  font-size: 0.72em;
  color: var(--laipe-text-muted, #6e6e73);
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
}
.model-card-actions {
  display: flex;
  gap: 4px;
  align-items: center;
}
.model-card-btn {
  font-size: 0.7em;
  padding: 2px 8px;
  background: transparent;
  border: 1px solid var(--laipe-border, #d2d2d7);
  border-radius: 3px;
  color: var(--laipe-text, #1d1d1f);
  cursor: pointer;
  font-family: inherit;
}
.model-card-btn:hover:not(:disabled) {
  background: var(--laipe-bg, #f5f5f7);
}
.model-card-btn.danger:hover:not(:disabled) {
  background: rgba(255, 59, 48, 0.12);
  color: #ff3b30;
  border-color: #ff3b30;
}
.model-card-default-tag {
  font-size: 0.65em;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.4px;
  padding: 1px 6px;
  border-radius: 3px;
  color: var(--laipe-accent, #007aff);
  background: rgba(0, 122, 255, 0.12);
  border: 1px solid var(--laipe-accent, #007aff);
}

/* Status bar */
.dialog-status {
  padding: 10px 20px;
  border-top: 1px solid var(--laipe-border, #e5e5e7);
  flex-shrink: 0;
}
.test-result {
  display: flex;
  flex-direction: column;
  gap: 4px;
  font-size: 0.78em;
  padding: 6px 10px;
  border-radius: 4px;
  background: var(--laipe-bg, #f5f5f7);
}
.test-result.ok {
  color: #34c759;
  background: rgba(52, 199, 89, 0.1);
  border: 1px solid rgba(52, 199, 89, 0.3);
}
.test-result.fail {
  color: #ff3b30;
  background: rgba(255, 59, 48, 0.1);
  border: 1px solid rgba(255, 59, 48, 0.3);
}
.test-result.testing {
  color: var(--laipe-text-muted, #6e6e73);
}
.test-response,
.test-error {
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  font-size: 0.92em;
  word-break: break-all;
  background: rgba(0, 0, 0, 0.04);
  padding: 4px 6px;
  border-radius: 3px;
  color: var(--laipe-text, #1d1d1f);
}

.dialog-footer {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
  padding: 12px 20px;
  border-top: 1px solid var(--laipe-border, #e5e5e7);
  flex-shrink: 0;
}
.btn-cancel,
.btn-test,
.btn-save {
  padding: 6px 16px;
  font-size: 0.82em;
  border-radius: 6px;
  cursor: pointer;
  font-family: inherit;
  display: flex;
  align-items: center;
  gap: 6px;
}
.btn-cancel {
  background: transparent;
  border: 1px solid var(--laipe-border, #d2d2d7);
  color: var(--laipe-text, #1d1d1f);
}
.btn-test {
  background: var(--laipe-bg, #ffffff);
  border: 1px solid var(--laipe-border, #d2d2d7);
  color: var(--laipe-text, #1d1d1f);
}
.btn-save {
  background: var(--laipe-accent, #007aff);
  border: 1px solid var(--laipe-accent, #007aff);
  color: white;
}
.btn-cancel:hover:not(:disabled) {
  background: var(--laipe-bg, #f5f5f7);
}
.btn-test:hover:not(:disabled) {
  background: var(--laipe-bg, #f5f5f7);
  border-color: var(--laipe-accent, #007aff);
}
.btn-save:hover:not(:disabled) {
  opacity: 0.88;
}
.btn-cancel:disabled,
.btn-test:disabled,
.btn-save:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
</style>
