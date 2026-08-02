<script setup lang="ts">
// ToolCallCard — render a single tool call (name + accumulating arguments).
//
// Used inside MessageBubble to render `message.tool_calls[i]`. The card is
// also valid as a standalone component if you want to show a tool call
// detached from a message (e.g. in a "recent actions" sidebar).
//
// During streaming, `arguments` may be a partial JSON string. The card
// shows whatever the upstream has sent so far and re-parses on every
// update so the pretty-printed view appears as soon as the JSON is
// complete (and falls back to a raw view mid-stream).
//
// Extension points:
//   - `header` slot: replace the name row
//   - `footer` slot: append custom content (e.g. result preview)
//   - `default` slot: replace the arguments body entirely
//
// Usage (inside MessageBubble):
//   <ToolCallCard :call="message.tool_calls[0]" />

import { computed, ref } from "vue";
import type { AssistantToolCall } from "laipe-ts";

defineOptions({ name: "ToolCallCard" });

const props = withDefaults(
  defineProps<{
    /** The tool call to render. */
    call: AssistantToolCall;
    /** Optional result returned by the backend (shown in a footer block). */
    result?: string;
    /** Optional error message from the backend (overrides result display). */
    error?: string;
    /** Whether the call is still streaming arguments in. */
    pending?: boolean;
  }>(),
  { result: undefined, error: undefined, pending: false },
);

defineSlots<{
  default(): unknown;
  header(): unknown;
  footer(): unknown;
}>();

const expanded = ref(true);

const displayName = computed(() => props.call.function.name || "<unnamed>");
const callId = computed(() => props.call.id || "—");

const prettyArgs = computed(() => {
  const raw = props.call.function.arguments;
  if (!raw) return "";
  // Try to parse and pretty-print; if it's incomplete JSON, fall back to raw.
  try {
    return JSON.stringify(JSON.parse(raw), null, 2);
  } catch {
    return raw;
  }
});

const argsLooksComplete = computed(() => {
  const raw = props.call.function.arguments;
  if (!raw) return true;
  try {
    JSON.parse(raw);
    return true;
  } catch {
    return false;
  }
});

const prettyResult = computed(() => {
  const r = props.result;
  if (!r) return "";
  try {
    return JSON.stringify(JSON.parse(r), null, 2);
  } catch {
    return r;
  }
});
</script>

<template>
  <div :class="['tool-call', { pending, error: !!error }]">
    <div v-if="$slots.header" class="header-slot"><slot name="header" /></div>
    <div v-else class="header">
      <div class="left">
        <span class="icon" aria-hidden="true">⚙</span>
        <span class="name">{{ displayName }}</span>
        <span v-if="pending" class="status pending">calling…</span>
        <span v-else-if="error" class="status error">error</span>
        <span v-else-if="result" class="status done">done</span>
      </div>
      <button
        type="button"
        class="toggle"
        :aria-expanded="expanded"
        @click="expanded = !expanded"
        :title="expanded ? 'Hide arguments' : 'Show arguments'"
      >
        {{ expanded ? "▾" : "▸" }}
      </button>
    </div>

    <div v-if="$slots.default" class="body"><slot /></div>
    <div v-else-if="expanded" class="body">
      <div class="meta-row">
        <span class="label">id</span>
        <code class="call-id">{{ callId }}</code>
      </div>
      <div class="args-block">
        <div class="args-label">
          arguments
          <span v-if="!argsLooksComplete" class="streaming-tag">streaming</span>
        </div>
        <pre class="args"><code>{{ prettyArgs || "(no args)" }}</code></pre>
      </div>
      <div v-if="error" class="result-block error">
        <div class="result-label">error</div>
        <pre class="result"><code>{{ error }}</code></pre>
      </div>
      <div v-else-if="result" class="result-block">
        <div class="result-label">result</div>
        <pre class="result"><code>{{ prettyResult }}</code></pre>
      </div>
    </div>

    <div v-if="$slots.footer" class="footer-slot"><slot name="footer" /></div>
  </div>
</template>

<style scoped>
.tool-call {
  border: 1px solid var(--laipe-border, #d2d2d7);
  border-radius: 8px;
  background: var(--laipe-bg-elevated, #ffffff);
  font-family: ui-monospace, "SF Mono", Menlo, Consolas, monospace;
  font-size: 0.82em;
  overflow: hidden;
  width: 100%;
  box-sizing: border-box;
}
.tool-call.pending {
  border-color: var(--laipe-accent, #007aff);
  box-shadow: 0 0 0 1px var(--laipe-accent, #007aff) inset;
}
.tool-call.error {
  border-color: #ff3b30;
  box-shadow: 0 0 0 1px #ff3b30 inset;
}
.header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 10px;
  background: var(--laipe-bg-subtle, #f5f5f7);
  border-bottom: 1px solid var(--laipe-border, #d2d2d7);
  gap: 8px;
}
.left {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
}
.icon {
  font-size: 1em;
  color: var(--laipe-accent, #007aff);
  flex-shrink: 0;
}
.name {
  font-weight: 600;
  color: var(--laipe-text, #1d1d1f);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.status {
  font-size: 0.85em;
  padding: 1px 6px;
  border-radius: 4px;
  font-weight: 500;
}
.status.pending {
  background: rgba(0, 122, 255, 0.12);
  color: var(--laipe-accent, #007aff);
  animation: tcPulse 1.2s ease-in-out infinite;
}
.status.done {
  background: rgba(52, 199, 89, 0.15);
  color: #34c759;
}
.status.error {
  background: rgba(255, 59, 48, 0.12);
  color: #ff3b30;
}
@keyframes tcPulse {
  0%, 100% { opacity: 0.5; }
  50% { opacity: 1; }
}
.toggle {
  border: none;
  background: transparent;
  color: var(--laipe-text-muted, #6e6e73);
  font-size: 0.9em;
  cursor: pointer;
  padding: 2px 6px;
  border-radius: 4px;
}
.toggle:hover {
  background: var(--laipe-border, #d2d2d7);
}
.body {
  padding: 8px 10px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.meta-row {
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--laipe-text-muted, #6e6e73);
}
.label,
.args-label,
.result-label {
  font-size: 0.8em;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  color: var(--laipe-text-muted, #6e6e73);
}
.call-id {
  color: var(--laipe-text, #1d1d1f);
  font-size: 0.85em;
}
.args-block,
.result-block {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.args-label,
.result-label {
  display: flex;
  align-items: center;
  gap: 6px;
}
.streaming-tag {
  font-size: 0.85em;
  text-transform: none;
  letter-spacing: normal;
  padding: 1px 5px;
  border-radius: 3px;
  background: rgba(0, 122, 255, 0.12);
  color: var(--laipe-accent, #007aff);
}
.args,
.result {
  margin: 0;
  padding: 6px 8px;
  background: var(--laipe-bg-subtle, #f5f5f7);
  border-radius: 4px;
  color: var(--laipe-text, #1d1d1f);
  font-size: 0.92em;
  white-space: pre-wrap;
  word-wrap: break-word;
  max-height: 240px;
  overflow: auto;
}
.result-block.error .result {
  background: rgba(255, 59, 48, 0.08);
  color: #ff3b30;
}
.header-slot,
.footer-slot {
  padding: 0;
}
</style>
