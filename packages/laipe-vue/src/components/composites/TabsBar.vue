<script setup lang="ts">
// TabsBar — base tab container UI for laipe-vue.
//
// Renders a horizontal bar of tabs. The host wires it like a
// v-model control (`:model-value` + `@update:model-value`); the bar
// itself is view-agnostic. When a tab is clicked, the host
// typically navigates to the corresponding route (or shows the
// corresponding view by some other mechanism).
//
// v0.2+ laipe-app: uses this with 2 pinned tabs (Chat, Settings)
// that map 1:1 to routes. The host derives the active tab from
// `route.name` so the URL stays the source of truth.
//
// Visual shape
// ============
//
//     [icon] Title    [icon] Title    [icon] Title    [+]  ← (no add in v0.1)
//              ▲
//              active
//
// - Active tab:  bg = bg, border-bottom = accent (2px), text = text
// - Hover (inactive): bg = border (subtle), text = text
// - Pinned tabs: no × button
// - Closeable tabs: × button on hover
// - Badge: small pill in the right of the tab
//
// Why inline SVG, not a library
// =============================
//
// We avoid pulling in lucide-vue-next (1+ MB) just for the 2-3 icons
// a starter app needs. Hosts can pass `iconPath` (Lucide path data
// is MIT-licensed and easy to inline) and the bar handles the
// <svg> wrapper.

import type { Tab } from "../../lib/tabs";

defineOptions({ name: "TabsBar" });

const props = withDefaults(
  defineProps<{
    /** Tab definitions. Order is preserved (left to right). */
    tabs: Tab[];
    /** Currently active tab id. v-model friendly. */
    modelValue: string | null;
    /** Visual density. "compact" (default) = 32px tall; "comfortable" = 40px. */
    density?: "compact" | "comfortable";
  }>(),
  { density: "compact" },
);

const emit = defineEmits<{
  "update:modelValue": [id: string];
  /** Emitted when the user clicks a tab's close (×) button. The
   * host decides whether to actually close it (e.g. confirm
   * unsaved changes). */
  close: [id: string];
}>();

function activate(id: string): void {
  if (id !== props.modelValue) {
    emit("update:modelValue", id);
  }
}

function onClose(e: MouseEvent, id: string): void {
  e.stopPropagation();
  emit("close", id);
}

function isCloseable(tab: Tab): boolean {
  // Default: pinned = not closeable, not pinned = closeable.
  if (tab.closeable !== undefined) return tab.closeable;
  return !tab.pinned;
}
</script>

<template>
  <div
    class="tabs-bar"
    :class="[`density-${density}`]"
    role="tablist"
    aria-label="Open tabs"
  >
    <button
      v-for="tab in tabs"
      :key="tab.id"
      type="button"
      role="tab"
      :aria-selected="tab.id === modelValue"
      :tabindex="tab.id === modelValue ? 0 : -1"
      class="tab"
      :class="{ active: tab.id === modelValue }"
      :title="tab.tooltip ?? tab.title"
      @click="activate(tab.id)"
    >
      <svg
        v-if="tab.iconPath"
        class="tab-icon"
        viewBox="0 0 24 24"
        width="14"
        height="14"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
      >
        <path :d="tab.iconPath" />
      </svg>
      <span class="tab-title">{{ tab.title }}</span>
      <span v-if="tab.badge" class="tab-badge">{{ tab.badge }}</span>
      <button
        v-if="isCloseable(tab)"
        type="button"
        class="tab-close"
        :aria-label="`Close ${tab.title}`"
        @click="(e) => onClose(e, tab.id)"
      >
        <svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d="M18 6L6 18M6 6l12 12" />
        </svg>
      </button>
    </button>
  </div>
</template>

<style scoped>
.tabs-bar {
  display: flex;
  align-items: stretch;
  background: var(--laipe-bg-elevated, #fafafa);
  border-bottom: 1px solid var(--laipe-border, #e5e5e7);
  overflow-x: auto;
  overflow-y: hidden;
  scrollbar-width: thin;
}
.density-compact { min-height: 36px; }
.density-comfortable { min-height: 44px; }

.tab {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 0 12px;
  background: transparent;
  color: var(--laipe-text-muted, #6e6e73);
  border: none;
  border-bottom: 2px solid transparent;
  border-right: 1px solid var(--laipe-border, #e5e5e7);
  cursor: pointer;
  font-size: 0.85em;
  font-family: inherit;
  white-space: nowrap;
  position: relative;
  transition: background 0.12s ease, color 0.12s ease;
}
.density-comfortable .tab { padding: 0 16px; }
.tab:hover {
  background: var(--laipe-border, #e5e5e7);
  color: var(--laipe-text, #1d1d1f);
}
.tab.active {
  background: var(--laipe-bg, #ffffff);
  color: var(--laipe-accent, #007aff);
  border-bottom-color: var(--laipe-accent, #007aff);
  font-weight: 500;
}
.tab-icon {
  flex-shrink: 0;
  color: currentColor;
}
.tab-title {
  white-space: nowrap;
}
.tab-badge {
  display: inline-block;
  min-width: 18px;
  padding: 0 5px;
  height: 16px;
  line-height: 16px;
  background: var(--laipe-accent, #007aff);
  color: white;
  border-radius: 8px;
  font-size: 0.85em;
  font-weight: 600;
  text-align: center;
}
.tab-close {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 18px;
  height: 18px;
  margin-left: 2px;
  background: transparent;
  border: none;
  border-radius: 3px;
  color: var(--laipe-text-muted, #a1a1a6);
  cursor: pointer;
  opacity: 0;
  transition: opacity 0.12s ease, background 0.12s ease;
}
.tab:hover .tab-close,
.tab.active .tab-close {
  opacity: 0.7;
}
.tab-close:hover {
  opacity: 1 !important;
  background: rgba(0, 0, 0, 0.1);
  color: var(--laipe-text, #1d1d1f);
}
</style>
