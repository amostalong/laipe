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
import type { ProviderConfig, ToolPermission } from "laipe-ts";

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
  /**
   * Per-tool execution permission. Tools not in this map default to
   * `"auto"` (run immediately). Set to `"ask"` to gate behind user
   * approval, or `"deny"` to refuse the call and tell the LLM the
   * user rejected it.
   */
  toolPermissions: Record<string, ToolPermission>;
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
  return { enabledTools: {}, toolPermissions: {} };
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

/** Load provider + agent settings from a storage backend into the refs. */
async function loadFromStorage(s: ConfigStorage): Promise<void> {
  const c = await s.loadProviderConfig();
  if (c) config.value = c;
  const a = await s.loadAgentSettings();
  if (a) agentSettings.value = a;
}

/**
 * Initial load runs once at module init against the default storage
 * (localStorage), so saved settings come back on every boot without
 * any app wiring — same pattern as useConversations. Apps that swap
 * the backend via setConfigStorage() get a second load against the
 * new storage, and its data wins (scheduled after this one).
 */
let storageReady: Promise<void> = loadFromStorage(currentStorage);

/** Replace the storage backend. Loads from the new storage if it has data. */
export function setConfigStorage(s: ConfigStorage): void {
  currentStorage = s;
  storageReady = loadFromStorage(s);
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

/**
 * Resolve a tool's permission from the user-facing settings map.
 * Missing entries default to `"auto"`. Never throws; the contract is
 * "always returns a valid permission".
 */
export function resolveToolPermission(
  settings: AgentSettings,
  toolName: string,
): ToolPermission {
  return settings.toolPermissions[toolName] ?? "auto";
}

/** Convenience: is the tool allowed to run at all? `deny` = no. */
export function isToolAllowed(
  settings: AgentSettings,
  toolName: string,
): boolean {
  return resolveToolPermission(settings, toolName) !== "deny";
}
