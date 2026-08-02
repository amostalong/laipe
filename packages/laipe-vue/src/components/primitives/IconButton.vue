<script setup lang="ts">
// IconButton — small, icon-only button. Used in Sidebar and toolbar slots.
//
// `icon` accepts any SVG path data (the `<path d="..." />` content). For
// more complex icons, use the default slot with a full `<svg>`.

defineOptions({ name: "IconButton" });

withDefaults(
  defineProps<{
    title?: string;
    active?: boolean;
    icon?: string;
    size?: number;
  }>(),
  { title: undefined, active: false, icon: undefined, size: 16 },
);

defineSlots<{
  default(): unknown;
}>();

defineEmits<{
  click: [event: MouseEvent];
}>();
</script>

<template>
  <button
    :title="title"
    :class="['icon-btn', { active }]"
    type="button"
    @click="(e) => $emit('click', e)"
  >
    <svg
      v-if="icon"
      :width="size"
      :height="size"
      viewBox="0 0 16 16"
      fill="currentColor"
      aria-hidden="true"
    >
      <path :d="icon" />
    </svg>
    <slot v-else />
  </button>
</template>

<style scoped>
.icon-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--laipe-text-secondary, #6e6e73);
  cursor: pointer;
  padding: 0;
}
.icon-btn:hover {
  background: rgba(0, 0, 0, 0.06);
  color: var(--laipe-text, #1d1d1f);
}
.icon-btn.active {
  background: rgba(0, 122, 255, 0.12);
  color: var(--laipe-accent, #007aff);
}
</style>
