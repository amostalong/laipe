// useProviderConfig — laipe-app 的 multi-provider LLM 配置 (PlotCraft-equivalent)
//
// v0.2+ 替代 laipe-vue 的 useConfig —— 单 ProviderConfig 不够 PlotCraft 那种
// 多 provider UX. laipe-app 自有 schema (跟 PlotCraft `CustomProvider` 镜像):
// - persisted: ProviderStore { customProviders[], activeProviderId }
// - runtime:    customProviders ref / activeProviderId ref / activeConfig computed
// - persistence: localStorage (laipe-app 是 Vue+Vite 浏览器模式, 没 Tauri Rust 端)
//
// 保留 laipe-vue useConfig 的 API 表面 (config / agentSettings / reset) 让 laipe-app
// 现有 MainView / SettingsView / ProviderPanel / ToolsPanel 改动最小.
//
// v0.2+ 字段对齐 PlotCraft `lib/settings.ts` `CustomProvider`:
// - id / name / baseUrl / apiKey / apiFormat / enabled / models[] / defaultModel
// - (PlotCraft 字段 camelCase; laipe-app 也用 camelCase 跟它互通)
// - apiFormat 字面值: "openai_chat" | "openai_responses" | "anthropic_messages"
// - effort: "none" | "low" | "medium" | "high" | "xhigh" | "max" (per-run override)

import { ref, computed, watch } from "vue";
import type { ProviderConfig, ApiFormat, EffortLevel } from "laipe-ts";
import type { AgentSettings } from "laipe-vue";

/** 单个 provider (镜像 PlotCraft `CustomProvider` schema, camelCase) */
export interface CustomProvider {
  id: string;
  name: string;
  baseUrl: string;
  apiKey: string;
  apiFormat: ApiFormat;
  enabled: boolean;
  /** 该 provider 下的 model 列表（v0.2+ 跟 PlotCraft 同款多 model） */
  models: ProviderModel[];
  /** 该 provider 发请求时用的默认 model id */
  defaultModel: string;
}

export interface ProviderModel {
  id: string;
  name: string;
}

/**
 * v0.2+ 旧盘兼容（v0.1 snake_case → v0.2 camelCase 迁移）
 *
 * 老 v0.1 schema (snake_case):
 *   { custom_providers: [{ endpoint, api_key, default_model, ... }],
 *     active_provider_id, agent_settings }
 *
 * 新 v0.2 schema (camelCase, 镜像 PlotCraft):
 *   { customProviders: [{ baseUrl, apiKey, defaultModel, ... }],
 *     activeProviderId, agentSettings }
 *
 * `STORAGE_KEY_V1` 标记 v0.1, 读时检测到 snake_case 走迁移路径 → 写 v0.2.
 * 老 key 不覆盖（玩家可手动清 localStorage 强制重置）。
 */
export interface ProviderStoreV1 {
  custom_providers: Array<{
    id: string;
    name: string;
    endpoint: string;
    api_key: string;
    model: string;
    api_format: string;
    enabled: boolean;
    effort: string | null;
    max_tokens: number | null;
    temperature: number | null;
    default_model: string | null;
    models?: ProviderModel[];
  }>;
  active_provider_id: string | null;
  agent_settings: AgentSettings;
}

interface ProviderStore {
  customProviders: CustomProvider[];
  activeProviderId: string | null;
  /** laipe-vue useConfig 的 agentSettings (per-tool enabled map) —— 也走 localStorage */
  agentSettings: AgentSettings;
}

const STORAGE_KEY = "laipe.providers.v2";
const STORAGE_KEY_V1 = "laipe.providers.v1";

const DEFAULT_AGENT_SETTINGS: AgentSettings = {
  enabledTools: {},
  toolPermissions: {},
};

function v1ToV2Provider(p: ProviderStoreV1["custom_providers"][number]): CustomProvider {
  return {
    id: p.id,
    name: p.name,
    baseUrl: p.endpoint,
    apiKey: p.api_key,
    apiFormat: (p.api_format as ApiFormat) ?? "openai_chat",
    enabled: p.enabled,
    models: p.models ?? [],
    defaultModel: p.default_model ?? p.model ?? "",
  };
}

function migrateV1(v1: ProviderStoreV1): ProviderStore {
  return {
    customProviders: v1.custom_providers.map(v1ToV2Provider),
    activeProviderId: v1.active_provider_id,
    agentSettings: v1.agent_settings,
  };
}

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
  // 优先 v0.2 新 key
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw) as Partial<ProviderStore>;
      return {
        customProviders: parsed.customProviders ?? [],
        activeProviderId: parsed.activeProviderId ?? null,
        agentSettings: {
          ...DEFAULT_AGENT_SETTINGS,
          ...(parsed.agentSettings ?? {}),
        },
      };
    }
  } catch {
    /* corrupted — fall through to v1 migration */
  }
  // v0.2+ 自动迁移 v1 → v2
  try {
    const v1raw = localStorage.getItem(STORAGE_KEY_V1);
    if (v1raw) {
      const v1 = JSON.parse(v1raw) as ProviderStoreV1;
      const migrated = migrateV1(v1);
      // 立即把迁移结果写回 v2 key
      try {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(migrated));
        // 保留 v1 作为兜底（玩家可手动清），但同时把 v2 设为权威
      } catch {
        /* quota / private mode — silently drop */
      }
      return migrated;
    }
  } catch {
    /* corrupted v1 — fall through */
  }
  return {
    customProviders: [],
    activeProviderId: null,
    agentSettings: { ...DEFAULT_AGENT_SETTINGS },
  };
}

function persist(): void {
  try {
    const snapshot: ProviderStore = {
      customProviders: providers.value.map((p) => ({ ...p })),
      activeProviderId: activeProviderId.value,
      agentSettings: { ...agentSettings.value },
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
  providers.value = state.customProviders;
  activeProviderId.value = state.activeProviderId;
  agentSettings.value = state.agentSettings;
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
    endpoint: p.baseUrl,
    api_key: p.apiKey,
    model: p.defaultModel?.trim() || "",
    api_format: p.apiFormat,
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
      baseUrl: "https://api.openai.com/v1",
      apiKey: "",
      apiFormat: "openai_chat",
      enabled: true,
      models: [],
      defaultModel: "",
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
