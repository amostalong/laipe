<script setup lang="ts">
// ToolsSettings — per-tool enable/permission UI.
//
// Renders one row per ToolDefinition with:
//   - an on/off toggle (disabled tools are NOT sent to the LLM)
//   - a permission dropdown (only meaningful when enabled):
//       auto — run immediately, no UI
//       ask  — show Approve/Deny card before the backend runs the tool
//       deny — refuse the call; tell the LLM the user rejected it
//
// The permission dropdown is disabled when the tool is off (no point
// gating a tool the LLM can't see). When all tools are off, the
// permission selects are hidden but the on/off row stays so the user
// can re-enable any of them.
//
// v0.2+ auto-save: emits `update:enabledTools` and
// `update:toolPermissions` immediately on change. The parent's
// v-model writes through to the storage layer.

import type { ToolDefinition, ToolPermission } from "laipe-ts";

defineOptions({ name: "ToolsSettings" });

const props = defineProps<{
  /** All known tools (from `src/tools.ts`). */
  tools: ToolDefinition[];
  /** Current per-tool enabled state. */
  enabledTools: Record<string, boolean>;
  /** Current per-tool permission. Missing entries default to `"auto"`. */
  toolPermissions: Record<string, ToolPermission>;
}>();

const emit = defineEmits<{
  "update:enabledTools": [next: Record<string, boolean>];
  "update:toolPermissions": [next: Record<string, ToolPermission>];
}>();

function toggle(name: string, enabled: boolean) {
  emit("update:enabledTools", { ...props.enabledTools, [name]: enabled });
}

function setPermission(name: string, perm: ToolPermission) {
  emit("update:toolPermissions", {
    ...props.toolPermissions,
    [name]: perm,
  });
}

function isEnabled(name: string): boolean {
  // Default to true if not yet recorded (matches "tools default to on" UX).
  return props.enabledTools[name] ?? true;
}

function getPermission(name: string): ToolPermission {
  return props.toolPermissions[name] ?? "auto";
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

function resetPermissions() {
  // Wipe to defaults — every tool becomes "auto".
  emit("update:toolPermissions", {});
}
</script>

<template>
  <section class="tools-settings">
    <header class="section-header">
      <h3>AI Tools</h3>
      <p class="section-desc">
        Control which tools the LLM may call and whether to gate each
        one behind user approval. Disabled tools are NOT sent to the
        LLM in the request body — it has zero knowledge they exist.
      </p>
    </header>

    <div class="bulk-actions">
      <button type="button" class="link-btn" @click="enableAll">Enable all</button>
      <span class="dot">·</span>
      <button type="button" class="link-btn" @click="disableAll">Disable all</button>
      <span class="dot">·</span>
      <button
        type="button"
        class="link-btn"
        title="Reset every tool's permission to auto"
        @click="resetPermissions"
      >
        Reset permissions
      </button>
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
          <div v-if="isEnabled(t.function.name)" class="permission-row">
            <label class="permission-label" :for="`perm-${t.function.name}`">
              Permission
            </label>
            <select
              :id="`perm-${t.function.name}`"
              class="permission-select"
              :value="getPermission(t.function.name)"
              @change="
                (e) => setPermission(
                  t.function.name,
                  (e.target as HTMLSelectElement).value as ToolPermission,
                )
              "
            >
              <option value="auto">auto — run immediately</option>
              <option value="ask">ask — confirm before running</option>
              <option value="deny">deny — refuse, tell the LLM</option>
            </select>
          </div>
        </div>
        <label class="toggle" :title="`${t.function.name} ${isEnabled(t.function.name) ? 'enabled' : 'disabled'}`">
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
  flex-wrap: wrap;
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
  align-items: flex-start;
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
  gap: 4px;
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
.permission-row {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 4px;
  font-size: 0.78em;
}
.permission-label {
  color: var(--laipe-text-muted, #6e6e73);
  text-transform: uppercase;
  letter-spacing: 0.4px;
  font-size: 0.92em;
}
.permission-select {
  flex: 1 1 auto;
  min-width: 0;
  max-width: 240px;
  padding: 3px 6px;
  border: 1px solid var(--laipe-border, #d2d2d7);
  border-radius: 4px;
  background: white;
  color: var(--laipe-text, #1d1d1f);
  font-family: inherit;
  font-size: inherit;
  cursor: pointer;
}
.permission-select:focus {
  outline: none;
  border-color: var(--laipe-accent, #007aff);
  box-shadow: 0 0 0 2px rgba(0, 122, 255, 0.2);
}
.toggle {
  position: relative;
  display: inline-block;
  width: 36px;
  height: 20px;
  flex-shrink: 0;
  cursor: pointer;
  margin-top: 2px;
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
