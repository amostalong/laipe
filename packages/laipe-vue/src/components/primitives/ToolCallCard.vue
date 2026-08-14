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
// Approval flow (v0.2+): when the per-tool permission is `"ask"`, the
// backend sets `call.status = "pending_approval"` and waits for the
// user to click Approve or Deny. The card renders the action bar
// inline in the header when the parent supplies `onApprove` / `onDeny`
// callbacks. If the callbacks are not provided, the card just shows a
// "pending approval" label (read-only) so consumers that don't need
// user gating can ignore the new behavior.
//
// Extension points:
//   - `header` slot: replace the name row
//   - `footer` slot: append custom content (e.g. result preview)
//   - `default` slot: replace the arguments body entirely
//
// Usage (inside MessageBubble):
//   <ToolCallCard :call="message.tool_calls[0]" />

import { computed, ref } from "vue";
import type { AssistantToolCall, AssistantToolCallStatus } from "laipe-ts";

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
    /** Approve handler — when provided AND status=pending_approval, the
     *  card renders an Approve button. Host app should send the decision
     *  to the backend (e.g. via a Tauri command from `useToolApprovals`)
     *  and update `call.status` to `"running"` afterwards. */
    onApprove?: () => void;
    /** Deny handler — same contract as `onApprove`, but the resulting
     *  `call.status` should be `"denied"`. */
    onDeny?: () => void;
  }>(),
  {
    result: undefined,
    error: undefined,
    pending: false,
    onApprove: undefined,
    onDeny: undefined,
  },
);

defineSlots<{
  default(): unknown;
  header(): unknown;
  footer(): unknown;
}>();

const expanded = ref(true);

const displayName = computed(() => props.call.function.name || "<unnamed>");
const callId = computed(() => props.call.id || "—");

// Effective status — call.status wins; the `pending` prop is kept for
// backward compat with consumers that haven't adopted the new status
// field (e.g. partial-JSON streaming while LLM is still emitting).
const effectiveStatus = computed<AssistantToolCallStatus | undefined>(() => {
  if (props.call.status) return props.call.status;
  if (props.pending) return "streaming";
  return undefined;
});

const isPendingApproval = computed(
  () => effectiveStatus.value === "pending_approval",
);
const isRunning = computed(() => effectiveStatus.value === "running");
const isDenied = computed(() => effectiveStatus.value === "denied");
const isErrorState = computed(() => effectiveStatus.value === "error");
const isDoneState = computed(() => effectiveStatus.value === "done");
const isStreamingState = computed(
  () => effectiveStatus.value === "streaming" || props.pending,
);

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
  <div
    :class="[
      'tool-call',
      {
        pending: isStreamingState,
        error: isErrorState || !!error,
        'awaiting-approval': isPendingApproval,
        denied: isDenied,
        running: isRunning,
      },
    ]"
  >
    <div v-if="$slots.header" class="header-slot"><slot name="header" /></div>
    <div v-else class="header">
      <div class="left">
        <span class="icon" aria-hidden="true">⚙</span>
        <span class="name">{{ displayName }}</span>
        <span v-if="isPendingApproval" class="status pending-approval">awaiting approval</span>
        <span v-else-if="isRunning" class="status running">running…</span>
        <span v-else-if="isDenied" class="status denied">denied</span>
        <span v-else-if="isErrorState || error" class="status error">error</span>
        <span v-else-if="isDoneState || result" class="status done">done</span>
        <span v-else-if="isStreamingState" class="status pending">calling…</span>
      </div>
      <div class="header-actions">
        <!-- Approval bar — only when status=pending_approval AND the
             host wired up the handlers. The buttons are deliberately
             small and color-coded (green / red) to keep the design
             language consistent with the rest of the card. -->
        <div v-if="isPendingApproval" class="approval-bar">
          <button
            v-if="onApprove"
            type="button"
            class="approval-btn approve"
            :title="`Approve ${displayName}`"
            @click="onApprove"
          >
            ✓ Approve
          </button>
          <button
            v-if="onDeny"
            type="button"
            class="approval-btn deny"
            :title="`Deny ${displayName}`"
            @click="onDeny"
          >
            ✕ Deny
          </button>
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
      <div v-if="isErrorState || error" class="result-block error">
        <div class="result-label">error</div>
        <pre class="result"><code>{{ error || call.error || "(no details)" }}</code></pre>
      </div>
      <div v-else-if="isDoneState || result" class="result-block">
        <div class="result-label">result</div>
        <pre class="result"><code>{{ prettyResult || call.result }}</code></pre>
      </div>
      <div v-else-if="isDenied && (result || call.result)" class="result-block denied">
        <div class="result-label">denied — server response</div>
        <pre class="result"><code>{{ prettyResult || call.result }}</code></pre>
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
.tool-call.awaiting-approval {
  border-color: #ff9500;
  box-shadow: 0 0 0 1px #ff9500 inset;
}
.tool-call.running {
  border-color: var(--laipe-accent, #007aff);
  box-shadow: 0 0 0 1px var(--laipe-accent, #007aff) inset;
}
.tool-call.denied {
  border-color: #6e6e73;
  box-shadow: 0 0 0 1px #6e6e73 inset;
  opacity: 0.85;
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
  flex: 1 1 auto;
}
.header-actions {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-shrink: 0;
}
.approval-bar {
  display: flex;
  align-items: center;
  gap: 4px;
}
.approval-btn {
  font-family: inherit;
  font-size: 0.92em;
  font-weight: 500;
  padding: 2px 8px;
  border-radius: 4px;
  cursor: pointer;
  border: 1px solid transparent;
  transition: background 0.12s, border-color 0.12s;
}
.approval-btn.approve {
  background: rgba(52, 199, 89, 0.15);
  color: #248a3d;
  border-color: rgba(52, 199, 89, 0.4);
}
.approval-btn.approve:hover {
  background: rgba(52, 199, 89, 0.25);
  border-color: rgba(52, 199, 89, 0.7);
}
.approval-btn.deny {
  background: rgba(255, 59, 48, 0.12);
  color: #d70015;
  border-color: rgba(255, 59, 48, 0.4);
}
.approval-btn.deny:hover {
  background: rgba(255, 59, 48, 0.2);
  border-color: rgba(255, 59, 48, 0.7);
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
.status.pending-approval {
  background: rgba(255, 149, 0, 0.15);
  color: #c97a00;
}
.status.running {
  background: rgba(0, 122, 255, 0.12);
  color: var(--laipe-accent, #007aff);
  animation: tcPulse 1.2s ease-in-out infinite;
}
.status.done {
  background: rgba(52, 199, 89, 0.15);
  color: #34c759;
}
.status.denied {
  background: rgba(110, 110, 115, 0.15);
  color: #6e6e73;
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
.result-block.denied .result {
  background: rgba(110, 110, 115, 0.08);
  color: #6e6e73;
}
.header-slot,
.footer-slot {
  padding: 0;
}
</style>
