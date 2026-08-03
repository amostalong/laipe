<script setup lang="ts">
// ProviderPanel — LLM provider config (model, key, format, effort).
//
// v0.2+ extracted from the Settings modal's #model slot. The panel
// is a "form section" — it renders the model selector + key + format
// + effort, all auto-saving via v-model on the shared config ref.
//
// All edits write through to `useConfig().config` which the
// underlying composable persists (localStorage by default, Tauri
// `tauriConfigStorage` if installed). No Save button — the modal
// pattern's "edit then click save" is gone, matching PlotCraft v0.1.5+.

import type { EffortLevel, ProviderConfig } from "laipe-ts";
import ModelSelector from "../ModelSelector.vue";

defineOptions({ name: "ProviderPanel" });

const props = defineProps<{
  config: ProviderConfig;
}>();

const emit = defineEmits<{
  "update:config": [next: ProviderConfig];
}>();

// v-model shim — we receive a writable ref via the parent and forward
// mutations. (The parent owns the actual storage.)
function patch(p: Partial<ProviderConfig>): void {
  emit("update:config", { ...props.config, ...p });
}
</script>

<template>
  <section class="provider-panel">
    <h2>Provider</h2>
    <p class="hint">
      LLM endpoint + API key. v0.2+ all edits auto-save; no Save button.
    </p>

    <div class="section">
      <div class="section-header">
        <span class="section-title">Model</span>
      </div>
      <ModelSelector
        :model-id="config.model"
        :api-format="config.api_format"
        :effort="config.effort ?? null"
        @update:model-id="(id: string) => patch({ model: id })"
        @update:effort="(lv: EffortLevel | null) => patch({ effort: lv ?? undefined })"
      />
      <small class="help">
        Pick from the curated list, or "Custom…" to type any model id
        (e.g. via OpenRouter). <em>Effort</em> only appears for
        reasoning-capable models.
      </small>
    </div>

    <div class="section">
      <div class="section-header">
        <span class="section-title">API key</span>
      </div>
      <input
        type="password"
        :value="config.api_key"
        @input="(e) => patch({ api_key: (e.target as HTMLInputElement).value })"
        placeholder="sk-…"
        class="text-input"
        autocomplete="off"
        spellcheck="false"
      />
      <small class="help">
        Lives in <code>localStorage</code> by default; switch to
        <code>tauri-plugin-stronghold</code> for OS keyring storage in
        production. The key is never written to the diagnostic
        recorder's request body — the redaction strips
        <code>Authorization</code> headers.
      </small>
    </div>

    <div class="section">
      <div class="section-header">
        <span class="section-title">Endpoint</span>
      </div>
      <input
        type="url"
        :value="config.endpoint"
        @input="(e) => patch({ endpoint: (e.target as HTMLInputElement).value })"
        placeholder="https://api.openai.com/v1"
        class="text-input"
        autocomplete="off"
        spellcheck="false"
      />
      <small class="help">
        Base URL. laipe appends <code>/chat/completions</code> or
        <code>/responses</code> or <code>/messages</code> per the
        chosen format.
      </small>
    </div>
  </section>
</template>

<style scoped>
.provider-panel {
  display: flex;
  flex-direction: column;
  gap: 24px;
  padding: 8px 0;
}
.provider-panel h2 {
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
.section {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.section-header {
  display: flex;
  align-items: center;
  gap: 6px;
}
.section-title {
  font-size: 0.85em;
  font-weight: 600;
  color: var(--laipe-text, #1d1d1f);
  text-transform: uppercase;
  letter-spacing: 0.5px;
}
.text-input {
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  font-size: 0.9em;
  padding: 8px 10px;
  border: 1px solid var(--laipe-border, #d2d2d7);
  border-radius: 6px;
  background: var(--laipe-bg, #ffffff);
  color: var(--laipe-text, #1d1d1f);
  width: 100%;
  max-width: 480px;
}
.text-input:focus {
  outline: none;
  border-color: var(--laipe-accent, #007aff);
}
.help {
  font-size: 0.78em;
  color: var(--laipe-text-muted, #6e6e73);
  line-height: 1.5;
  max-width: 480px;
}
.help code {
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  font-size: 0.92em;
  background: var(--laipe-bg-elevated, #f5f5f7);
  padding: 0 3px;
  border-radius: 2px;
}
.help em {
  font-style: italic;
  color: var(--laipe-text-secondary, #6e6e73);
}
</style>
