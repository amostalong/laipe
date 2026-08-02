<script setup lang="ts">
// MessageInput — textarea + send/stop button row.
//
// Pure presentational. Emits `send` (with the typed text) and `cancel`.
// Parent owns the input value (v-model), the disabled state, and what
// happens on send.
//
// Extension points:
//   - `before` / `after` slots: add buttons, attachments, voice input, etc.
//   - `placeholder` prop override
//   - `disabled` prop for streaming state

defineOptions({ name: "MessageInput" });

const props = withDefaults(
  defineProps<{
    modelValue: string;
    disabled?: boolean;
    placeholder?: string;
    rows?: number;
  }>(),
  { disabled: false, placeholder: "Type a message…", rows: 2 },
);

const emit = defineEmits<{
  "update:modelValue": [value: string];
  send: [];
  cancel: [];
}>();

defineSlots<{
  before(): unknown;
  after(): unknown;
}>();

function onInput(e: Event) {
  emit("update:modelValue", (e.target as HTMLTextAreaElement).value);
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Enter" && !e.shiftKey) {
    e.preventDefault();
    if (!props.disabled && props.modelValue.trim().length > 0) {
      emit("send");
    }
  }
}
</script>

<template>
  <form class="input-row" @submit.prevent="emit('send')">
    <slot name="before" />
    <textarea
      :value="modelValue"
      :disabled="disabled"
      :placeholder="placeholder"
      :rows="rows"
      @input="onInput"
      @keydown="onKeydown"
    />
    <div class="actions">
      <slot name="after" />
      <button
        v-if="disabled"
        type="button"
        class="btn-cancel"
        @click="emit('cancel')"
      >
        Stop
      </button>
      <button
        v-else
        type="submit"
        class="btn-send"
        :disabled="modelValue.trim().length === 0"
      >
        Send
      </button>
    </div>
  </form>
</template>

<style scoped>
.input-row {
  display: flex;
  gap: 8px;
  padding: 12px 16px 16px;
  background: var(--laipe-bg, #fafafa);
  border-top: 1px solid var(--laipe-border, #e5e5e7);
  align-items: flex-end;
  flex-shrink: 0;
}
textarea {
  flex: 1;
  resize: none;
  padding: 10px 14px;
  border: 1px solid var(--laipe-border-strong, #d2d2d7);
  border-radius: 8px;
  font-size: 14px;
  line-height: 1.4;
  min-height: 44px;
  max-height: 160px;
  background: var(--laipe-bg-elevated, #ffffff);
  color: var(--laipe-text, #1d1d1f);
  font-family: inherit;
}
textarea:focus {
  outline: none;
  border-color: var(--laipe-accent, #007aff);
  box-shadow: 0 0 0 3px rgba(0, 122, 255, 0.15);
}
textarea:disabled {
  background: #f5f5f5;
  cursor: not-allowed;
}
.actions {
  display: flex;
  gap: 6px;
  flex-shrink: 0;
}
button {
  padding: 10px 18px;
  border: none;
  border-radius: 8px;
  font-weight: 500;
  font-size: 14px;
  white-space: nowrap;
  cursor: pointer;
  font-family: inherit;
}
.btn-send {
  background: var(--laipe-accent, #007aff);
  color: white;
}
.btn-send:hover:not(:disabled) {
  background: var(--laipe-accent-hover, #0066d6);
}
.btn-send:disabled {
  background: #ccc;
  cursor: not-allowed;
}
.btn-cancel {
  background: var(--laipe-error, #ff3b30);
  color: white;
}
.btn-cancel:hover {
  background: #e02e23;
}
</style>
