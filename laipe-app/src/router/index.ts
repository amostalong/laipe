// laipe-app router — minimal 2-route setup
//
// v0.2+: settings became its own route (was a modal pre-v0.2).
// PlotCraft-shape: `/` is the chat, `/settings` is the full settings
// page. The conversation Sidebar only shows on `/`.
//
// History queries on `/settings`:
//   ?tab=api|tools|console|diagnostics → initial category
//   ?runId=<id>                        → console panel search filter
//
// v0.2+ diagnostic flow: chat error fires `console:entry` (kind set)
// → user clicks "查看详情" → router.push('/settings?tab=diagnostics&runId=…')
// → Diagnostics panel auto-opens and Console panel's search filter is
// set to the failing run id. The user can then click the entry's
// "save" button to write a .md report.
//
// We don't import vue-router as a hard dep elsewhere; this file is
// the only place it lives. If you need more routes later, add them
// here.

import { createRouter, createWebHashHistory, type RouteRecordRaw } from "vue-router";
import MainView from "../views/MainView.vue";
import SettingsView from "../views/SettingsView.vue";

const routes: RouteRecordRaw[] = [
  {
    path: "/",
    name: "main",
    component: MainView,
  },
  {
    path: "/settings",
    name: "settings",
    component: SettingsView,
    // Persist the active category + any runId filter in the URL so a
    // chat error's "查看详情" link can deep-link straight to the right
    // panel + filter.
    props: (route) => ({
      initialTab: typeof route.query.tab === "string" ? route.query.tab : undefined,
      consoleRunIdFilter:
        typeof route.query.runId === "string" ? route.query.runId : null,
    }),
  },
];

export const router = createRouter({
  // Use hash history for Tauri 2 compatibility (Tauri's webview serves
  // from `tauri://localhost` which doesn't support SPA pushState
  // fallback routes — hash mode is the safe default for desktop apps).
  history: createWebHashHistory(),
  routes,
});

export type AppRouteName = "main" | "settings";
