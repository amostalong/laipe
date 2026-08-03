<script setup lang="ts">
// laipe-app — root component.
//
// v0.2+: routing + tab system.
//   - Routes:    `/` (chat) and `/settings` (PlotCraft-shape page).
//   - Tabs:      Chat + Settings, both pinned. The tab bar sits at
//                the top of the app; clicking a tab navigates to
//                the corresponding route. The active tab is
//                derived from `route.name`, so the URL stays the
//                source of truth.
//   - Sidebar:   the conversation Sidebar only shows on the
//                "main" route. On `/settings` the page has its
//                own internal sidebar (LLM/General/Diagnostics
//                nav), so the conversation list would be
//                redundant.
//
// Toast (e.g. "no API key" on app start) lives here because both
// views might trigger it.

import { computed, ref } from "vue";
import { useRoute, useRouter } from "vue-router";
import { Sidebar, TabsBar, useConversations, type Tab } from "laipe-vue";

const route = useRoute();
const router = useRouter();
const { conversations, currentId, select, remove, create, clearAll } =
  useConversations();

const sidebarOpen = ref(true);
const toast = ref<string | null>(null);
let toastTimer: ReturnType<typeof setTimeout> | null = null;

// Tab definitions for v0.2. Both are pinned (always present, not
// closeable). The icon path is inline Lucide data (MIT-licensed).
// To add more tabs later (e.g. per-conversation tabs, a
// diagnostics-only tab) just append to this array.
const tabs: Tab[] = [
  {
    id: "main",
    title: "Chat",
    pinned: true,
    iconPath:
      "M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z",
  },
  {
    id: "settings",
    title: "Settings",
    pinned: true,
    iconPath:
      "M9.405 1.05c-.413-1.4-2.397-1.4-2.81 0l-.1.34a1.464 1.464 0 0 1-2.105.872l-.31-.17c-1.283-.698-2.686.705-1.987 1.987l.169.311c.446.82.023 1.841-.872 2.105l-.34.1c-1.4.413-1.4 2.397 0 2.81l.34.1a1.464 1.464 0 0 1 .872 2.105l-.17.31c-.698 1.283.705 2.686 1.987 1.987l.311-.169a1.464 1.464 0 0 1 2.105.872l.1.34c.413 1.4 2.397 1.4 2.81 0l.1-.34a1.464 1.464 0 0 1 2.105-.872l.31.17c1.283.698 2.686-.705 1.987-1.987l-.169-.311a1.464 1.464 0 0 1 .872-2.105l.34-.1c1.4-.413 1.4-2.397 0-2.81l-.34-.1a1.464 1.464 0 0 1-.872-2.105l.17-.31c.698-1.283-.705-2.686-1.987-1.987l-.311.169a1.464 1.464 0 0 1-2.105-.872l-.1-.34zM8 10.93a2.929 2.929 0 1 1 0-5.858 2.929 2.929 0 0 1 0 5.858z",
  },
];

// Active tab = current route name. v-model friendly: changing
// the route updates the tab highlight, clicking a tab updates the
// route.
const activeTab = computed<string | null>(() => {
  const name = route.name;
  return typeof name === "string" ? name : null;
});

function onTabChange(id: string): void {
  if (id === route.name) return;
  // v0.2+: both tabs map 1:1 to routes with matching names.
  router.push({ name: id });
}

// On tab close (× click). Pinned tabs ignore the event; we still
// receive it because the TabsBar emits for every closeable tab.
function onTabClose(_id: string): void {
  // v0.2 laipe: no closeable tabs. Reserved for v0.3+ per-conversation
  // tabs (each chat conversation opens as a closeable tab; the
  // chat-default tab itself stays pinned).
}

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
    <TabsBar
      :tabs="tabs"
      :model-value="activeTab"
      @update:model-value="onTabChange"
      @close="onTabClose"
    />

    <div class="app-body">
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
    </div>

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
  flex-direction: column;
  height: 100vh;
  background: #fafafa;
  position: relative;
}
.app-body {
  display: flex;
  flex: 1;
  min-height: 0;
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
