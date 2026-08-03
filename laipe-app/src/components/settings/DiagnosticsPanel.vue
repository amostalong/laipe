<script setup lang="ts">
// DiagnosticsPanel — LLM-debuggable diagnostics settings panel.
//
// v0.2+ extracted from the Settings modal's #extra slot. Renders
// the auto-snapshot / record-all toggles + max-bytes cap. The
// underlying state is the Rust `DiagnosticConfig` in
// `laipe-app/src-tauri/src/diagnostics.rs`, accessed via
// `getDiagnosticConfig()` / `setDiagnosticConfig()` (laipe-vue).
//
// v0.2+ auto-save: every change writes through immediately. No Save
// button — matches PlotCraft v0.1.5+.

import DiagnosticsSettings from "../DiagnosticsSettings.vue";

defineOptions({ name: "DiagnosticsPanel" });
</script>

<template>
  <section class="diagnostics-panel-page">
    <h2>Diagnostics</h2>
    <p class="hint">
      On-disk error reports — written when the LLM fails. Each report
      is a self-contained <code>.md</code> file you can hand to an
      LLM assistant. See <code>.agents/docs/DIAGNOSTICS.md</code> for
      the full design and <code>&lt;app_log_dir&gt;/README-FOR-LLM.md</code>
      for the per-error-class debug recipes.
    </p>
    <DiagnosticsSettings />
  </section>
</template>

<style scoped>
.diagnostics-panel-page {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 8px 0;
}
.diagnostics-panel-page h2 {
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
</style>
