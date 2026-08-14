<script setup lang="ts">
// ModelEffortSelector — chat composer 左下角的 model + effort 选择器
//
// 视觉 + 交互镜像 Locus / PlotCraft `ModelEffortSelector.vue`:
// - 单 trigger button: model 名 + effort 标签 (带颜色) + chevron ▾
// - 点开 → 双 panel 下拉:
//   - 左 panel: models 按 custom provider 分段 (每个 provider 一个段头)
//   - 右 panel: effort 列表 (None / Low / Med / High / XHigh / Max)
// - 位置: trigger 上方弹出
// - 交互: click outside 关闭 + transition
// - 颜色: effort 等级映射 (low=绿 / med=黄 / high/xhigh/max=橙)
//
// v0.2 laipe-app 简化 (vs PlotCraft):
// - 没有 streaming disabled (laipe-vue useChat 自带 status)
// - 不接 fast mode
// - selection change 不直接改 settings.config, 调 emit 让 MainView 处理

import { computed, onMounted, onUnmounted, ref } from "vue";
import {
  groupCustomProviderShortcuts,
  asCustomProviderShortcuts,
  cleanupModelId,
  type ModelSelectorGroup,
} from "../../lib/modelCatalog";
import { EFFORT_LABELS, type EffortLevel } from "../../lib/settings";
import { useProviderConfig } from "../../composables/useProviderConfig";

const props = defineProps<{
  /** 当前选中的 model id */
  selectedId: string;
  /** 当前 effort */
  effort: EffortLevel;
  /** 当前 model 是否支持 effort (false → 右 panel 隐藏) */
  effortSupported?: boolean;
  /** 弹层对齐: start=左对齐, end=右对齐 (默认) */
  align?: "start" | "end";
  /** 弹层位置: top=trigger 上方, bottom=trigger 下方 */
  placement?: "top" | "bottom";
  disabled?: boolean;
  /** 没填 model 的 enabled provider 数 — empty state 文案分流 */
  unconfiguredProviderCount?: number;
}>();

const emit = defineEmits<{
  selectModel: [id: string];
  selectEffort: [level: EffortLevel];
}>();

const open = ref(false);
const selectorRef = ref<HTMLElement | null>(null);

const { providers } = useProviderConfig();

/** 玩家 enabled + 有 defaultModel 的 custom provider 段头列表 */
const customProviderShortcuts = computed(() =>
  asCustomProviderShortcuts(providers.value),
);

/** 当前选中的 custom provider shortcut (selectedId 匹配某个 custom provider 的 defaultModel) */
const selectedCustomShortcut = computed(() => {
  if (!props.selectedId) return null;
  return customProviderShortcuts.value.find(
    (cp) => cp.defaultModel === props.selectedId,
  ) ?? null;
});

/** trigger 按钮显示名 */
const TRIGGER_MAX_LEN = 24;
const selectedDisplayName = computed(() => {
  if (selectedCustomShortcut.value) {
    const cp = selectedCustomShortcut.value;
    return `${cp.name} / ${cleanupModelId(cp.defaultModel)}`;
  }
  const raw = props.selectedId;
  if (!raw) return "Select model";
  return cleanupModelId(raw);
});

/** 当前 model 支持的 effort 列表 — 全部 6 个都展示 (best-effort: 后端对不支持的 model 静默 no-op) */
const levels = computed<EffortLevel[]>(() => [
  "none",
  "low",
  "medium",
  "high",
  "xhigh",
  "max",
]);

/** 当前选中的 effort 在 trigger 里的显示 label */
const currentLevelLabel = computed<string | null>(() => {
  if (!props.effortSupported) return null;
  if (props.effort === "none") return null;
  return EFFORT_LABELS[props.effort];
});

/** grouped models (左 panel) */
const groupedModels = computed<ModelSelectorGroup[]>(() =>
  groupCustomProviderShortcuts(customProviderShortcuts.value),
);

/** trigger 整 title (hover tooltip) */
const triggerTitle = computed(() => {
  const modelTitle = selectedDisplayName.value;
  if (!props.effortSupported) return modelTitle;
  return `${modelTitle} · ${EFFORT_LABELS[props.effort]}`;
});

/** effort 颜色 */
function levelColor(level: EffortLevel): string {
  switch (level) {
    case "low":
      return "#38a169";
    case "medium":
      return "#d69e2e";
    case "high":
      return "#dd6b20";
    case "xhigh":
      return "#c05621";
    case "max":
      return "#c05621";
    default:
      return "var(--laipe-text-muted, #6e6e73)";
  }
}

function toggle() {
  if (props.disabled) return;
  open.value = !open.value;
}

function selectModel(id: string) {
  emit("selectModel", id);
}

function selectEffort(level: EffortLevel) {
  emit("selectEffort", level);
  open.value = false;
}

function onClickOutside(e: MouseEvent) {
  if (!open.value) return;
  if (selectorRef.value && !selectorRef.value.contains(e.target as Node)) {
    open.value = false;
  }
}

onMounted(() => document.addEventListener("mousedown", onClickOutside));
onUnmounted(() => document.removeEventListener("mousedown", onClickOutside));
</script>

<template>
  <div
    class="model-effort-selector"
    :class="{ open, 'align-end': align !== 'start', 'placement-bottom': placement === 'bottom' }"
    ref="selectorRef"
  >
    <button
      class="model-effort-trigger"
      :class="{ open, disabled, empty: !selectedId }"
      type="button"
      :title="triggerTitle"
      :disabled="disabled"
      @click="toggle"
    >
      <span class="model-effort-model">{{ selectedDisplayName }}</span>
      <span
        v-if="currentLevelLabel"
        class="model-effort-level"
        :style="{ color: levelColor(props.effort) }"
      >
        {{ currentLevelLabel }}
      </span>
      <span class="model-effort-chevron">▾</span>
    </button>

    <Transition name="dropdown">
      <div
        v-if="open"
        class="model-effort-dropdown"
        :class="{
          'has-effort': props.effortSupported !== false,
          'align-end': align !== 'start',
          'placement-bottom': placement === 'bottom',
        }"
      >
        <div class="model-effort-model-panel">
          <template v-if="groupedModels.length === 0">
            <div class="model-effort-empty">
              <template v-if="(props.unconfiguredProviderCount ?? 0) > 0">
                <p>有 {{ unconfiguredProviderCount }} 个 provider，但都没填 model</p>
                <p class="model-effort-empty-hint">Settings → Providers 库点 ✎ Edit → 加 model</p>
              </template>
              <template v-else>
                <p>未添加任何 provider</p>
                <p class="model-effort-empty-hint">Settings → Providers 加一个</p>
              </template>
            </div>
          </template>
          <template
            v-else
            v-for="(group, gi) in groupedModels"
            :key="group.key"
          >
            <div v-if="gi > 0" class="model-effort-divider"></div>
            <div
              class="model-effort-section-label"
              :class="{ uppercase: group.uppercaseLabel }"
            >
              {{ group.label }}
            </div>

            <button
              v-if="group.customProvider"
              type="button"
              class="model-effort-option"
              :class="{ active: group.customProvider.defaultModel === selectedId }"
              @click="selectModel(group.customProvider.defaultModel)"
            >
              <span class="model-effort-option-name">
                {{ cleanupModelId(group.customProvider.defaultModel) }}
              </span>
              <span
                v-if="group.customProvider.defaultModel === selectedId && currentLevelLabel"
                class="model-effort-option-tag"
                :style="{ color: levelColor(props.effort) }"
              >
                {{ currentLevelLabel }}
              </span>
            </button>
          </template>
        </div>

        <div
          v-if="props.effortSupported !== false"
          class="model-effort-effort-panel"
        >
          <div class="model-effort-section-label">Effort</div>
          <button
            v-for="level in levels"
            :key="level"
            type="button"
            class="model-effort-option"
            :class="{ active: level === props.effort }"
            @click="selectEffort(level)"
          >
            <span
              class="model-effort-option-name"
              :style="level === props.effort ? { color: levelColor(level), fontWeight: 600 } : {}"
            >
              {{ EFFORT_LABELS[level] }}
            </span>
          </button>
        </div>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.model-effort-selector {
  position: relative;
  display: inline-flex;
  flex-shrink: 1;
  min-width: 0;
  margin-right: 4px;
}
.model-effort-selector.open {
  z-index: 50;
}

.model-effort-trigger {
  display: flex;
  align-items: center;
  gap: 5px;
  min-width: 0;
  min-height: 28px;
  max-width: min(280px, 100%);
  padding: 4px 7px;
  border: 1px solid transparent;
  border-radius: 6px;
  background: transparent;
  color: var(--laipe-text-muted, #6e6e73);
  font-size: 12px;
  font-family: inherit;
  cursor: pointer;
  transition: color 0.15s ease, border-color 0.15s ease, background 0.15s ease;
  white-space: nowrap;
}
.model-effort-trigger:hover:not(.disabled) {
  color: var(--laipe-text, #1d1d1f);
  background: var(--laipe-bg, #f5f5f7);
}
.model-effort-trigger.open {
  color: var(--laipe-text, #1d1d1f);
  background: var(--laipe-bg, #f5f5f7);
}
.model-effort-trigger.disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.model-effort-trigger.empty .model-effort-model {
  color: var(--laipe-text-muted, #6e6e73);
  font-style: italic;
}

.model-effort-model {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}
.model-effort-level {
  flex-shrink: 0;
  font-weight: 600;
  letter-spacing: 0.2px;
}
.model-effort-chevron {
  flex-shrink: 0;
  font-size: 10px;
  transition: transform 0.15s ease;
}
.model-effort-trigger.open .model-effort-chevron {
  transform: rotate(180deg);
}

.model-effort-dropdown {
  position: absolute;
  bottom: calc(100% + 6px);
  left: 0;
  right: auto;
  min-width: 220px;
  max-width: min(420px, calc(100vw - 24px));
  max-height: min(420px, calc(100vh - 160px));
  overflow: hidden;
  padding: 4px;
  border: 1px solid var(--laipe-border, #d2d2d7);
  border-radius: 10px;
  background: var(--laipe-bg, #ffffff);
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.12);
  z-index: 50;
  transform-origin: bottom left;
}
.model-effort-dropdown.placement-bottom {
  bottom: auto;
  top: calc(100% + 6px);
  transform-origin: top left;
}
.model-effort-dropdown.align-end {
  left: auto;
  right: 0;
  transform-origin: bottom right;
}
.model-effort-dropdown.placement-bottom.align-end {
  transform-origin: top right;
}
.model-effort-dropdown.has-effort {
  width: min(420px, calc(100vw - 24px));
  display: grid;
  grid-template-columns: minmax(0, 1fr) 96px;
}

.model-effort-model-panel,
.model-effort-effort-panel {
  min-width: 0;
  max-height: min(404px, calc(100vh - 176px));
  overflow-y: auto;
}
.model-effort-effort-panel {
  border-left: 1px solid var(--laipe-border, #e5e5e7);
  padding-left: 4px;
}

.model-effort-section-label {
  padding: 6px 12px 4px;
  font-size: 11px;
  font-weight: 500;
  letter-spacing: 0.2px;
  color: var(--laipe-text-muted, #6e6e73);
}
.model-effort-section-label.uppercase {
  text-transform: uppercase;
  letter-spacing: 0.5px;
  color: var(--laipe-text, #1d1d1f);
  font-weight: 600;
}
.model-effort-divider {
  height: 1px;
  margin: 4px 8px;
  background: var(--laipe-border, #d2d2d7);
}
.model-effort-empty {
  padding: 12px;
  font-size: 12px;
  color: var(--laipe-text-muted, #6e6e73);
  text-align: center;
}
.model-effort-empty p {
  margin: 0;
}
.model-effort-empty-hint {
  margin-top: 4px;
  font-size: 11px;
  opacity: 0.8;
}

.model-effort-option {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 12px;
  border: none;
  border-radius: 8px;
  background: transparent;
  color: inherit;
  font-family: inherit;
  text-align: left;
  cursor: pointer;
  transition: background 0.12s ease;
}
.model-effort-option:hover {
  background: var(--laipe-bg, #f5f5f7);
}
.model-effort-option.active {
  background: rgba(0, 122, 255, 0.08);
}
.model-effort-option-name {
  flex: 1;
  min-width: 0;
  color: var(--laipe-text, #1d1d1f);
  font-size: 13px;
  font-weight: 500;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.model-effort-option-tag {
  flex-shrink: 0;
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.2px;
}

.dropdown-enter-active,
.dropdown-leave-active {
  transition: opacity 0.12s ease, transform 0.12s ease;
}
.dropdown-enter-from,
.dropdown-leave-to {
  opacity: 0;
  transform: translateY(4px);
}
</style>
