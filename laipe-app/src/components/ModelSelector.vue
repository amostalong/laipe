<script setup lang="ts">
// ModelSelector — dropdown for picking from a curated model catalog.
//
// Shows a trigger button (current model name + effort label). Click
// opens a popover with:
//   - List of models available for the current API format
//   - Effort level sub-selector (only when the picked model supports it)
//   - "Custom…" option to type any model id (for unlisted models)
//
// Used in laipe-app's Settings. The free-text fallback remains for
// power users and OpenRouter-style `vendor/model` ids.

import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import type { ApiFormat, EffortLevel } from "laipe-ts";
import {
  MODEL_CATALOG,
  cleanupModelId,
  findModel,
  type ModelInfo,
} from "../modelCatalog";

defineOptions({ name: "ModelSelector" });

const props = defineProps<{
  modelId: string;
  apiFormat: ApiFormat;
  effort: EffortLevel | null;
}>();

const emit = defineEmits<{
  "update:modelId": [id: string];
  "update:effort": [level: EffortLevel | null];
}>();

const open = ref(false);
const showCustomInput = ref(false);
const customDraft = ref("");
const triggerRef = ref<HTMLElement | null>(null);
const popoverRef = ref<HTMLElement | null>(null);

const EFFORT_OPTIONS: { value: EffortLevel; label: string; color: string }[] = [
  { value: "low", label: "Low", color: "#38a169" },
  { value: "medium", label: "Med", color: "#d69e2e" },
  { value: "high", label: "High", color: "#dd6b20" },
];

/** Models filtered by current API format. */
const availableModels = computed<ModelInfo[]>(() =>
  MODEL_CATALOG.filter((m) => m.api_formats.includes(props.apiFormat)),
);

/** The currently selected model (curated) — or undefined for custom. */
const currentModel = computed<ModelInfo | undefined>(() => findModel(props.modelId));

/** True when the model supports reasoning effort. */
const effortSupported = computed<boolean>(() => {
  const m = currentModel.value;
  return !!m && m.supported_efforts.length > 0;
});

/** Display name for the trigger button. */
const triggerLabel = computed<string>(() => {
  const m = currentModel.value;
  if (m) return m.name;
  // Custom / unknown id → cleaned + truncated
  return props.modelId ? cleanupModelId(props.modelId) : "Select model…";
});

/** Effort chip text (null = don't show). */
const effortChip = computed<string | null>(() => {
  if (!effortSupported.value) return null;
  if (!props.effort) return null;
  const opt = EFFORT_OPTIONS.find((e) => e.value === props.effort);
  return opt?.label ?? null;
});

const effortChipColor = computed<string | undefined>(() => {
  if (!effortSupported.value || !props.effort) return undefined;
  return EFFORT_OPTIONS.find((e) => e.value === props.effort)?.color;
});

/**
 * When the format changes, if the current model (in the catalog) doesn't
 * support the new format, switch to the first available one. Custom models
 * (not in the catalog, i.e. !m) are preserved — the player picked them
 * explicitly and we shouldn't silently overwrite.
 */
watch(
  () => props.apiFormat,
  () => {
    const m = currentModel.value;
    if (m && !m.api_formats.includes(props.apiFormat)) {
      const fallback = availableModels.value[0];
      if (fallback) {
        emit("update:modelId", fallback.id);
        emit("update:effort", fallback.default_effort);
      }
    }
  },
);

function toggle() {
  open.value = !open.value;
  if (open.value) {
    showCustomInput.value = false;
    customDraft.value = props.modelId;
  }
}

function pickModel(m: ModelInfo) {
  emit("update:modelId", m.id);
  // Reset effort to the model's default (or null if unsupported).
  if (m.supported_efforts.length === 0) {
    emit("update:effort", null);
  } else {
    emit("update:effort", m.default_effort);
  }
  // Keep popover open for the effort sub-selector; close on backdrop click.
}

function pickCustom() {
  showCustomInput.value = true;
}

function commitCustom() {
  const v = customDraft.value.trim();
  if (v) emit("update:modelId", v);
  emit("update:effort", null);
  showCustomInput.value = false;
  open.value = false;
}

function pickEffort(level: EffortLevel | null) {
  emit("update:effort", level);
  open.value = false;
}

function onBackdropClick() {
  open.value = false;
}

function onClickOutside(e: MouseEvent) {
  if (!open.value) return;
  const t = e.target as Node;
  if (
    triggerRef.value && !triggerRef.value.contains(t) &&
    popoverRef.value && !popoverRef.value.contains(t)
  ) {
    open.value = false;
  }
}

onMounted(() => document.addEventListener("mousedown", onClickOutside));
onUnmounted(() => document.removeEventListener("mousedown", onClickOutside));
</script>

<template>
  <div class="model-selector">
    <button
      ref="triggerRef"
      type="button"
      class="trigger"
      :class="{ open, empty: !modelId }"
      :title="modelId || 'Select model'"
      @click="toggle"
    >
      <span class="model-name">{{ triggerLabel }}</span>
      <span
        v-if="effortChip"
        class="effort-chip"
        :style="{ color: effortChipColor }"
      >{{ effortChip }}</span>
      <span class="chevron">▾</span>
    </button>

    <Transition name="popover">
      <div
        v-if="open"
        ref="popoverRef"
        class="popover"
        :class="{ 'has-effort': effortSupported }"
      >
        <div v-if="!showCustomInput" class="model-panel">
          <div class="section-label">Model</div>
          <button
            v-for="m in availableModels"
            :key="m.id"
            type="button"
            class="option"
            :class="{ active: m.id === modelId }"
            @click="pickModel(m)"
          >
            <span class="option-name">{{ m.name }}</span>
            <span v-if="m.note" class="option-note">{{ m.note }}</span>
          </button>
          <div class="divider"></div>
          <button
            type="button"
            class="option custom"
            :class="{ active: !currentModel }"
            @click="pickCustom"
          >
            <span class="option-name">Custom…</span>
            <span class="option-note">Type any model id (e.g. via OpenRouter)</span>
          </button>
        </div>

        <div v-else class="custom-panel">
          <div class="section-label">Custom model id</div>
          <input
            v-model="customDraft"
            type="text"
            class="custom-input"
            placeholder="vendor/model-name or model-id"
            autofocus
            @keydown.enter="commitCustom"
            @keydown.escape="open = false"
          />
          <div class="custom-actions">
            <button type="button" class="btn-cancel" @click="open = false">Cancel</button>
            <button type="button" class="btn-ok" @click="commitCustom">Use</button>
          </div>
        </div>

        <div v-if="effortSupported && !showCustomInput" class="effort-panel">
          <div class="section-label">Effort</div>
          <button
            v-for="opt in EFFORT_OPTIONS"
            :key="opt.value"
            type="button"
            class="option"
            :class="{ active: effort === opt.value }"
            @click="pickEffort(opt.value)"
          >
            <span class="option-name" :style="effort === opt.value ? { color: opt.color, fontWeight: 600 } : {}">
              {{ opt.label }}
            </span>
          </button>
          <button
            type="button"
            class="option"
            :class="{ active: effort === null }"
            @click="pickEffort(null)"
          >
            <span class="option-name">None</span>
          </button>
        </div>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.model-selector {
  position: relative;
  display: inline-flex;
  flex-shrink: 1;
  min-width: 0;
  width: 100%;
}
.trigger {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  min-width: 0;
  min-height: 36px;
  padding: 6px 10px;
  border: 1px solid var(--laipe-border-strong, #d2d2d7);
  border-radius: 6px;
  background: var(--laipe-bg-elevated, #ffffff);
  color: var(--laipe-text, #1d1d1f);
  font-size: 0.9em;
  font-family: inherit;
  cursor: pointer;
  text-align: left;
  white-space: nowrap;
  transition: border-color 0.15s ease, box-shadow 0.15s ease;
}
.trigger:hover,
.trigger.open {
  border-color: var(--laipe-accent, #007aff);
  box-shadow: 0 0 0 3px rgba(0, 122, 255, 0.15);
}
.trigger.empty .model-name {
  color: var(--laipe-text-muted, #a1a1a6);
  font-style: italic;
}
.model-name {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  font-weight: 500;
}
.effort-chip {
  flex-shrink: 0;
  font-weight: 600;
  font-size: 0.85em;
  letter-spacing: 0.2px;
}
.chevron {
  flex-shrink: 0;
  font-size: 0.75em;
  color: var(--laipe-text-muted, #a1a1a6);
  transition: transform 0.15s ease;
}
.trigger.open .chevron {
  transform: rotate(180deg);
}

.popover {
  position: absolute;
  top: calc(100% + 6px);
  left: 0;
  right: 0;
  min-width: 280px;
  max-height: 400px;
  overflow: hidden;
  padding: 4px;
  border: 1px solid var(--laipe-border, #d2d2d7);
  border-radius: 10px;
  background: var(--laipe-bg-elevated, #ffffff);
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.12);
  z-index: 200;
}
.popover.has-effort {
  display: grid;
  grid-template-columns: 1fr 96px;
}

.model-panel,
.effort-panel {
  min-width: 0;
  max-height: 388px;
  overflow-y: auto;
}
.effort-panel {
  border-left: 1px solid var(--laipe-border, #e5e5e7);
  padding-left: 4px;
}

.section-label {
  padding: 6px 10px 4px;
  font-size: 0.7em;
  font-weight: 500;
  letter-spacing: 0.3px;
  color: var(--laipe-text-muted, #6e6e73);
  text-transform: uppercase;
}
.divider {
  height: 1px;
  margin: 4px 8px;
  background: var(--laipe-border, #e5e5e7);
}
.option {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  width: 100%;
  padding: 6px 10px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: inherit;
  font-family: inherit;
  text-align: left;
  cursor: pointer;
  gap: 1px;
}
.option:hover {
  background: var(--laipe-bg, #f5f5f7);
}
.option.active {
  background: rgba(0, 122, 255, 0.12);
}
.option-name {
  font-size: 0.9em;
  font-weight: 500;
  color: var(--laipe-text, #1d1d1f);
}
.option.active .option-name {
  color: var(--laipe-accent, #007aff);
}
.option-note {
  font-size: 0.75em;
  color: var(--laipe-text-muted, #6e6e73);
}
.option.custom .option-name {
  font-style: italic;
}

.custom-panel {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 8px;
}
.custom-input {
  width: 100%;
  padding: 6px 8px;
  border: 1px solid var(--laipe-border-strong, #d2d2d7);
  border-radius: 4px;
  font-size: 0.85em;
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  background: var(--laipe-bg, #ffffff);
  color: var(--laipe-text, #1d1d1f);
  outline: none;
  box-sizing: border-box;
}
.custom-input:focus {
  border-color: var(--laipe-accent, #007aff);
}
.custom-actions {
  display: flex;
  gap: 6px;
  justify-content: flex-end;
}
.btn-cancel,
.btn-ok {
  padding: 4px 12px;
  border-radius: 4px;
  border: 1px solid var(--laipe-border-strong, #d2d2d7);
  font-size: 0.8em;
  font-family: inherit;
  cursor: pointer;
  background: var(--laipe-bg-elevated, #ffffff);
  color: var(--laipe-text, #1d1d1f);
}
.btn-ok {
  background: var(--laipe-accent, #007aff);
  color: white;
  border-color: var(--laipe-accent, #007aff);
}

.popover-enter-active,
.popover-leave-active {
  transition: opacity 0.12s ease, transform 0.12s ease;
}
.popover-enter-from,
.popover-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}
</style>
