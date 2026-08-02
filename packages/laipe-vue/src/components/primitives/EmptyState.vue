<script setup lang="ts">
// EmptyState — shown when a conversation has no messages.
//
// Defaults to 4 sample prompts (English, deterministic). Override either
// by passing `samples` or by using the `default` slot for fully custom
// content.

defineOptions({ name: "EmptyState" });

withDefaults(
  defineProps<{
    title?: string;
    subtitle?: string;
    samples?: { title: string; prompt: string }[];
  }>(),
  {
    title: "laipe",
    subtitle: "A minimal chat starter built on laipe.",
    samples: () => [
      { title: "Explain a concept", prompt: "Explain how SSE (server-sent events) works in 3 short paragraphs." },
      { title: "Write code", prompt: "Write a Python function that flattens a nested list of arbitrary depth." },
      { title: "Brainstorm", prompt: "Give me 5 names for a small TypeScript library that wraps an HTTP API." },
      { title: "Translate", prompt: "Translate this to English (formal register): 'Laipe framework does not include Vue'." },
    ],
  },
);

defineEmits<{ prompt: [text: string] }>();
defineSlots<{ default(): unknown }>();
</script>

<template>
  <div class="empty">
    <slot>
      <h1>{{ title }}</h1>
      <p class="subtitle">{{ subtitle }}</p>
      <div v-if="samples && samples.length" class="samples">
        <button
          v-for="(s, i) in samples"
          :key="i"
          class="sample-card"
          @click="$emit('prompt', s.prompt)"
        >
          <div class="sample-title">{{ s.title }}</div>
          <div class="sample-prompt">{{ s.prompt }}</div>
        </button>
      </div>
    </slot>
  </div>
</template>

<style scoped>
.empty {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 32px;
  text-align: center;
  max-width: 640px;
  margin: 0 auto;
  width: 100%;
}
h1 {
  margin: 0 0 6px 0;
  font-size: 1.8em;
  font-weight: 600;
  color: var(--laipe-text, #1d1d1f);
}
.subtitle {
  margin: 0 0 32px 0;
  color: var(--laipe-text-secondary, #6e6e73);
}
.samples {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 12px;
  width: 100%;
}
.sample-card {
  text-align: left;
  padding: 16px;
  background: var(--laipe-bg-elevated, #ffffff);
  border: 1px solid var(--laipe-border, #e5e5e7);
  border-radius: 8px;
  cursor: pointer;
  font-family: inherit;
  transition: border-color 0.12s ease, transform 0.12s ease;
}
.sample-card:hover {
  border-color: var(--laipe-accent, #007aff);
  transform: translateY(-1px);
}
.sample-title {
  font-weight: 500;
  font-size: 0.9em;
  margin-bottom: 6px;
  color: var(--laipe-accent, #007aff);
}
.sample-prompt {
  font-size: 0.85em;
  color: var(--laipe-text-secondary, #6e6e73);
  line-height: 1.5;
}
@media (max-width: 600px) {
  .samples { grid-template-columns: 1fr; }
}
</style>
