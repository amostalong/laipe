<script setup lang="ts">
// ToolsPanel — per-tool enable/permission UI (formerly the #extra
// slot of the Settings modal). v0.2+ extracted into its own page
// panel.
//
// Renders one row per ToolDefinition with:
//   - an on/off toggle (disabled tools are NOT sent to the LLM)
//   - a permission dropdown (only meaningful when enabled):
//       auto — run immediately, no UI
//       ask  — show Approve/Deny card before the backend runs the tool
//       deny — refuse the call; tell the LLM the user rejected it
//
// v0.2+ auto-save: emits `update:enabledTools` and
// `update:toolPermissions` immediately on change. The parent's
// v-model writes through to the storage layer.

import type { ToolDefinition, ToolPermission } from "laipe-ts";
import ToolsSettings from "../ToolsSettings.vue";

defineOptions({ name: "ToolsPanel" });

const props = defineProps<{
  tools: ToolDefinition[];
  enabledTools: Record<string, boolean>;
  toolPermissions: Record<string, ToolPermission>;
}>();

const emit = defineEmits<{
  "update:enabledTools": [next: Record<string, boolean>];
  "update:toolPermissions": [next: Record<string, ToolPermission>];
}>();

function onEnabledToolsChange(next: Record<string, boolean>): void {
  emit("update:enabledTools", next);
}

function onToolPermissionsChange(next: Record<string, ToolPermission>): void {
  emit("update:toolPermissions", next);
}
</script>

<template>
  <section class="tools-panel">
    <h2>AI Tools</h2>
    <p class="hint">
      Control which tools the LLM may call and how each call is gated.
      Disabled tools are not sent in the request body — the model has
      zero knowledge they exist. For enabled tools, pick a permission
      level:
      <strong>auto</strong> runs the tool without asking,
      <strong>ask</strong> shows an Approve/Deny card before each
      call, and
      <strong>deny</strong> refuses the call and tells the LLM the
      user rejected it.
    </p>
    <ToolsSettings
      :tools="tools"
      :enabled-tools="enabledTools"
      :tool-permissions="toolPermissions"
      @update:enabled-tools="onEnabledToolsChange"
      @update:tool-permissions="onToolPermissionsChange"
    />
  </section>
</template>

<style scoped>
.tools-panel {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 8px 0;
}
.tools-panel h2 {
  margin: 0 0 4px 0;
  font-size: 1.4em;
  font-weight: 600;
  color: var(--laipe-text, #1d1d1f);
}
.hint {
  margin: 0;
  font-size: 0.85em;
  color: var(--laipe-text-muted, #6e6e73);
  line-height: 1.5;
}
.hint strong {
  color: var(--laipe-text, #1d1d1f);
  font-weight: 600;
}
</style>
