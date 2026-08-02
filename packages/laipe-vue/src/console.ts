// Debug console — frontend library.
//
// Mirrors the Rust `console.rs` in apps that have a Tauri backend
// (laipe-app). When running browser-only (no Tauri), all features
// still work — the `console:entry` listener and `get_console_entries`
// invoke just no-op / fail silently. Frontend `console.log/warn/error`
// hooks continue to populate the panel.
//
// ## Diagnostic context (v0.2+)
//
// `ConsoleEntry` now carries optional diagnostic fields (`conversation_id`,
// `turn`, `kind`, `request_digest`, `response_digest`, `cause`). The
// chat command fills these when surfacing an LLM error, and the
// `saveReport()` API uses them to locate the on-disk recording and
// synthesize a self-contained `.md` report for an LLM assistant.
//
// Usage:
//   import { initConsole, useConsoleEntries, clearConsole, installConsoleHook, saveReport } from "laipe-vue";
//
//   // In main.ts (once):
//   installConsoleHook();
//   await initConsole();
//
//   // In a component:
//   const entries = useConsoleEntries();
//   entries.value.forEach(...)
//   await saveReport(entries.value[0].id);   // writes <log_dir>/reports/<ts>-<id>.md

import { ref, type Ref } from "vue";
import type { UnlistenFn } from "@tauri-apps/api/event";

export type ConsoleLevel = "info" | "warn" | "error";
export type ConsoleSource = "backend" | "frontend";

/** 8-way error classification. Mirrors `laipe_core::ChatErrorKind`. */
export type ChatErrorKind =
  | "network"
  | "auth"
  | "model_not_found"
  | "bad_request"
  | "rate_limit"
  | "server_error"
  | "stream_protocol"
  | "unknown";

/** Single console entry. Mirrors Rust `ConsoleEntry` (snake_case via serde). */
export interface ConsoleEntry {
  id: string;
  level: ConsoleLevel;
  source: ConsoleSource;
  module: string;
  message: string;
  /** Milliseconds since epoch (snake_case to match Rust). */
  timestamp_ms: number;
  // === Diagnostic context (all optional) ===
  /** Conversation id the entry belongs to. `undefined` for app-wide logs. */
  conversation_id?: string;
  /** Agent-loop turn (0-based). `undefined` outside an agent turn. */
  turn?: number;
  /** Typed error class. Drives the saved report's "Likely causes" section. */
  kind?: ChatErrorKind;
  /** One-line summary of the outgoing request. */
  request_digest?: string;
  /** One-line summary of the incoming response (or error). */
  response_digest?: string;
  /** The original lower-level error string (reqwest / serde / etc.). */
  cause?: string;
}

/** Result of `saveReport`. `path` is relative to the app log dir. */
export interface SavedReport {
  path: string;
}

const MAX_ENTRIES = 1000;

// === Module-level singleton state ===
//
// Console is a single stream — all views share the same data. Module-level
// ref makes the UI auto-reactive without needing a Pinia store.
const entries: Ref<ConsoleEntry[]> = ref([]);
let initialSnapshotLoaded = false;
let unsubscribeEvent: UnlistenFn | null = null;
let consoleHookInstalled = false;

function pushLocal(entry: ConsoleEntry): void {
  // Dedup by id (defends against the listen + snapshot race).
  if (entries.value.some((e) => e.id === entry.id)) return;
  entries.value = [entry, ...entries.value];
  if (entries.value.length > MAX_ENTRIES) {
    entries.value = entries.value.slice(0, MAX_ENTRIES);
  }
}

function formatArgs(args: unknown[]): string {
  return args
    .map((a) => {
      if (typeof a === "string") return a;
      if (a instanceof Error) return a.stack || a.message;
      try {
        return JSON.stringify(a, null, 2);
      } catch {
        return String(a);
      }
    })
    .join(" ");
}

function pushFrontend(level: ConsoleLevel, args: unknown[]): void {
  pushLocal({
    id: `fe-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
    level,
    source: "frontend",
    module: "app",
    message: formatArgs(args),
    timestamp_ms: Date.now(),
  });
}

async function loadInitialSnapshot(): Promise<void> {
  if (initialSnapshotLoaded) return;
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    const fromBackend = await invoke<ConsoleEntry[]>("get_console_entries");
    for (const e of fromBackend) pushLocal(e);
    initialSnapshotLoaded = true;
  } catch {
    // No Tauri runtime, or command not registered. Silent.
  }
}

async function subscribeBackendEvents(): Promise<void> {
  if (unsubscribeEvent) return;
  try {
    const { listen } = await import("@tauri-apps/api/event");
    unsubscribeEvent = await listen<ConsoleEntry>("console:entry", (e) => {
      pushLocal(e.payload);
    });
  } catch {
    // No Tauri runtime. Silent.
  }
}

/**
 * Hook `console.log` / `console.warn` / `console.error` so every
 * frontend log line also lands in the debug console. Install once
 * (idempotent) at app startup.
 */
export function installConsoleHook(): void {
  if (consoleHookInstalled) return;
  consoleHookInstalled = true;

  const origLog = console.log.bind(console);
  const origWarn = console.warn.bind(console);
  const origError = console.error.bind(console);

  console.log = (...args: unknown[]) => {
    origLog(...args);
    pushFrontend("info", args);
  };
  console.warn = (...args: unknown[]) => {
    origWarn(...args);
    pushFrontend("warn", args);
  };
  console.error = (...args: unknown[]) => {
    origError(...args);
    pushFrontend("error", args);
  };
}

/** Pull snapshot + subscribe to backend events. */
export async function initConsole(): Promise<void> {
  await Promise.all([loadInitialSnapshot(), subscribeBackendEvents()]);
}

/** Clear all console entries (frontend + backend). */
export async function clearConsole(): Promise<void> {
  entries.value = [];
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    await invoke("clear_console");
  } catch {
    // No Tauri runtime. Frontend-only clear already happened.
  }
}

/** Reactive ref to the entries list. Read this in your console UI. */
export function useConsoleEntries(): Ref<ConsoleEntry[]> {
  return entries;
}

/** Force reload of the backend snapshot (UI "Refresh" button). */
export async function refreshConsole(): Promise<void> {
  initialSnapshotLoaded = false;
  await loadInitialSnapshot();
}

/**
 * Synthesize a self-contained `.md` error report for one console
 * entry. Backend locates the on-disk recording by
 * `(conversation_id, turn)`, assembles the YAML-frontmatter `.md`,
 * and appends a one-line entry to `INDEX.jsonl`.
 *
 * Returns the path to the report (relative to the app log dir).
 * Throws if the entry has no matching recording (e.g. the entry
 * was emitted before diagnostics initialized, or the user manually
 * cleared the recordings dir).
 *
 * Usage from a Vue component:
 * ```ts
 * import { saveReport } from "laipe-vue";
 * const path = await saveReport(entry.id);
 * showToast(`Report saved: ${path}`);
 * ```
 */
export async function saveReport(consoleId: string): Promise<SavedReport> {
  const { invoke } = await import("@tauri-apps/api/core");
  const path = await invoke<string>("dump_error_report", {
    consoleId,
  });
  return { path };
}

/**
 * Diagnostic mode config. Mirrors the Rust `DiagnosticConfig` in
 * `laipe-app/src-tauri/src/diagnostics.rs`. Frontend apps expose
 * these as toggles in Settings; the backend uses them to decide
 * whether to record every round / auto-snapshot errors.
 */
export interface DiagnosticConfig {
  /** Auto-snapshot every error to a `.md` report (off by default). */
  auto_snapshot: boolean;
  /** Max bytes per saved report (response is truncated beyond this). */
  max_report_bytes: number;
  /** Record every chat round, not just failures (off by default). */
  record_successful_rounds: boolean;
}

/** Get the current diagnostic config. */
export async function getDiagnosticConfig(): Promise<DiagnosticConfig> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<DiagnosticConfig>("get_diagnostic_mode");
}

/** Set the diagnostic config. Pass the full object — partial updates
 * are not supported. */
export async function setDiagnosticConfig(cfg: DiagnosticConfig): Promise<void> {
  const { invoke } = await import("@tauri-apps/api/core");
  await invoke("set_diagnostic_mode", { cfg });
}
