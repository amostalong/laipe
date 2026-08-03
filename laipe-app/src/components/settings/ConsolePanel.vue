<script setup lang="ts">
// ConsolePanel — debug console settings page panel.
//
// v0.2+ extracted from the Settings modal's #extra slot. Renders the
// laipe-vue `ConsolePanel` (read-only log viewer) plus a small
// in-panel toggle group for console-level filtering defaults
// (these persist via the same `useConfig().config` storage as the
// provider config).
//
// v0.2+ deep-link: when `/settings?tab=console&runId=<id>` is opened
// (e.g. from a chat error's "查看详情"), we forward `runIdFilter` to
// the underlying console. The console's search box is set to the
// runId so the user immediately sees entries from that run.

import { ConsolePanel as LaipeConsolePanel } from "laipe-vue";

defineOptions({ name: "ConsolePanel" });

defineProps<{
  /** Optional runId to filter the console to. Set by route query. */
  runIdFilter?: string | null;
}>();
</script>

<template>
  <section class="console-panel-page">
    <h2>Console</h2>
    <p class="hint">
      Live runtime logs (info / warn / error, backend / frontend). In-memory
      only, max 1000 entries, clears on restart. Click an error row's
      <strong>save</strong> to write a self-contained <code>.md</code> report
      for an LLM assistant.
    </p>
    <div v-if="runIdFilter" class="filter-banner">
      Filtering to runId: <code>{{ runIdFilter }}</code>
    </div>
    <LaipeConsolePanel />
  </section>
</template>

<style scoped>
.console-panel-page {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 8px 0;
  height: 100%;
  min-height: 0;
}
.console-panel-page h2 {
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
.hint code {
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  font-size: 0.92em;
  background: var(--laipe-bg-elevated, #f5f5f7);
  padding: 0 3px;
  border-radius: 2px;
}
.filter-banner {
  padding: 6px 10px;
  background: rgba(0, 122, 255, 0.08);
  border: 1px solid rgba(0, 122, 255, 0.4);
  color: var(--laipe-accent, #007aff);
  font-size: 0.85em;
  border-radius: 4px;
}
.filter-banner code {
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  background: rgba(0, 122, 255, 0.12);
  padding: 0 4px;
  border-radius: 2px;
}
</style>
