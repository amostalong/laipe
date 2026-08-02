<script setup lang="ts">
// Sidebar — multi-conversation list.
//
// Extension points:
//   - `header` slot: above the "New chat" button (e.g. branding)
//   - `footer` slot: below the conversation list (e.g. settings button)
//   - `item` slot: fully replace a single conversation item's rendering
//   - `item-actions` slot: action buttons per conversation (default: delete)

import { computed } from "vue";
import type { Conversation } from "../../composables/useConversations";
import IconButton from "../primitives/IconButton.vue";

defineOptions({ name: "Sidebar" });

const props = defineProps<{
  conversations: Conversation[];
  currentId: string | null;
  collapsed: boolean;
}>();

const emit = defineEmits<{
  select: [id: string];
  create: [];
  remove: [id: string];
  toggle: [];
}>();

defineSlots<{
  header(): unknown;
  footer(): unknown;
  item(props: { conversation: Conversation; active: boolean }): unknown;
  "item-actions"(props: { conversation: Conversation }): unknown;
}>();

const sorted = computed(() => {
  return [...props.conversations].sort((a, b) => b.updatedAt - a.updatedAt);
});

function relativeTime(ts: number): string {
  const diff = Date.now() - ts;
  if (diff < 60_000) return "just now";
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)}m ago`;
  if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)}h ago`;
  return `${Math.floor(diff / 86_400_000)}d ago`;
}

function confirmRemove(id: string, e: Event): void {
  e.stopPropagation();
  if (confirm("Delete this conversation?")) {
    emit("remove", id);
  }
}
</script>

<template>
  <aside :class="['sidebar', { collapsed }]">
    <slot name="header" />
    <div class="sidebar-header">
      <button class="btn-new" @click="emit('create')">
        <svg class="icon" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
          <path d="M8 2a.5.5 0 0 1 .5.5v5h5a.5.5 0 0 1 0 1h-5v5a.5.5 0 0 1-1 0v-5h-5a.5.5 0 0 1 0-1h5v-5A.5.5 0 0 1 8 2Z" />
        </svg>
        <span>New chat</span>
      </button>
      <IconButton
        v-if="!collapsed"
        title="Collapse sidebar"
        icon="M2 3.5A.5.5 0 0 1 2.5 3h11a.5.5 0 0 1 0 1h-11a.5.5 0 0 1-.5-.5Zm0 4A.5.5 0 0 1 2.5 7h11a.5.5 0 0 1 0 1h-11a.5.5 0 0 1-.5-.5Zm0 4a.5.5 0 0 1 .5-.5h7a.5.5 0 0 1 0 1h-7a.5.5 0 0 1-.5-.5Z"
        @click="emit('toggle')"
      />
    </div>

    <div v-if="!collapsed" class="conversation-list">
      <div v-if="sorted.length === 0" class="empty">
        No conversations yet.<br />Click <strong>New chat</strong> to start.
      </div>
      <template v-for="conv in sorted" :key="conv.id">
        <slot
          name="item"
          :conversation="conv"
          :active="conv.id === currentId"
        >
          <button
            :class="['conversation-item', { active: conv.id === currentId }]"
            @click="emit('select', conv.id)"
          >
            <div class="title">{{ conv.title }}</div>
            <div class="meta">
              <span>{{ relativeTime(conv.updatedAt) }}</span>
              <span v-if="conv.messages.length > 0" class="count">
                {{ conv.messages.length }} msg{{ conv.messages.length === 1 ? '' : 's' }}
              </span>
            </div>
            <slot name="item-actions" :conversation="conv">
              <button
                class="btn-remove"
                title="Delete conversation"
                @click="confirmRemove(conv.id, $event)"
              >
                <svg class="icon" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
                  <path d="M5.5 5.5A.5.5 0 0 1 6 5h4a.5.5 0 0 1 0 1H6a.5.5 0 0 1-.5-.5ZM2.5 3.5A.5.5 0 0 1 3 3h10a.5.5 0 0 1 0 1H3a.5.5 0 0 1-.5-.5ZM3.118 5h9.764l-.804 8.066A2 2 0 0 1 10.092 15H5.908a2 2 0 0 1-1.986-1.934L3.118 5Z" />
                </svg>
              </button>
            </slot>
          </button>
        </slot>
      </template>
    </div>

    <slot name="footer" />
  </aside>
</template>

<style scoped>
.sidebar {
  width: 280px;
  background: var(--laipe-bg-sidebar, #f5f5f7);
  border-right: 1px solid var(--laipe-border, #e5e5e7);
  display: flex;
  flex-direction: column;
  flex-shrink: 0;
  transition: width 0.18s ease;
  overflow: hidden;
}
.sidebar.collapsed {
  width: 0;
  border-right: none;
}
.sidebar-header {
  display: flex;
  gap: 6px;
  padding: 12px;
  border-bottom: 1px solid var(--laipe-border, #e5e5e7);
  background: var(--laipe-bg-sidebar, #f5f5f7);
  flex-shrink: 0;
}
.btn-new {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 8px 12px;
  border: 1px solid var(--laipe-border-strong, #d2d2d7);
  border-radius: 8px;
  background: var(--laipe-bg-elevated, #ffffff);
  color: var(--laipe-text, #1d1d1f);
  font-size: 0.9em;
  font-weight: 500;
  cursor: pointer;
  font-family: inherit;
  transition: background 0.12s ease;
}
.btn-new:hover {
  background: #f0f0f0;
}
.conversation-list {
  flex: 1;
  overflow-y: auto;
  padding: 8px;
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.empty {
  padding: 24px 16px;
  text-align: center;
  color: var(--laipe-text-muted, #a1a1a6);
  font-size: 0.85em;
  line-height: 1.6;
}
.conversation-item {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 10px 12px;
  border: none;
  border-radius: 6px;
  background: transparent;
  text-align: left;
  cursor: pointer;
  width: 100%;
  font-family: inherit;
  transition: background 0.1s ease;
}
.conversation-item:hover {
  background: rgba(0, 0, 0, 0.04);
}
.conversation-item.active {
  background: rgba(0, 122, 255, 0.1);
}
.conversation-item .title {
  font-size: 0.9em;
  color: var(--laipe-text, #1d1d1f);
  font-weight: 500;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  padding-right: 24px;
}
.conversation-item .meta {
  display: flex;
  gap: 8px;
  font-size: 0.75em;
  color: var(--laipe-text-muted, #a1a1a6);
}
.btn-remove {
  position: absolute;
  top: 8px;
  right: 8px;
  width: 22px;
  height: 22px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: var(--laipe-text-muted, #a1a1a6);
  cursor: pointer;
  opacity: 0;
  transition: opacity 0.1s ease, background 0.1s ease;
  padding: 0;
}
.conversation-item:hover .btn-remove,
.conversation-item.active .btn-remove {
  opacity: 1;
}
.btn-remove:hover {
  background: rgba(255, 59, 48, 0.12);
  color: var(--laipe-error, #ff3b30);
}
.icon {
  width: 16px;
  height: 16px;
  display: inline-block;
  vertical-align: middle;
  flex-shrink: 0;
}
@media (max-width: 720px) {
  .sidebar {
    position: absolute;
    z-index: 10;
    height: 100%;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.08);
  }
}
</style>
