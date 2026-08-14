<script setup lang="ts">
// SettingsModal — ProviderConfig form (Connection + Advanced).
//
// Owns only the protocol-agnostic fields. App-specific sections
// (Model selector, Tools toggles, Console) are passed in via slots
// so laipe-vue doesn't depend on a specific app's component shape.
//
// Slots:
//   - `model`     — rendered above the Connection section
//   - `extra`     — rendered between Advanced and the footer
//   - `footer`    — replaces the default Close button
//
// Props / emits:
//   - `v-model="config"` — ProviderConfig (endpoint, key, model, format, etc.)
//   - `v-model:open`     — boolean. The modal emits `update:open: false` on
//                          every close path (Close button, backdrop, Esc,
//                          form submit, footer button) so `v-model:open`
//                          stays in sync. We also keep emitting `close`
//                          for backward-compat with code that listens
//                          to it directly.
//   - `@close`           — emitted alongside `update:open: false` on every
//                          close path. Kept for callers that prefer the
//                          explicit event name over v-model.

import { ref, watch, onMounted, onUnmounted } from "vue";
import type { ApiFormat, ProviderConfig } from "laipe-ts";

defineOptions({ name: "SettingsModal" });

const props = defineProps<{
  open: boolean;
  modelValue: ProviderConfig;
}>();

const emit = defineEmits<{
  "update:modelValue": [value: ProviderConfig];
  "update:open": [value: boolean];
  close: [];
}>();

defineSlots<{
  default(): unknown;
  model(): unknown;
  extra(): unknown;
  footer(): unknown;
}>();

const draft = ref<ProviderConfig>({ ...props.modelValue });

watch(
  () => props.modelValue,
  (v) => {
    draft.value = { ...v };
  },
  { deep: true },
);

watch(
  () => props.open,
  (open) => {
    if (open) draft.value = { ...props.modelValue };
  },
);

const apiFormats: { value: ApiFormat; label: string; help: string }[] = [
  { value: "openai_chat", label: "OpenAI Chat Completions", help: "POST {endpoint}/chat/completions — works for OpenAI, Azure, GLM, DeepSeek, etc." },
  { value: "openai_responses", label: "OpenAI Responses", help: "POST {endpoint}/responses — newer OpenAI endpoint, event-based SSE." },
  { value: "anthropic_messages", label: "Anthropic Messages", help: "POST {endpoint}/v1/messages — direct from browser with the dangerous-direct-browser-access header." },
];

const temperatureStr = ref(
  draft.value.temperature !== undefined ? String(draft.value.temperature) : "",
);
const maxTokensStr = ref(
  draft.value.max_tokens !== undefined ? String(draft.value.max_tokens) : "",
);

watch(() => draft.value.temperature, (v) => {
  temperatureStr.value = v !== undefined ? String(v) : "";
});
watch(() => draft.value.max_tokens, (v) => {
  maxTokensStr.value = v !== undefined ? String(v) : "";
});

const showAdvanced = ref(false);

function commit(): void {
  emit("update:modelValue", { ...draft.value });
}

/**
 * Single source of truth for closing the modal. Fires both:
 *   - `close`            — explicit event for `@close` listeners
 *   - `update:open: false` — required so `v-model:open` stays in sync
 *
 * Also calls `commit()` first so any in-flight edits to `draft` that
 * haven't been blurred (and therefore haven't fired `@change` yet)
 * still propagate to the parent + get persisted. Without this, a
 * user who types into Endpoint then immediately hits Esc loses the
 * change on the next reload.
 *
 * The `if (props.open)` guard avoids redundant emits when the parent
 * has already set `open = false` externally (defensive).
 *
 * Trade-off: there's no "Cancel / discard" path right now. Closing
 * via any route (X / Esc / backdrop / footer Close / form submit)
 * commits the draft. If the user wants to revert, they can clear the
 * field before closing, or we can add a dedicated Cancel button later.
 */
function closeModal(): void {
  if (!props.open) return;
  commit();
  emit("close");
  emit("update:open", false);
}

function onTemperatureInput(): void {
  const t = temperatureStr.value.trim();
  if (t === "") {
    draft.value.temperature = undefined;
  } else {
    const n = Number(t);
    draft.value.temperature = Number.isFinite(n) ? n : undefined;
  }
  commit();
}

function onMaxTokensInput(): void {
  const t = maxTokensStr.value.trim();
  if (t === "") {
    draft.value.max_tokens = undefined;
  } else {
    const n = Number(t);
    draft.value.max_tokens = Number.isFinite(n) && n > 0 ? Math.floor(n) : undefined;
  }
  commit();
}

function onBackdropClick(e: MouseEvent): void {
  if (e.target === e.currentTarget) closeModal();
}

function onEscape(e: KeyboardEvent): void {
  if (e.key === "Escape" && props.open) closeModal();
}

onMounted(() => window.addEventListener("keydown", onEscape));
onUnmounted(() => window.removeEventListener("keydown", onEscape));
</script>

<template>
  <Transition name="modal">
    <div v-if="open" class="modal-backdrop" @click="onBackdropClick">
      <div class="modal" role="dialog" aria-labelledby="settings-title">
        <header class="modal-header">
          <h2 id="settings-title">Settings</h2>
          <button class="btn-close" title="Close (Esc)" @click="closeModal()">
            <svg class="icon" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
              <path d="M4.646 4.646a.5.5 0 0 1 .708 0L8 7.293l2.646-2.647a.5.5 0 0 1 .708.708L8.707 8l2.647 2.646a.5.5 0 0 1-.708.708L8 8.707l-2.646 2.647a.5.5 0 0 1-.708-.708L7.293 8 4.646 5.354a.5.5 0 0 1 0-.708Z" />
            </svg>
          </button>
        </header>

        <form class="modal-body" @submit.prevent="closeModal()">
          <!-- `model` slot: app-supplied fancy model selector (e.g.
               ModelEffortSelector with reasoning effort controls).
               When provided, it REPLACES the default Model input
               below — apps that need a richer selector should pass
               it; apps that just need a plain text field get the
               default. -->
          <slot name="model" />

          <!-- Default Model input — only shown when no `model` slot
               is supplied. Single-provider apps (and most v0.1 apps)
               don't need a fancy selector; a plain text field is
               enough. -->
          <label v-if="!$slots.model" class="field">
            <span class="label">Model</span>
            <input
              v-model="draft.model"
              type="text"
              placeholder="gpt-4o-mini"
              spellcheck="false"
              @change="commit"
            />
            <small class="help">
              The model name to send in the API request. Must be one
              your endpoint actually serves (e.g. <code>gpt-4o</code>,
              <code>claude-3-5-sonnet-20241022</code>).
            </small>
          </label>

          <!-- Connection: Endpoint, API Key, API Format -->
          <section class="field-group">
            <label class="field">
              <span class="label">Endpoint</span>
              <input
                v-model="draft.endpoint"
                type="url"
                placeholder="https://api.openai.com/v1"
                @change="commit"
              />
              <small class="help">
                The base URL of your LLM API. laipe appends
                <code>/chat/completions</code>, <code>/responses</code>, or
                <code>/v1/messages</code> based on the format.
              </small>
            </label>

            <label class="field">
              <span class="label">API Key</span>
              <input
                v-model="draft.api_key"
                type="password"
                placeholder="sk-..."
                autocomplete="off"
                spellcheck="false"
                @change="commit"
              />
              <small class="help">
                Stored only in this app's <code>localStorage</code>. Never
                transmitted anywhere except to the endpoint above.
              </small>
            </label>

            <label class="field">
              <span class="label">API Format</span>
              <select v-model="draft.api_format" @change="commit">
                <option v-for="f in apiFormats" :key="f.value" :value="f.value">
                  {{ f.label }}
                </option>
              </select>
              <small class="help">
                {{ apiFormats.find((f) => f.value === draft.api_format)?.help }}
              </small>
            </label>
          </section>

          <!-- Advanced (collapsible) -->
          <details class="advanced-block" :open="showAdvanced" @toggle="showAdvanced = ($event.target as HTMLDetailsElement).open">
            <summary>Advanced</summary>
            <label class="field">
              <span class="label">Temperature</span>
              <input
                v-model="temperatureStr"
                type="text"
                inputmode="decimal"
                placeholder="leave empty for upstream default"
                @change="onTemperatureInput"
              />
            </label>
            <label class="field">
              <span class="label">Max Tokens</span>
              <input
                v-model="maxTokensStr"
                type="text"
                inputmode="numeric"
                placeholder="leave empty for upstream default"
                @change="onMaxTokensInput"
              />
            </label>
          </details>

          <!-- `extra` slot: app-supplied sections (Tools, Console, etc.) -->
          <slot name="extra" />
        </form>

        <footer class="modal-footer">
          <slot name="footer">
            <button type="button" class="btn-secondary" @click="closeModal()">Close</button>
          </slot>
        </footer>
      </div>
    </div>
  </Transition>
</template>

<style scoped>
.modal-backdrop {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.4);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
  padding: 16px;
}
.modal {
  background: var(--laipe-bg-elevated, #ffffff);
  border-radius: 12px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.08);
  width: 100%;
  max-width: 560px;
  max-height: 90vh;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 20px;
  border-bottom: 1px solid var(--laipe-border, #e5e5e7);
  flex-shrink: 0;
}
.modal-header h2 {
  margin: 0;
  font-size: 1.1em;
  font-weight: 600;
}
.btn-close {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--laipe-text-secondary, #6e6e73);
  cursor: pointer;
}
.btn-close:hover {
  background: rgba(0, 0, 0, 0.06);
  color: var(--laipe-text, #1d1d1f);
}
.modal-body {
  padding: 18px 20px;
  display: flex;
  flex-direction: column;
  gap: 16px;
  overflow-y: auto;
  flex: 1;
}
.field-group {
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.field {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.label {
  font-size: 0.85em;
  font-weight: 500;
  color: var(--laipe-text, #1d1d1f);
}
.field input,
.field select {
  padding: 8px 12px;
  border: 1px solid var(--laipe-border-strong, #d2d2d7);
  border-radius: 6px;
  font-size: 14px;
  background: var(--laipe-bg-elevated, #ffffff);
  color: var(--laipe-text, #1d1d1f);
  font-family: inherit;
}
.field input:focus,
.field select:focus {
  outline: none;
  border-color: var(--laipe-accent, #007aff);
  box-shadow: 0 0 0 3px rgba(0, 122, 255, 0.15);
}
.help {
  font-size: 0.78em;
  color: var(--laipe-text-muted, #a1a1a6);
  line-height: 1.5;
}
.help code {
  background: rgba(0, 0, 0, 0.05);
  padding: 1px 4px;
  border-radius: 3px;
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
}

details.advanced-block {
  border-top: 1px solid var(--laipe-border, #e5e5e7);
  padding-top: 12px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}
details summary {
  cursor: pointer;
  font-size: 0.85em;
  font-weight: 500;
  color: var(--laipe-text-secondary, #6e6e73);
  user-select: none;
  list-style: none;
}
details summary::-webkit-details-marker { display: none; }
details summary::before {
  content: "▸";
  display: inline-block;
  margin-right: 6px;
  transition: transform 0.15s ease;
}
details[open] summary::before { transform: rotate(90deg); }

.modal-footer {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 12px 20px;
  border-top: 1px solid var(--laipe-border, #e5e5e7);
  background: #fafafa;
  flex-shrink: 0;
}
.btn-secondary {
  padding: 8px 16px;
  border: 1px solid var(--laipe-border-strong, #d2d2d7);
  border-radius: 6px;
  background: var(--laipe-bg-elevated, #ffffff);
  color: var(--laipe-text, #1d1d1f);
  font-size: 14px;
  font-weight: 500;
  cursor: pointer;
  font-family: inherit;
}
.btn-secondary:hover { background: #f0f0f0; }
.modal-enter-active,
.modal-leave-active { transition: opacity 0.18s ease; }
.modal-enter-active .modal,
.modal-leave-active .modal { transition: transform 0.18s ease; }
.modal-enter-from, .modal-leave-to { opacity: 0; }
.modal-enter-from .modal, .modal-leave-to .modal {
  transform: scale(0.95) translateY(-8px);
}
.icon { width: 16px; height: 16px; }
</style>
