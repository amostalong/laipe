<script setup lang="ts">
// SettingsView — PlotCraft-style settings page (v0.2+).
//
// 1:1 alignment with PlotCraft's `SettingsView.vue`:
//   - 220px left sidebar with grouped navigation (LLM / General / AI)
//   - Right content area with switchable panels (no Save button —
//     all edits auto-save via v-model + watch, see PlotCraft v0.1.5+)
//   - Bottom action bar: Reset (confirms) + error state
//   - Route query params:
//       ?tab=api|tools|console|diagnostics → initial category
//       ?runId=<id>                        → console search filter
//
// "AI" group is reserved for v0.2+ AI tools permissioning; in v0.1
// (this file's first commit) the Tools entry is the only AI-group
// nav item, alongside Console and Diagnostics under General.
//
// Settings state: `useProviderConfig()` (laipe-app 自有, multi-provider PlotCraft 等价)
// + `agentSettings` (laipe-vue useConfig 的 agent 配置, 走 localStorage).
// diagnostic config 走 Rust 端 (getDiagnosticConfig / setDiagnosticConfig).

import { computed, onMounted, ref, watch } from "vue";
import { useRoute } from "vue-router";

import { useConfig } from "laipe-vue";
import { useProviderConfig } from "../composables/useProviderConfig";
import { TOOLS } from "../tools";
import ProviderPanel from "../components/settings/ProviderPanel.vue";
import ToolsPanel from "../components/settings/ToolsPanel.vue";
import ConsolePanel from "../components/settings/ConsolePanel.vue";
import DiagnosticsPanel from "../components/settings/DiagnosticsPanel.vue";

type Category = "api" | "tools" | "console" | "diagnostics";

const route = useRoute();

// v0.2+ multi-provider: Provider section 走 useProviderConfig (PlotCraft 等价)
// agentSettings 仍走 laipe-vue useConfig (agent config 跟 provider config 解耦)
const providerConfig = useProviderConfig()
const { agentSettings, reset: resetAgentSettings } = useConfig()

const validTabs: ReadonlySet<Category> = new Set<Category>([
  "api",
  "tools",
  "console",
  "diagnostics",
]);

// Active category — defaults to `api` (the most common landing).
// v0.2+ deep-link: if the route has `?tab=…`, sync from there.
const activeCategory = ref<Category>("api");
const consoleRunIdFilter = ref<string | null>(null);

function syncFromQuery(): void {
  const tab = route.query.tab;
  if (typeof tab === "string" && validTabs.has(tab as Category)) {
    activeCategory.value = tab as Category;
  }
  const runId = route.query.runId;
  consoleRunIdFilter.value = typeof runId === "string" ? runId : null;
}

onMounted(() => {
  syncFromQuery();
});

watch(() => route.query, () => {
  syncFromQuery();
});

const settingsError = ref<string | null>(null);

function onReset(): void {
  if (!window.confirm("Reset all settings to defaults? This does not clear your conversations.")) {
    return;
  }
  try {
    providerConfig.reset()
    resetAgentSettings()
    settingsError.value = null
  } catch (e) {
    settingsError.value = e instanceof Error ? e.message : String(e);
  }
}

// Sidebar groups. Each item's icon is a path string rendered into an
// inline SVG; we keep the icons as data here (no extra dep) rather
// than shipping lucide-vue-next. Path data is from Lucide's free
// icon set, MIT-licensed.
const sidebarGroups = computed(() => [
  {
    label: "LLM",
    items: [
      {
        id: "api" as Category,
        label: "Provider",
        iconPath:
          "M9 2v6h6V2H9zm6 8h-4V4h-2v6H5V4H3v6c0 2.2 1.8 4 4 4v6h2v-6h2v6h2v-6h2c2.2 0 4-1.8 4-4V4h-2v6z",
      },
    ],
  },
  {
    label: "General",
    items: [
      {
        id: "tools" as Category,
        label: "Tools",
        iconPath:
          "M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z",
      },
      {
        id: "console" as Category,
        label: "Console",
        iconPath:
          "M4 17l6-6-6-6h3l6 6-6 6H4zm9 0V11h2v6h-2z",
      },
    ],
  },
  {
    label: "Diagnostics",
    items: [
      {
        id: "diagnostics" as Category,
        label: "Errors & reports",
        iconPath:
          "M22 12h-4l-3 9L9 3l-3 9H2",
      },
    ],
  },
]);

function setActive(cat: Category): void {
  activeCategory.value = cat;
}
</script>

<template>
  <div class="settings-page">
    <!-- Sidebar (PlotCraft shape: 220px, grouped nav) -->
    <aside class="settings-sidebar">
      <header class="sidebar-header">
        <h2 class="sidebar-title">Settings</h2>
        <p class="sidebar-hint">
          v0.2+ all edits auto-save. No Save button.
        </p>
      </header>

      <div class="sidebar-nav">
        <template v-for="group in sidebarGroups" :key="group.label">
          <div class="sidebar-group-label">{{ group.label }}</div>
          <button
            v-for="item in group.items"
            :key="item.id"
            class="sidebar-item"
            :class="{ active: activeCategory === item.id }"
            @click="setActive(item.id)"
          >
            <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <path :d="item.iconPath" />
            </svg>
            <span>{{ item.label }}</span>
          </button>
        </template>
      </div>
    </aside>

    <!-- Content: switchable panel (Transition matches PlotCraft fade) -->
    <main class="settings-content">
      <Transition name="fade" mode="out-in">
        <ProviderPanel
          v-if="activeCategory === 'api'"
          key="api"
        />
        <ToolsPanel
          v-else-if="activeCategory === 'tools'"
          key="tools"
          :tools="TOOLS"
          :enabled-tools="agentSettings.enabledTools"
          :tool-permissions="agentSettings.toolPermissions"
          @update:enabled-tools="(next) => (agentSettings.enabledTools = next)"
          @update:tool-permissions="(next) => (agentSettings.toolPermissions = next)"
        />
        <ConsolePanel
          v-else-if="activeCategory === 'console'"
          key="console"
          :run-id-filter="consoleRunIdFilter"
        />
        <DiagnosticsPanel
          v-else-if="activeCategory === 'diagnostics'"
          key="diagnostics"
        />
      </Transition>

      <!-- Bottom action bar (PlotCraft pattern: Reset + error only,
           no Save button — auto-save via v-model) -->
      <div class="actions">
        <button class="reset-btn" @click="onReset">
          <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" />
            <path d="M3 3v5h5" />
          </svg>
          <span>Reset to defaults</span>
        </button>
        <div v-if="settingsError" class="error">
          <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <circle cx="12" cy="12" r="10" />
            <line x1="12" y1="8" x2="12" y2="12" />
            <line x1="12" y1="16" x2="12.01" y2="16" />
          </svg>
          <span>{{ settingsError }}</span>
        </div>
      </div>
    </main>
  </div>
</template>

<style scoped>
.settings-page {
  display: flex;
  height: 100%;
  background: var(--laipe-bg, #fafafa);
  color: var(--laipe-text, #1d1d1f);
}

/* --- Sidebar (PlotCraft shape) --- */
.settings-sidebar {
  width: 220px;
  flex-shrink: 0;
  background: var(--laipe-bg-elevated, #fafafa);
  border-right: 1px solid var(--laipe-border, #e5e5e7);
  overflow-y: auto;
  display: flex;
  flex-direction: column;
}
.sidebar-header {
  padding: 16px 16px 12px;
  border-bottom: 1px solid var(--laipe-border, #e5e5e7);
  margin-bottom: 8px;
}
.sidebar-title {
  margin: 0 0 4px 0;
  font-size: 1.05em;
  font-weight: 600;
  color: var(--laipe-text, #1d1d1f);
}
.sidebar-hint {
  margin: 0;
  font-size: 0.75em;
  color: var(--laipe-text-muted, #a1a1a6);
  line-height: 1.4;
}
.sidebar-nav {
  display: flex;
  flex-direction: column;
  gap: 1px;
  padding-bottom: 16px;
}
.sidebar-group-label {
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  color: var(--laipe-text-muted, #a1a1a6);
  padding: 12px 16px 6px;
  opacity: 0.85;
}
.sidebar-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 16px;
  background: transparent;
  color: var(--laipe-text-muted, #6e6e73);
  border: none;
  border-left: 2px solid transparent;
  cursor: pointer;
  font-size: 13px;
  font-family: inherit;
  text-align: left;
  transition: all 0.12s;
}
.sidebar-item:hover {
  background: var(--laipe-border, #e5e5e7);
  color: var(--laipe-text, #1d1d1f);
}
.sidebar-item.active {
  background: rgba(0, 122, 255, 0.08);
  color: var(--laipe-accent, #007aff);
  border-left-color: var(--laipe-accent, #007aff);
  font-weight: 500;
}

/* --- Content --- */
.settings-content {
  flex: 1;
  overflow-y: auto;
  padding: 24px 32px;
  min-width: 0;
  display: flex;
  flex-direction: column;
}
.actions {
  display: flex;
  gap: 8px;
  align-items: center;
  margin-top: auto;
  padding-top: 24px;
  border-top: 1px solid var(--laipe-border, #e5e5e7);
}
.reset-btn {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  background: transparent;
  color: var(--laipe-text-muted, #6e6e73);
  border: 1px solid var(--laipe-border, #d2d2d7);
  border-radius: 6px;
  cursor: pointer;
  font-size: 0.82em;
  font-family: inherit;
}
.reset-btn:hover {
  background: var(--laipe-border, #e5e5e7);
  color: var(--laipe-text, #1d1d1f);
}
.error {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 0.82em;
  color: #ff3b30;
}

/* --- Panel transition (PlotCraft fade) --- */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.12s;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
</style>
