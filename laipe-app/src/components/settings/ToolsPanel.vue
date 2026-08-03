<script setup lang="ts">
// ToolsPanel — per-tool enable/permission UI (formerly the #extra
// slot of the Settings modal). v0.2+ extracted into its own page
// panel.
//
// Renders one row per ToolDefinition with an on/off toggle. Disabled
// tools are NOT sent to the LLM (filtered out in MainView before
// useChat is called). Permission (auto / ask / deny) is reserved for
// v0.2+ — the starter only exposes the enable toggle.
//
// v0.2+ auto-save: emits `update:enabledTools` immediately on
// toggle. The parent's v-model writes through to the storage layer.

import type { ToolDefinition } from "laipe-ts";
import ToolsSettings from "../ToolsSettings.vue";

defineOptions({ name: "ToolsPanel" });

const props = defineProps<{
  tools: ToolDefinition[];
  enabledTools: Record<string, boolean>;
}>();

const emit = defineEmits<{
  "update:enabledTools": [next: Record<string, boolean>];
}>();

function onEnabledToolsChange(next: Record<string, boolean>): void {
  emit("update:enabledTools", next);
}
</script>

<template>
  <section class="tools-panel">
    <h2>AI Tools</h2>
    <p class="hint">
      Control which tools the LLM may call. Disabled tools are not
      sent in the request body — the model has zero knowledge they
      exist. v0.2+ also adds per-tool permissions (auto / ask / deny).
    </p>
    <ToolsSettings
      :tools="tools"
      :enabled-tools="enabledTools"
      @update:enabled-tools="onEnabledToolsChange"
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
</style>
