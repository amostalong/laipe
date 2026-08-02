<script setup lang="ts">
// ToolsSettings — per-tool enable/permission UI for the Settings modal.
//
// Renders one row per ToolDefinition with an on/off toggle. Disabled
// tools are NOT sent to the LLM (filtered out in App.vue before
// useChat is called). Permission (auto / ask / deny) is reserved for
// v0.2+ — the starter only exposes the enable toggle.

import type { ToolDefinition } from "laipe-ts";

defineOptions({ name: "ToolsSettings" });

const props = defineProps<{
  /** All known tools (from `src/tools.ts`). */
  tools: ToolDefinition[];
  /** Current per-tool enabled state. */
  enabledTools: Record<string, boolean>;
}>();

const emit = defineEmits<{
  "update:enabledTools": [next: Record<string, boolean>];
}>();

function toggle(name: string, enabled: boolean) {
  emit("update:enabledTools", { ...props.enabledTools, [name]: enabled });
}

function isEnabled(name: string): boolean {
  // Default to true if not yet recorded (matches "tools default to on" UX).
  return props.enabledTools[name] ?? true;
}

function enableAll() {
  const next: Record<string, boolean> = {};
  for (const t of props.tools) next[t.function.name] = true;
  emit("update:enabledTools", next);
}

function disableAll() {
  const next: Record<string, boolean> = {};
  for (const t of props.tools) next[t.function.name] = false;
  emit("update:enabledTools", next);
}
</script>

<template>
  <section class="tools-settings">
    <header class="section-header">
      <h3>AI Tools</h3>
      <p class="section-desc">
        Control which tools the LLM may call. Disabled tools are NOT sent
        to the LLM in the request body — it has zero knowledge they exist.
      </p>
    </header>

    <div class="bulk-actions">
      <button type="button" class="link-btn" @click="enableAll">Enable all</button>
      <span class="dot">·</span>
      <button type="button" class="link-btn" @click="disableAll">Disable all</button>
    </div>

    <div class="tool-list">
      <div
        v-for="t in tools"
        :key="t.function.name"
        class="tool-item"
        :class="{ disabled: !isEnabled(t.function.name) }"
      >
        <div class="tool-info">
          <div class="tool-name">{{ t.function.name }}</div>
          <div class="tool-desc">{{ t.function.description }}</div>
        </div>
        <label class="toggle">
          <input
            type="checkbox"
            :checked="isEnabled(t.function.name)"
            @change="(e) => toggle(t.function.name, (e.target as HTMLInputElement).checked)"
          />
          <span class="slider"></span>
        </label>
      </div>
    </div>
  </section>
</template>

<style scoped>
.tools-settings {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.section-header h3 {
  margin: 0 0 4px 0;
  font-size: 1em;
  font-weight: 600;
  color: var(--laipe-text, #1d1d1f);
}
.section-desc {
  margin: 0;
  font-size: 0.78em;
  color: var(--laipe-text-muted, #6e6e73);
  line-height: 1.5;
}
.bulk-actions {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 0.78em;
}
.link-btn {
  background: transparent;
  border: none;
  color: var(--laipe-accent, #007aff);
  cursor: pointer;
  font-size: inherit;
  font-family: inherit;
  padding: 0;
}
.link-btn:hover {
  text-decoration: underline;
}
.dot {
  color: var(--laipe-text-muted, #a1a1a6);
}
.tool-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.tool-item {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 12px;
  background: var(--laipe-bg-elevated, #fafafa);
  border: 1px solid var(--laipe-border, #e5e5e7);
  border-radius: 6px;
  transition: opacity 0.12s ease, border-style 0.12s ease;
}
.tool-item.disabled {
  opacity: 0.55;
  border-style: dashed;
}
.tool-info {
  flex: 1 1 auto;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.tool-name {
  font-size: 0.85em;
  font-weight: 600;
  color: var(--laipe-text, #1d1d1f);
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
}
.tool-desc {
  font-size: 0.78em;
  color: var(--laipe-text-muted, #6e6e73);
  line-height: 1.4;
}
.toggle {
  position: relative;
  display: inline-block;
  width: 36px;
  height: 20px;
  flex-shrink: 0;
  cursor: pointer;
}
.toggle input {
  opacity: 0;
  width: 0;
  height: 0;
}
.slider {
  position: absolute;
  inset: 0;
  background: var(--laipe-border-strong, #d2d2d7);
  border-radius: 10px;
  transition: background 0.15s ease;
}
.slider::before {
  content: "";
  position: absolute;
  top: 2px;
  left: 2px;
  width: 16px;
  height: 16px;
  background: white;
  border-radius: 50%;
  transition: transform 0.15s ease;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.2);
}
.toggle input:checked + .slider {
  background: var(--laipe-accent, #007aff);
}
.toggle input:checked + .slider::before {
  transform: translateX(16px);
}
</style>
