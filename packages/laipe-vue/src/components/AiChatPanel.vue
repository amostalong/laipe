<script setup lang="ts">
// AiChatPanel — one-line drop-in chat UI.
//
// Internally uses ChatView + useChat + useConfig + useConversations.
// For full control, use those primitives directly (see laipe-vue exports).
//
// Extension points:
//   - All slots are forwarded to ChatView (header, empty, message,
//     message-actions, before-messages, after-messages, input-before,
//     input-after)
//
// Usage:
//   <AiChatPanel :config="cfg" @error="onError" />

import { computed, onMounted } from "vue";
import type { ChatMessage, ProviderConfig, ToolDefinition } from "laipe-ts";
import type { StreamSource } from "../streams";
import { useChat, useConfig, useConversations } from "../composables";
import { defaultStreamSource } from "../streams";
import ChatView from "./composites/ChatView.vue";

defineOptions({ name: "AiChatPanel" });

const props = withDefaults(
  defineProps<{
    config?: ProviderConfig;
    /** Override the default stream source (default: auto-detect Tauri vs browser). */
    stream?: StreamSource;
    /**
     * Optional tool definitions. When provided, the upstream is told the
     * LLM is tool-aware and may emit tool_calls during the stream. The
     * default (no tools) preserves the prior "no tools on the wire" behavior.
     */
    tools?: ToolDefinition[];
  }>(),
  { config: undefined, stream: undefined, tools: () => [] },
);

const emit = defineEmits<{
  error: [message: string];
}>();

// Use provided config or fall back to the global one from useConfig()
const fallback = useConfig();
const activeConfig = computed<ProviderConfig>(() => props.config ?? fallback.config.value);

const conv = useConversations();
const { status, send, cancel } = useChat(
  props.stream ?? defaultStreamSource(),
  props.tools,
);

const messages = computed<ChatMessage[]>(() => conv.current.value?.messages ?? []);

onMounted(() => {
  if (!fallback.isReady()) {
    setTimeout(
      () => emit("error", "No API key configured. Open Settings to add one."),
      400,
    );
  }
});

async function handleSend(text: string): Promise<void> {
  if (!activeConfig.value.api_key) {
    emit("error", "No API key configured. Open Settings to add one.");
    return;
  }
  const next: ChatMessage[] = [
    ...messages.value,
    { role: "user", content: text },
  ];
  conv.setMessages(next);
  await send(activeConfig.value, next);
}

function handleCancel(): void {
  cancel();
}
</script>

<template>
  <ChatView
    :messages="messages"
    :status="status"
    @send="handleSend"
    @cancel="handleCancel"
    @update="(m: ChatMessage[]) => conv.setMessages(m)"
  >
    <!-- Forward all slots through to ChatView so consumers can customize
         header / empty / per-message / input area without reaching into
         the primitives. -->
    <template v-if="$slots.header" #header><slot name="header" /></template>
    <template v-if="$slots.empty" #empty><slot name="empty" /></template>
    <template v-if="$slots['before-messages']" #before-messages><slot name="before-messages" /></template>
    <template v-if="$slots['after-messages']" #after-messages><slot name="after-messages" /></template>
    <template v-if="$slots['message-actions']" #message-actions="props">
      <slot name="message-actions" v-bind="props" />
    </template>
    <template v-if="$slots.message" #message="props">
      <slot name="message" v-bind="props" />
    </template>
    <template v-if="$slots['input-before']" #input-before><slot name="input-before" /></template>
    <template v-if="$slots['input-after']" #input-after><slot name="input-after" /></template>
  </ChatView>
</template>
