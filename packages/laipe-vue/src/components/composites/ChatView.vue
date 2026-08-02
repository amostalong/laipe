<script setup lang="ts">
// ChatView — message list + input row. Pure composition of primitives.
//
// Parent owns the message list (passes via `messages` prop + listens to
// `update` events). Parent also drives the streaming state (`status` prop)
// and decides what to do on send (typically calls a chat composable).
//
// Extension points:
//   - `header` slot: rendered above the messages (e.g. model info, banner)
//   - `empty` slot: replaces the default EmptyState (e.g. custom onboarding)
//   - `message-actions` slot: rendered inside each message bubble
//   - `before-messages` / `after-messages` slots: render in the list area
//   - `input-before` / `input-after` slots: render in the input row
//   - `message` slot: fully replace how a single message is rendered

import { ref, nextTick, watch, onMounted } from "vue";
import type { ChatMessage } from "laipe-ts";
import MessageBubble from "../primitives/MessageBubble.vue";
import MessageInput from "../primitives/MessageInput.vue";
import EmptyState from "../primitives/EmptyState.vue";

defineOptions({ name: "ChatView" });

const props = withDefaults(
  defineProps<{
    messages: ChatMessage[];
    status?: "idle" | "streaming";
    /** Override the placeholder text in the input. */
    inputPlaceholder?: string;
    /** Override rows of the textarea. */
    inputRows?: number;
  }>(),
  { status: "idle", inputPlaceholder: undefined, inputRows: 2 },
);

const emit = defineEmits<{
  update: [messages: ChatMessage[]];
  send: [text: string];
  cancel: [];
}>();

defineSlots<{
  header(): unknown;
  empty(): unknown;
  "before-messages"(): unknown;
  "after-messages"(): unknown;
  "message-actions"(props: { message: ChatMessage; index: number }): unknown;
  "message"(props: { message: ChatMessage; index: number; streaming: boolean }): unknown;
  "input-before"(): unknown;
  "input-after"(): unknown;
}>();

const input = ref("");
const messagesEl = ref<HTMLElement | null>(null);

const isStreaming = () => props.status === "streaming";
const canSend = () => !isStreaming() && input.value.trim().length > 0;

onMounted(() => scrollToBottom());
watch(() => props.messages.length, () => nextTick(scrollToBottom));
watch(
  () => {
    const last = props.messages[props.messages.length - 1];
    return last?.content?.length ?? 0;
  },
  () => nextTick(scrollToBottom),
);

function scrollToBottom(): void {
  const el = messagesEl.value;
  if (el) el.scrollTop = el.scrollHeight;
}

function onSend(): void {
  const text = input.value.trim();
  if (!text || isStreaming()) return;
  emit("send", text);
  input.value = "";
}

function onCancel(): void {
  emit("cancel");
}
</script>

<template>
  <div class="chat-view">
    <div v-if="$slots.header" class="header-slot"><slot name="header" /></div>
    <div ref="messagesEl" class="messages">
      <template v-if="messages.length === 0">
        <slot name="empty">
          <EmptyState @prompt="(p) => emit('send', p)" />
        </slot>
      </template>
      <template v-else>
        <slot name="before-messages" />
        <template v-for="(m, i) in messages" :key="i">
          <slot name="message" :message="m" :index="i" :streaming="isStreaming() && i === messages.length - 1 && m.role === 'assistant'">
            <MessageBubble
              :message="m"
              :streaming="isStreaming() && i === messages.length - 1 && m.role === 'assistant'"
            >
              <template v-if="$slots['message-actions']" #actions>
                <slot name="message-actions" :message="m" :index="i" />
              </template>
            </MessageBubble>
          </slot>
        </template>
        <slot name="after-messages" />
      </template>
    </div>
    <MessageInput
      v-model="input"
      :disabled="isStreaming()"
      :placeholder="inputPlaceholder"
      :rows="inputRows"
      @send="onSend"
      @cancel="onCancel"
    >
      <template v-if="$slots['input-before']" #before>
        <slot name="input-before" />
      </template>
      <template v-if="$slots['input-after']" #after>
        <slot name="input-after" />
      </template>
    </MessageInput>
  </div>
</template>

<style scoped>
.chat-view {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
}
.header-slot {
  padding: 8px 16px;
  border-bottom: 1px solid var(--laipe-border, #e5e5e7);
  background: var(--laipe-bg-elevated, #ffffff);
}
.messages {
  flex: 1;
  overflow-y: auto;
  padding: 24px 16px 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  max-width: 800px;
  width: 100%;
  margin: 0 auto;
}
</style>
