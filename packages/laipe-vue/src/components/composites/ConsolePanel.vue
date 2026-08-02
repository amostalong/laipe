<script setup lang="ts">
// ConsolePanel — in-app debug log viewer.
//
// Reads from the shared `useConsoleEntries()` ref (singleton in console.ts).
// Filter by level (info / warn / error) + source (backend / frontend) +
// free-text search across module + message. Refresh reloads the Rust
// snapshot; Clear wipes both frontend + backend buffers.
//
// ## Save report (v0.2+)
//
// For backend `error` entries that have a `kind` (i.e. real LLM
// errors emitted by the chat command), the row shows a small "save"
// action that calls the Rust `dump_error_report` command. The result
// is a self-contained `.md` file in the app's log dir — paste the
// file into an LLM assistant to debug. The path is shown as a
// transient banner above the list.

import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import {
  clearConsole,
  initConsole,
  refreshConsole,
  saveReport,
  useConsoleEntries,
  type ConsoleEntry,
  type ConsoleLevel,
  type ConsoleSource,
} from "../../console";

defineOptions({ name: "ConsolePanel" });

const entries = useConsoleEntries();

// === Filters ===
const levelFilter = ref<"all" | ConsoleLevel>("all");
const sourceFilter = ref<"all" | ConsoleSource>("all");
const searchQuery = ref("");
const autoScroll = ref(true);

const listEl = ref<HTMLElement | null>(null);
const copiedId = ref<string | null>(null);
let copiedTimer: ReturnType<typeof setTimeout> | null = null;

// === Last saved report banner ===
const lastSavedReport = ref<string | null>(null);
let lastSavedTimer: ReturnType<typeof setTimeout> | null = null;

const filtered = computed(() => {
  const q = searchQuery.value.trim().toLowerCase();
  return entries.value.filter((e) => {
    if (levelFilter.value !== "all" && e.level !== levelFilter.value) return false;
    if (sourceFilter.value !== "all" && e.source !== sourceFilter.value) return false;
    if (q) {
      const hay = `${e.module} ${e.message} ${e.level} ${e.source}`.toLowerCase();
      if (!hay.includes(q)) return false;
    }
    return true;
  });
});

const countLabel = computed(() => `${filtered.value.length} / ${entries.value.length} 条`);

/** True iff the entry has enough context for a saved report. */
function canSaveReport(entry: ConsoleEntry): boolean {
  return entry.level === "error" && entry.source === "backend" && !!entry.kind;
}

function formatTime(ts: number): string {
  const d = new Date(ts);
  const h = String(d.getHours()).padStart(2, "0");
  const m = String(d.getMinutes()).padStart(2, "0");
  const s = String(d.getSeconds()).padStart(2, "0");
  return `${h}:${m}:${s}`;
}

async function onRefresh() {
  await refreshConsole();
}

async function onClear() {
  await clearConsole();
}

async function copyEntry(entry: ConsoleEntry) {
  const diag = [
    entry.conversation_id ? `conv=${entry.conversation_id}` : null,
    entry.turn !== undefined ? `turn=${entry.turn}` : null,
    entry.kind ? `kind=${entry.kind}` : null,
  ]
    .filter(Boolean)
    .join(" ");
  const header = `[${formatTime(entry.timestamp_ms)}] [${entry.source}] [${entry.module}] [${entry.level.toUpperCase()}]`;
  const diagSuffix = diag ? ` (${diag})` : "";
  const text = `${header}${diagSuffix} ${entry.message}`;
  try {
    await navigator.clipboard.writeText(text);
    copiedId.value = entry.id;
    if (copiedTimer) clearTimeout(copiedTimer);
    copiedTimer = setTimeout(() => {
      copiedId.value = null;
      copiedTimer = null;
    }, 1200);
  } catch (e) {
    console.error("[console] clipboard write failed:", e);
  }
}

async function onSaveReport(entry: ConsoleEntry, ev: Event) {
  // Don't trigger the row's click-to-copy.
  ev.stopPropagation();
  try {
    const { path } = await saveReport(entry.id);
    lastSavedReport.value = path;
    if (lastSavedTimer) clearTimeout(lastSavedTimer);
    lastSavedTimer = setTimeout(() => {
      lastSavedReport.value = null;
      lastSavedTimer = null;
    }, 5000);
  } catch (e) {
    // Surface the failure in the console itself (frontend) — the user
    // can copy it from there.
    console.error(`[console] save report failed: ${(e as Error).message ?? e}`);
  }
}

// Auto-scroll: new entry arrives → scroll to top (newest at [0]).
watch(
  () => [filtered.value[0]?.id ?? "", filtered.value.length] as const,
  async () => {
    if (!autoScroll.value) return;
    await nextTick();
    if (listEl.value) listEl.value.scrollTop = 0;
  },
);

onMounted(async () => {
  // Defensive: if main.ts init didn't run, do it here.
  await initConsole();
});

onUnmounted(() => {
  if (copiedTimer) {
    clearTimeout(copiedTimer);
    copiedTimer = null;
  }
  if (lastSavedTimer) {
    clearTimeout(lastSavedTimer);
    lastSavedTimer = null;
  }
});
</script>

<template>
  <div class="console-panel">
    <header class="header">
      <h3>Debug Console</h3>
      <p class="desc">
        App runtime logs (info / warn / error, backend / frontend). Rust key
        errors and frontend <code>console.log</code> are auto-captured. In-memory
        only, max 1000 entries, clears on restart. Click an error row's
        <strong>save</strong> to write a self-contained <code>.md</code> report
        for an LLM assistant.
      </p>
    </header>

    <div v-if="lastSavedReport" class="saved-banner">
      ✓ Report saved: <code>{{ lastSavedReport }}</code>
    </div>

    <div class="toolbar">
      <div class="filter-group" role="tablist" aria-label="level filter">
        <button
          v-for="lv in (['all', 'info', 'warn', 'error'] as const)"
          :key="lv"
          type="button"
          class="filter-chip"
          :class="{ active: levelFilter === lv }"
          @click="levelFilter = lv"
        >
          {{ lv }}
        </button>
      </div>

      <div class="filter-group" role="tablist" aria-label="source filter">
        <button
          v-for="src in (['all', 'backend', 'frontend'] as const)"
          :key="src"
          type="button"
          class="filter-chip"
          :class="{ active: sourceFilter === src }"
          @click="sourceFilter = src"
        >
          {{ src }}
        </button>
      </div>

      <label class="autoscroll-toggle">
        <input v-model="autoScroll" type="checkbox" />
        <span>auto-scroll</span>
      </label>

      <div class="search-wrap">
        <input
          v-model="searchQuery"
          type="text"
          placeholder="search module / message…"
          class="search-input"
        />
      </div>

      <button type="button" class="action-btn" @click="onRefresh" title="Reload snapshot from backend">
        ↻ Refresh
      </button>
      <button
        type="button"
        class="action-btn"
        :disabled="entries.length === 0"
        @click="onClear"
        title="Clear all entries"
      >
        ✕ Clear
      </button>
    </div>

    <div class="meta">
      <span>{{ countLabel }}</span>
      <span class="hint">click a row to copy · click save to write .md</span>
    </div>

    <div ref="listEl" class="list">
      <div v-if="filtered.length === 0" class="empty">
        <template v-if="entries.length === 0">
          No log entries yet. Rust key errors and frontend <code>console.log</code>
          will show up here automatically.
        </template>
        <template v-else>No entries match the current filter.</template>
      </div>
      <button
        v-for="entry in filtered"
        :key="entry.id"
        type="button"
        class="row"
        :class="[`level-${entry.level}`, `source-${entry.source}`, { copied: copiedId === entry.id }]"
        @click="copyEntry(entry)"
        title="Click to copy this line"
      >
        <span class="time">{{ formatTime(entry.timestamp_ms) }}</span>
        <span class="source">{{ entry.source }}</span>
        <span class="module">{{ entry.module }}</span>
        <span class="level">{{ entry.level.toUpperCase() }}</span>
        <span class="message">{{ entry.message }}</span>
        <span
          v-if="canSaveReport(entry)"
          class="save-btn"
          role="button"
          tabindex="0"
          title="Write a self-contained .md report for an LLM assistant"
          @click="onSaveReport(entry, $event)"
          @keydown.enter="onSaveReport(entry, $event)"
          @keydown.space.prevent="onSaveReport(entry, $event)"
        >
          save
        </span>
        <span v-else-if="copiedId === entry.id" class="copied">copied</span>
      </button>
    </div>
  </div>
</template>

<style scoped>
.console-panel {
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-height: 0;
}
.header h3 {
  margin: 0 0 4px 0;
  font-size: 1em;
  font-weight: 600;
  color: var(--laipe-text, #1d1d1f);
}
.header .desc {
  margin: 0;
  font-size: 0.78em;
  color: var(--laipe-text-muted, #a1a1a6);
  line-height: 1.5;
}
.header .desc code {
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  font-size: 0.92em;
  background: var(--laipe-bg-elevated, #f5f5f7);
  border: 1px solid var(--laipe-border, #e5e5e7);
  border-radius: 3px;
  padding: 0 4px;
}

.saved-banner {
  padding: 6px 10px;
  background: rgba(52, 199, 89, 0.12);
  border: 1px solid rgba(52, 199, 89, 0.4);
  color: #1d7d3a;
  font-size: 0.78em;
  border-radius: 4px;
}
.saved-banner code {
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  font-size: 0.95em;
  background: rgba(52, 199, 89, 0.12);
  padding: 0 4px;
  border-radius: 2px;
}

.toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  padding: 6px 8px;
  background: var(--laipe-bg-elevated, #fafafa);
  border: 1px solid var(--laipe-border, #e5e5e7);
  border-radius: 6px;
}
.filter-group {
  display: inline-flex;
  align-items: center;
  background: var(--laipe-bg, #ffffff);
  border: 1px solid var(--laipe-border, #e5e5e7);
  border-radius: 5px;
  padding: 1px;
  gap: 1px;
}
.filter-chip {
  padding: 2px 8px;
  font-size: 0.72em;
  font-family: inherit;
  background: transparent;
  color: var(--laipe-text-muted, #6e6e73);
  border: none;
  border-radius: 3px;
  cursor: pointer;
  text-transform: lowercase;
}
.filter-chip:hover {
  color: var(--laipe-text, #1d1d1f);
  background: var(--laipe-border, #e5e5e7);
}
.filter-chip.active {
  background: var(--laipe-accent, #007aff);
  color: white;
  font-weight: 500;
}
.autoscroll-toggle {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 0.75em;
  color: var(--laipe-text-muted, #6e6e73);
  cursor: pointer;
  user-select: none;
}
.autoscroll-toggle input {
  margin: 0;
  cursor: pointer;
}
.search-wrap {
  display: inline-flex;
  align-items: center;
  flex: 1 1 200px;
  min-width: 140px;
  padding: 2px 8px;
  background: var(--laipe-bg, #ffffff);
  border: 1px solid var(--laipe-border, #e5e5e7);
  border-radius: 4px;
}
.search-wrap:focus-within {
  border-color: var(--laipe-accent, #007aff);
}
.search-input {
  flex: 1;
  background: transparent;
  color: var(--laipe-text, #1d1d1f);
  border: none;
  outline: none;
  font-size: 0.8em;
  font-family: inherit;
  min-width: 0;
}
.search-input::placeholder {
  color: var(--laipe-text-muted, #a1a1a6);
}
.action-btn {
  padding: 3px 10px;
  background: transparent;
  color: var(--laipe-text-muted, #6e6e73);
  border: 1px solid var(--laipe-border, #e5e5e7);
  border-radius: 4px;
  cursor: pointer;
  font-size: 0.75em;
  font-family: inherit;
}
.action-btn:hover:not(:disabled) {
  background: var(--laipe-border, #e5e5e7);
  color: var(--laipe-text, #1d1d1f);
}
.action-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.meta {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: 0.72em;
  color: var(--laipe-text-muted, #a1a1a6);
  padding: 0 2px;
}
.meta .hint {
  opacity: 0.7;
}

.list {
  flex: 1 1 auto;
  min-height: 180px;
  max-height: 360px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  background: var(--laipe-bg, #ffffff);
  border: 1px solid var(--laipe-border, #e5e5e7);
  border-radius: 6px;
  padding: 2px;
}
.empty {
  padding: 24px 16px;
  text-align: center;
  font-size: 0.78em;
  color: var(--laipe-text-muted, #a1a1a6);
  opacity: 0.7;
}
.empty code {
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  font-size: 0.92em;
  background: var(--laipe-bg-elevated, #f5f5f7);
  padding: 0 3px;
  border-radius: 2px;
}
.row {
  display: grid;
  grid-template-columns: 64px 70px 110px 50px 1fr auto;
  align-items: center;
  gap: 8px;
  padding: 3px 8px;
  border: none;
  background: transparent;
  color: var(--laipe-text, #1d1d1f);
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  font-size: 0.75em;
  text-align: left;
  border-radius: 3px;
  cursor: pointer;
  min-width: 0;
}
.row:hover {
  background: var(--laipe-bg-elevated, #f5f5f7);
}
.row.copied {
  background: rgba(0, 122, 255, 0.12);
}
.time {
  color: var(--laipe-text-muted, #6e6e73);
  font-variant-numeric: tabular-nums;
}
.source {
  color: var(--laipe-text-muted, #6e6e73);
  font-size: 0.92em;
  text-transform: uppercase;
  letter-spacing: 0.3px;
}
.row.source-frontend .source {
  color: var(--laipe-accent, #007aff);
  opacity: 0.85;
}
.module {
  color: var(--laipe-text-muted, #6e6e73);
  font-size: 0.92em;
  text-align: right;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.level {
  font-size: 0.92em;
  font-weight: 600;
  letter-spacing: 0.3px;
  text-align: center;
  padding: 1px 0;
  border-radius: 2px;
}
.row.level-info .level {
  color: var(--laipe-text-muted, #6e6e73);
  background: var(--laipe-bg-elevated, #f5f5f7);
}
.row.level-warn .level {
  color: #d69e2e;
  background: rgba(214, 158, 46, 0.12);
}
.row.level-warn {
  border-left: 2px solid #d69e2e;
}
.row.level-error .level {
  color: #ff3b30;
  background: rgba(255, 59, 48, 0.12);
}
.row.level-error {
  border-left: 2px solid #ff3b30;
}
.message {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: pre;
  word-break: break-all;
}
.copied {
  color: var(--laipe-accent, #007aff);
  font-size: 0.92em;
  font-weight: 600;
}
.save-btn {
  display: inline-block;
  padding: 1px 8px;
  font-size: 0.85em;
  font-weight: 600;
  color: #ff3b30;
  background: rgba(255, 59, 48, 0.08);
  border: 1px solid rgba(255, 59, 48, 0.4);
  border-radius: 3px;
  cursor: pointer;
  text-transform: lowercase;
  font-family: inherit;
  user-select: none;
}
.save-btn:hover {
  background: rgba(255, 59, 48, 0.18);
}
.save-btn:focus {
  outline: 2px solid var(--laipe-accent, #007aff);
  outline-offset: 1px;
}
</style>
