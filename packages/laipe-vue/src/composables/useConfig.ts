// useConfig — single ProviderConfig, persisted to a swappable storage adapter.
//
// Pluggable design (see AGENTS.md "Pluggability"):
//   - `ConfigStorage` interface — load/save provider + agent settings
//   - Default impl uses `localStorage` (synchronous, ships with the starter)
//   - Apps can swap in any storage (Tauri command, SQLite, server, etc.)
//     by calling `setConfigStorage(s)` once at app startup.
//
// One global config for the whole app; new conversations inherit it.
// `agentSettings` (per-tool enable map, etc.) lives in a separate slot
// so `ProviderConfig` stays a pure mirror of the Rust struct.

import { ref, watch } from "vue";
import type { ProviderConfig } from "laipe-ts";

const STORAGE_KEY_CONFIG = "laipe.config.v1";
const STORAGE_KEY_AGENT = "laipe.agent.v1";

/** Pluggable persistence layer. Methods may be sync (localStorage) or async (Tauri). */
export interface ConfigStorage {
  /** Load the ProviderConfig or return null if nothing is stored. */
  loadProviderConfig(): ProviderConfig | null | Promise<ProviderConfig | null>;
  /** Persist the ProviderConfig. */
  saveProviderConfig(c: ProviderConfig): void | Promise<void>;
  /** Load the AgentSettings or return null if nothing is stored. */
  loadAgentSettings(): AgentSettings | null | Promise<AgentSettings | null>;
  /** Persist the AgentSettings. */
  saveAgentSettings(s: AgentSettings): void | Promise<void>;
}

/** Per-agent settings that don't fit in ProviderConfig. */
export interface AgentSettings {
  /** Per-tool enabled state. Tools not in this map default to true. */
  enabledTools: Record<string, boolean>;
}

function defaultConfig(): ProviderConfig {
  return {
    endpoint: "https://api.openai.com/v1",
    api_key: "",
    model: "gpt-4o-mini",
    api_format: "openai_chat",
  };
}

function defaultAgentSettings(): AgentSettings {
  return { enabledTools: {} };
}

/** Default storage: localStorage (sync). Ships with the starter. */
export const localStorageConfig: ConfigStorage = {
  loadProviderConfig() {
    try {
      const raw = localStorage.getItem(STORAGE_KEY_CONFIG);
      if (raw) {
        const saved = JSON.parse(raw) as Partial<ProviderConfig>;
        // Merge with defaults so old / partial saved configs still work.
        return { ...defaultConfig(), ...saved };
      }
    } catch {
      /* corrupted storage — fall through */
    }
    return null;
  },
  saveProviderConfig(c) {
    try {
      localStorage.setItem(STORAGE_KEY_CONFIG, JSON.stringify(c));
    } catch {
      /* private mode */
    }
  },
  loadAgentSettings() {
    try {
      const raw = localStorage.getItem(STORAGE_KEY_AGENT);
      if (raw) {
        const saved = JSON.parse(raw) as Partial<AgentSettings>;
        return { ...defaultAgentSettings(), ...saved };
      }
    } catch {
      /* corrupted storage — fall through */
    }
    return null;
  },
  saveAgentSettings(s) {
    try {
      localStorage.setItem(STORAGE_KEY_AGENT, JSON.stringify(s));
    } catch {
      /* private mode */
    }
  },
};

const config = ref<ProviderConfig>(defaultConfig());
const agentSettings = ref<AgentSettings>(defaultAgentSettings());
let currentStorage: ConfigStorage = localStorageConfig;
let storageReady: Promise<void> = Promise.resolve();

/** Replace the storage backend. Loads from the new storage if it has data. */
export function setConfigStorage(s: ConfigStorage): void {
  currentStorage = s;
  storageReady = (async () => {
    const c = await s.loadProviderConfig();
    if (c) config.value = c;
    const a = await s.loadAgentSettings();
    if (a) agentSettings.value = a;
  })();
}

/** Wait for the initial storage load to complete (no-op for sync storages). */
export function whenConfigReady(): Promise<void> {
  return storageReady;
}

watch(
  config,
  (v) => {
    void currentStorage.saveProviderConfig(v);
  },
  { deep: true },
);

watch(
  agentSettings,
  (v) => {
    void currentStorage.saveAgentSettings(v);
  },
  { deep: true },
);

export function useConfig() {
  function update(patch: Partial<ProviderConfig>): void {
    config.value = { ...config.value, ...patch };
  }
  function reset(): void {
    config.value = defaultConfig();
  }
  function isReady(): boolean {
    return config.value.api_key.trim().length > 0;
  }
  return {
    config,
    update,
    reset,
    isReady,
    agentSettings,
  };
}
