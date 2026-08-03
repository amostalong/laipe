// Tab system — base "open documents" abstraction for laipe-vue.
//
// A laipe app may have multiple top-level views (chat, settings,
// project workspace, etc.). The tab system gives the user a
// VSCode/browser-style "switch between open things" UI at the top of
// the app. Each tab is identified by a string id; the app maps
// tab id ↔ route (or whatever view mechanism it uses).
//
// Pluggability
// ============
//
// `Tab` is a plain interface; `useTabs` is a Vue composable that
// returns a reactive list of open tabs + the active id. The host
// app decides what view each tab maps to (route, v-if, custom
// component tree, etc.) — `useTabs` is view-agnostic.
//
// Why a composable, not a Pinia store
// ====================================
//
// Tabs are session-scoped (lost on reload) and per-app. Pinia is
// overkill for one list. The composable returns module-singleton
// state so all components calling `useTabs()` see the same tabs —
// matches the existing pattern (`useChat`, `useConversations`).
//
// Why a "base" system
// ===================
//
// v0.2 laipe ships 2 tabs (Chat, Settings). Future forks might have
// a workspace tab per project, a debug tab per session, etc. The
// composable + TabsBar component scale to N tabs without changes.

import { computed, ref, type Ref } from "vue";

/** A single open tab. */
export interface Tab {
  /** Stable id. Maps 1:1 to whatever view mechanism the host uses
   * (a route name, a v-if branch, etc.). */
  id: string;
  /** Display title. Shown in the tab + as the page heading. */
  title: string;
  /** Optional inline-SVG path for the tab icon. The TabsBar wraps
   * it in a 14×14 viewBox; consumers can pass a full SVG if they
   * need a different size. */
  iconPath?: string;
  /** When true, the tab cannot be closed (no × button). Use for
   * shell tabs (Settings, Home). Default: true. */
  pinned?: boolean;
  /** Whether the close (×) button is shown. Default: !pinned. */
  closeable?: boolean;
  /** Optional small numeric badge (e.g. unread count). Shown as a
   * pill in the tab; absent or 0 → no badge. */
  badge?: number;
  /** Tooltip when hovering. Defaults to `title`. */
  tooltip?: string;
}

/** The shared state returned by `useTabs()`. */
export interface TabsState {
  tabs: Ref<Tab[]>;
  activeId: Ref<string | null>;
  open(tab: Tab): void;
  close(id: string): void;
  activate(id: string): void;
  /** True if the given tab id is open (regardless of active state). */
  has(id: string): boolean;
  /** The active tab object (or null if no tabs are open). */
  active: Ref<Tab | null>;
}

// Module-level singleton. laipe-vue uses the same pattern for
// useChat / useConversations — module singletons avoid the boilerplate
// of a Pinia store for one global list.
const tabs: Ref<Tab[]> = ref([]);
const activeId: Ref<string | null> = ref(null);

export function useTabs(): TabsState {
  /** Register a tab. If already open, just activates it. */
  function open(tab: Tab): void {
    const existing = tabs.value.findIndex((t) => t.id === tab.id);
    if (existing >= 0) {
      // Update title/icon if changed (the host may rebuild the tab
      // object on each render; we want the latest values).
      tabs.value[existing] = { ...tabs.value[existing], ...tab };
    } else {
      tabs.value.push(tab);
    }
    activeId.value = tab.id;
  }

  /** Close a tab. Refuses to close pinned tabs. After close, if
   * the closed tab was active, activate the next one. */
  function close(id: string): void {
    const idx = tabs.value.findIndex((t) => t.id === id);
    if (idx < 0) return;
    const tab = tabs.value[idx];
    if (!tab || tab.pinned) return;
    tabs.value.splice(idx, 1);
    if (activeId.value === id) {
      // Activate the tab to the left, or the first remaining.
      const next = tabs.value[idx] ?? tabs.value[idx - 1] ?? null;
      activeId.value = next ? next.id : null;
    }
  }

  function activate(id: string): void {
    if (tabs.value.some((t) => t.id === id)) {
      activeId.value = id;
    }
  }

  function has(id: string): boolean {
    return tabs.value.some((t) => t.id === id);
  }

  const active = computed<Tab | null>(() => {
    if (activeId.value === null) return null;
    return tabs.value.find((t) => t.id === activeId.value) ?? null;
  });

  return { tabs, activeId, open, close, activate, has, active };
}

/** Reset all tab state. Test-only. */
export function __resetTabsForTests(): void {
  tabs.value = [];
  activeId.value = null;
}
