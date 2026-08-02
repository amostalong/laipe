<script setup lang="ts">
// MessageBubble — renders a single chat message.
//
// Pure presentational. Supports three roles (user / assistant / system)
// and a `streaming` state for the last assistant message while it's
// still receiving deltas. When the message carries `tool_calls`, each
// call is rendered inline below the content via `ToolCallCard`.
//
// Extension points:
//   - `default` slot: replace the content entirely (use for tool calls,
//     code blocks, or any custom rendering)
//   - `actions` slot: render action buttons (copy, regenerate, etc.)
//   - `header` slot: replace the role label / status row
//   - `tool-calls` slot: replace the per-call ToolCallCard rendering
//     (e.g. collapse multiple calls into a single summary line)

import { computed } from "vue";
import type { ChatMessage as ChatMessageT } from "laipe-ts";
import ToolCallCard from "./ToolCallCard.vue";

defineOptions({ name: "MessageBubble" });

const props = withDefaults(
  defineProps<{
    message: ChatMessageT;
    streaming?: boolean;
    /** Override the role label, e.g. for custom roles. */
    roleLabel?: string;
  }>(),
  { streaming: false, roleLabel: undefined },
);

defineSlots<{
  default(): unknown;
  actions(): unknown;
  header(): unknown;
  "tool-calls"(props: { calls: NonNullable<ChatMessageT["tool_calls"]> }): unknown;
}>();

const label = computed(() => {
  if (props.roleLabel) return props.roleLabel;
  if (props.message.role === "user") return "You";
  if (props.message.role === "tool") return "Tool";
  if (props.message.role === "system") return "System";
  return props.streaming ? "Assistant · streaming" : "Assistant";
});

const toolCalls = computed(() => props.message.tool_calls ?? []);
</script>

<template>
  <div :class="['msg', message.role, { streaming }]">
    <div v-if="$slots.header" class="header-slot"><slot name="header" /></div>
    <div v-else class="meta">
      <span class="role">{{ label }}</span>
      <span v-if="streaming" class="dot"><span></span></span>
    </div>
    <div v-if="$slots.default" class="content"><slot /></div>
    <div v-else class="content">{{ message.content }}</div>
    <div v-if="toolCalls.length > 0" class="tool-calls">
      <slot name="tool-calls" :calls="toolCalls">
        <ToolCallCard
          v-for="(call, i) in toolCalls"
          :key="i"
          :call="call"
          :pending="streaming"
        />
      </slot>
    </div>
    <div v-if="$slots.actions" class="actions"><slot name="actions" /></div>
  </div>
</template>

<style scoped>
.msg {
  padding: 10px 14px;
  border-radius: 12px;
  white-space: pre-wrap;
  word-wrap: break-word;
  line-height: 1.5;
  font-size: 0.95em;
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 6px;
  max-width: 85%;
}
.meta {
  display: flex;
  align-items: center;
  gap: 6px;
}
.role {
  font-size: 0.7em;
  color: #888;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  font-weight: 500;
}
.dot span {
  display: inline-block;
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: #007aff;
  animation: pulse 1.2s ease-in-out infinite;
}
@keyframes pulse {
  0%, 100% { opacity: 0.3; }
  50% { opacity: 1; }
}
.user {
  align-self: flex-end;
  background: #007aff;
  color: white;
  border-bottom-right-radius: 4px;
}
.user .role {
  color: rgba(255, 255, 255, 0.75);
}
.assistant {
  align-self: flex-start;
  background: white;
  color: #1d1d1f;
  border: 1px solid #e5e5e7;
  border-bottom-left-radius: 4px;
}
.tool {
  align-self: flex-start;
  background: #fff8e1;
  color: #1d1d1f;
  border: 1px solid #ffe082;
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  font-size: 0.85em;
  border-bottom-left-radius: 4px;
}
.system {
  align-self: center;
  background: #f0f0f0;
  color: #6e6e73;
  font-size: 0.85em;
  max-width: 90%;
  border-radius: 4px;
}
.header-slot,
.actions {
  display: flex;
  align-items: center;
  gap: 6px;
}
.tool-calls {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-top: 4px;
  white-space: normal;
  word-wrap: normal;
  max-width: 100%;
}
</style>
