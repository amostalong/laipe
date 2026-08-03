// useProviderConfig — laipe-app 的 multi-provider LLM 配置 (PlotCraft-equivalent)
//
// v0.2+ 替代 laipe-vue 的 useConfig —— 单 ProviderConfig 不够 PlotCraft 那种
// 多 provider UX. laipe-app 自有 schema (跟 FinaBoard 镜像):
// - persisted: ProviderStore { custom_providers[], active_provider_id }
// - runtime:    customProviders ref / activeProviderId ref / activeConfig computed
// - persistence: localStorage (laipe-app 是 Vue+Vite 浏览器模式, 没 Tauri Rust 端)
//
// 保留 laipe-vue useConfig 的 API 表面 (config / agentSettings / reset) 让 laipe-app
// 现有 MainView / SettingsView / ProviderPanel / ToolsPanel 改动最小.

import { ref, computed, watch } from "vue";
import type { ProviderConfig } from "laipe-ts";
import type { AgentSettings } from "laipe-vue";

// 单个 provider (跟 FinaBoard 镜像, app-level schema, 跟 laipe_core::ProviderConfig 区分)
export interface CustomProvider {
  id: string;
  name: string;
  endpoint: string;
  api_key: string;
  model: string;
  api_format: string; // "openai_chat" | "openai_responses" | "anthropic"
  enabled: boolean;
  effort: string | null;
  max_tokens: number | null;
  temperature: number | null;
  default_model: string | null;
  models: ProviderModel[];
}

export interface ProviderModel {
  id: string;
  name: string;
}

interface ProviderStore {
  custom_providers: CustomProvider[];
  active_provider_id: string | null;
  /** laipe-vue useConfig 的 agentSettings (per-tool enabled map) —— 也走 localStorage */
  agent_settings: AgentSettings;
}

const STORAGE_KEY = "laipe.providers.v1";

const DEFAULT_AGENT_SETTINGS: AgentSettings = { enabledTools: {} };

// === Singleton state (跟 laipe-vue useConfig 同样模式) ===
const providers = ref<CustomProvider[]>([]);
const activeProviderId = ref<string | null>(null);
const agentSettings = ref<AgentSettings>({ ...DEFAULT_AGENT_SETTINGS });
const loaded = ref(false);

function newProviderId(): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }
  return `p_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 10)}`;
}

function loadFromStorage(): ProviderStore {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw) as Partial<ProviderStore>;
      return {
        custom_providers: parsed.custom_providers ?? [],
        active_provider_id: parsed.active_provider_id ?? null,
        agent_settings: { ...DEFAULT_AGENT_SETTINGS, ...(parsed.agent_settings ?? {}) },
      };
    }
  } catch {
    /* corrupted — fall through */
  }
  return { custom_providers: [], active_provider_id: null, agent_settings: { ...DEFAULT_AGENT_SETTINGS } };
}

function persist(): void {
  try {
    const snapshot: ProviderStore = {
      custom_providers: providers.value.map((p) => ({ ...p })),
      active_provider_id: activeProviderId.value,
      agent_settings: { ...agentSettings.value },
    };
    localStorage.setItem(STORAGE_KEY, JSON.stringify(snapshot));
  } catch {
    /* private mode / quota exceeded — silently drop */
  }
}

// 任何字段改动都同步到 localStorage
watch(
  [providers, activeProviderId, agentSettings],
  () => {
    if (loaded.value) persist();
  },
  { deep: true },
);

function init(): void {
  if (loaded.value) return;
  const state = loadFromStorage();
  providers.value = state.custom_providers;
  activeProviderId.value = state.active_provider_id;
  agentSettings.value = state.agent_settings;
  pickActive();
  loaded.value = true;
}

function pickActive(): void {
  const cur = providers.value.find((p) => p.id === activeProviderId.value);
  if (cur && cur.enabled) return;
  const firstEnabled = providers.value.find((p) => p.enabled);
  activeProviderId.value = firstEnabled?.id ?? null;
}

function toLaipeConfig(p: CustomProvider): ProviderConfig {
  return {
    endpoint: p.endpoint,
    api_key: p.api_key,
    model: p.default_model?.trim() || p.model,
    api_format: p.api_format as ProviderConfig["api_format"],
    effort: (p.effort as ProviderConfig["effort"]) ?? undefined,
    max_tokens: p.max_tokens ?? undefined,
    temperature: p.temperature ?? undefined,
  };
}

export function useProviderConfig() {
  init();

  const activeProvider = computed<CustomProvider | null>(() => {
    const id = activeProviderId.value;
    if (!id) return null;
    return providers.value.find((p) => p.id === id) ?? null;
  });

  /** active provider 转 laipe ProviderConfig (给 useChat / tauriStream 接 cfg) */
  const config = computed<ProviderConfig | null>(() => {
    const p = activeProvider.value;
    return p ? toLaipeConfig(p) : null;
  });

  function setActive(id: string): void {
    const target = providers.value.find((p) => p.id === id);
    if (target && target.enabled) activeProviderId.value = id;
  }

  function add(init?: Partial<CustomProvider>): CustomProvider {
    const id = newProviderId();
    const blank: Omit<CustomProvider, "id"> = {
      name: "New provider",
      endpoint: "https://api.openai.com/v1",
      api_key: "",
      model: "gpt-4o-mini",
      api_format: "openai_chat",
      enabled: true,
      effort: null,
      max_tokens: null,
      temperature: null,
      default_model: null,
      models: [],
    };
    const provider: CustomProvider = { ...blank, ...init, id };
    providers.value.push(provider);
    if (!activeProviderId.value) activeProviderId.value = id;
    return provider;
  }

  function remove(id: string): void {
    const idx = providers.value.findIndex((p) => p.id === id);
    if (idx === -1) return;
    providers.value.splice(idx, 1);
    if (activeProviderId.value === id) pickActive();
  }

  function update(id: string, patch: Partial<CustomProvider>): void {
    const p = providers.value.find((p) => p.id === id);
    if (!p) return;
    Object.assign(p, patch);
  }

  function toggleEnabled(id: string): void {
    const p = providers.value.find((p) => p.id === id);
    if (!p) return;
    p.enabled = !p.enabled;
    if (!p.enabled && activeProviderId.value === id) activeProviderId.value = null;
  }

  function reset(): void {
    providers.value = [];
    activeProviderId.value = null;
    agentSettings.value = { ...DEFAULT_AGENT_SETTINGS };
  }

  return {
    // state
    providers,
    activeProviderId,
    activeProvider,
    config,
    agentSettings,
    loaded,
    // actions
    init,
    setActive,
    pickActive,
    add,
    remove,
    update,
    toggleEnabled,
    reset,
  };
}
