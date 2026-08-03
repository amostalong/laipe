<script setup lang="ts">
// laipe-app — root component.
//
// v0.2+: routing structure. The app has two routes:
//   - "main"    → MainView (chat, topbar, optional conversation Sidebar)
//   - "settings"→ SettingsView (PlotCraft-shape sidebar + content panels)
//
// The conversation Sidebar (left, list of conversations) only shows
// on the "main" route. On "settings" the page has its own internal
// sidebar (LLM/General/Diagnostics nav), so the conversation list
// would be redundant.
//
// Toast (new chat / no API key) lives here because both views might
// trigger it (e.g. an "API key missing" toast on app start). It's
// shown via Teleport-style absolute positioning.

import { computed, ref } from "vue";
import { useRoute } from "vue-router";
import { Sidebar, useConversations } from "laipe-vue";

const route = useRoute();
const { conversations, currentId, select, remove, create, clearAll } = useConversations();

const sidebarOpen = ref(true);
const toast = ref<string | null>(null);
let toastTimer: ReturnType<typeof setTimeout> | null = null;

const isMain = computed(() => route.name === "main");
const showConversationSidebar = computed(() => isMain.value);

function showToast(msg: string, duration = 4000): void {
  toast.value = msg;
  if (toastTimer) clearTimeout(toastTimer);
  toastTimer = setTimeout(() => {
    toast.value = null;
    toastTimer = null;
  }, duration);
}

function handleSelect(id: string): void {
  select(id);
}
function handleRemove(id: string): void {
  remove(id);
}
function handleNewChat(): void {
  create();
}
function handleClearAll(): void {
  if (!confirm("Delete all conversations? This cannot be undone.")) return;
  clearAll();
}
</script>

<template>
  <div class="app">
    <Sidebar
      v-if="showConversationSidebar"
      :conversations="conversations"
      :current-id="currentId"
      :collapsed="!sidebarOpen"
      @select="handleSelect"
      @create="handleNewChat"
      @remove="handleRemove"
      @toggle="sidebarOpen = false"
    />

    <main class="main">
      <router-view />
    </main>

    <Transition name="toast">
      <div v-if="toast" class="toast" @click="toast = null">
        <span class="toast-msg">{{ toast }}</span>
        <span class="toast-dismiss">(click to dismiss)</span>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.app {
  display: flex;
  height: 100vh;
  background: #fafafa;
  position: relative;
}
.main {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
  background: #fafafa;
}
.toast {
  position: fixed;
  bottom: 20px;
  right: 20px;
  background: #1d1d1f;
  color: white;
  padding: 12px 16px;
  border-radius: 8px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.08);
  max-width: 480px;
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 12px;
  font-size: 0.9em;
  z-index: 200;
}
.toast-msg { flex: 1; word-break: break-word; }
.toast-dismiss { opacity: 0.6; font-size: 0.85em; }
.toast-enter-active, .toast-leave-active { transition: all 0.2s ease; }
.toast-enter-from, .toast-leave-to { opacity: 0; transform: translateY(20px); }
</style>
