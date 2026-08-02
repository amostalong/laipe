<script setup lang="ts">
// DiagnosticsSettings — Settings-modal section for the diagnostic toggles.
//
// Mirrors the Rust `DiagnosticConfig` in
// `laipe-app/src-tauri/src/diagnostics.rs`. Loads via
// `getDiagnosticConfig()` on mount; writes via `setDiagnosticConfig()`
// on every change (debounced for the byte cap).
//
// The toggles are conservative by default — the user has to opt in
// to on-disk recording. The Settings-modal text is honest about
// what gets written where.

import { onMounted, ref, watch } from "vue";
import { getDiagnosticConfig, setDiagnosticConfig, type DiagnosticConfig } from "laipe-vue";

defineOptions({ name: "DiagnosticsSettings" });

const cfg = ref<DiagnosticConfig>({
  auto_snapshot: false,
  max_report_bytes: 5 * 1024 * 1024,
  record_successful_rounds: false,
});
const loaded = ref(false);
const saveError = ref<string | null>(null);

onMounted(async () => {
  try {
    const remote = await getDiagnosticConfig();
    cfg.value = remote;
  } catch (e) {
    // No Tauri runtime (browser-only) or backend doesn't expose the
    // command. The toggles still render but writes no-op.
    saveError.value = `could not load diagnostic config: ${(e as Error).message ?? e}`;
  } finally {
    loaded.value = true;
  }
});

async function persist() {
  saveError.value = null;
  try {
    await setDiagnosticConfig(cfg.value);
  } catch (e) {
    saveError.value = `could not save: ${(e as Error).message ?? e}`;
  }
}

// Debounce the byte cap input (typing triggers many writes); the
// toggles are written immediately because they're low-frequency.
let capTimer: ReturnType<typeof setTimeout> | null = null;
watch(
  () => cfg.value.max_report_bytes,
  () => {
    if (capTimer) clearTimeout(capTimer);
    capTimer = setTimeout(persist, 500);
  },
);
watch(
  () => [cfg.value.auto_snapshot, cfg.value.record_successful_rounds],
  () => {
    persist();
  },
);
</script>

<template>
  <details class="diag-block" :open="!loaded">
    <summary>
      Diagnostics
      <span class="diag-hint">· on-disk error reports (LLM-debuggable)</span>
    </summary>

    <div v-if="!loaded" class="diag-loading">loading…</div>
    <div v-else class="diag-body">
      <p class="diag-desc">
        When something goes wrong (auth fail, rate limit, network drop, SSE
        protocol error), laipe writes a self-contained <code>.md</code> report
        to the app's log directory. Paste the file into an LLM assistant —
        it has the full request, raw response, your conversation up to the
        failure, and a per-error-class debug recipe.
      </p>

      <label class="diag-row">
        <input v-model="cfg.auto_snapshot" type="checkbox" />
        <span class="diag-label">Auto-snapshot every error to a .md report</span>
        <span class="diag-sub">
          Writes a report file on every failed chat turn. Off by default —
          the user has to opt in.
        </span>
      </label>

      <label class="diag-row">
        <input v-model="cfg.record_successful_rounds" type="checkbox" />
        <span class="diag-label">Record every chat round (success + failure)</span>
        <span class="diag-sub">
          Saves request + response to disk for every turn. Useful when
          reproducing a flaky-but-not-fatal stream bug; large disk usage.
        </span>
      </label>

      <div class="diag-row">
        <label class="diag-cap-label" for="diag-cap">Max bytes per report</label>
        <input
          id="diag-cap"
          v-model.number="cfg.max_report_bytes"
          type="number"
          min="65536"
          step="65536"
          class="diag-cap-input"
        />
        <span class="diag-sub">
          Response bytes beyond this are truncated. Default 5 MiB. Min 64 KiB.
        </span>
      </div>

      <p v-if="saveError" class="diag-err">⚠ {{ saveError }}</p>
    </div>
  </details>
</template>

<style scoped>
.diag-block {
  border-top: 1px solid var(--laipe-border, #e5e5e7);
  padding-top: 12px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.diag-block summary {
  cursor: pointer;
  font-size: 0.85em;
  font-weight: 500;
  color: var(--laipe-text-secondary, #6e6e73);
  user-select: none;
  list-style: none;
}
.diag-block summary::-webkit-details-marker {
  display: none;
}
.diag-block summary::before {
  content: "▸";
  display: inline-block;
  margin-right: 6px;
  transition: transform 0.15s ease;
}
.diag-block[open] summary::before {
  transform: rotate(90deg);
}
.diag-hint {
  font-weight: 400;
  color: var(--laipe-text-muted, #a1a1a6);
  font-size: 0.9em;
  margin-left: 4px;
}
.diag-loading {
  font-size: 0.78em;
  color: var(--laipe-text-muted, #a1a1a6);
  padding: 8px 0;
}
.diag-body {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.diag-desc {
  margin: 0;
  font-size: 0.78em;
  color: var(--laipe-text-muted, #6e6e73);
  line-height: 1.5;
}
.diag-desc code {
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  font-size: 0.95em;
  background: var(--laipe-bg-elevated, #f5f5f7);
  padding: 0 3px;
  border-radius: 2px;
}
.diag-row {
  display: flex;
  flex-direction: column;
  gap: 2px;
  font-size: 0.78em;
  cursor: pointer;
}
.diag-row input[type="checkbox"] {
  margin-right: 6px;
  vertical-align: middle;
}
.diag-label {
  color: var(--laipe-text, #1d1d1f);
  font-weight: 500;
}
.diag-sub {
  color: var(--laipe-text-muted, #a1a1a6);
  font-size: 0.92em;
  line-height: 1.4;
  margin-left: 22px;
}
.diag-cap-label {
  color: var(--laipe-text, #1d1d1f);
  font-weight: 500;
  display: block;
  margin-bottom: 4px;
}
.diag-cap-input {
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  font-size: 0.9em;
  padding: 4px 8px;
  border: 1px solid var(--laipe-border, #e5e5e7);
  border-radius: 4px;
  background: var(--laipe-bg, #ffffff);
  color: var(--laipe-text, #1d1d1f);
  width: 160px;
  margin-bottom: 4px;
}
.diag-cap-input:focus {
  outline: none;
  border-color: var(--laipe-accent, #007aff);
}
.diag-err {
  margin: 0;
  font-size: 0.78em;
  color: #ff3b30;
  padding: 6px 8px;
  background: rgba(255, 59, 48, 0.08);
  border-radius: 4px;
}
</style>
