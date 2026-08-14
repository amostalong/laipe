// useToolApprovals — bridges the chat UI and the Rust agent loop for
// `permission = "ask"` tools.
//
// What this does
// ==============
//
// When the Rust backend encounters a tool call whose permission is
// `"ask"`, it emits `chat:tool_needs_approval` and pauses the agent
// loop, waiting for the user to decide. Two Tauri commands —
// `approve_tool` and `deny_tool` — unblock the waiter; the backend
// then runs the tool (or refuses) and emits `chat:tool_result` with
// the outcome so the frontend can render the result in the matching
// `ToolCallCard`.
//
// This composable wraps both directions:
//
//   - Approve / Deny buttons call `approve(callId)` / `deny(callId)`,
//     which fire the Tauri commands.
//   - `onResult` is invoked when the backend's `chat:tool_result`
//     event arrives, so the host can update the call's `status`,
//     `result`, and `error` in the assistant message.
//
// Lifecycle
// ---------
//
// The composable is meant to live for the lifetime of one chat
// session (mount/unmount via Vue). It registers one Tauri event
// listener on first use and tears it down on unmount. Per-call state
// is held in a Map<callId, AssistantToolCall> so the result handler
// can find the right call to update.
//
// Usage (in MainView):
// ```ts
// const approvals = useToolApprovals();
// // On user click:
// approvals.approve(call.id);
// approvals.deny(call.id);
// // When the streaming assistant message gains a new tool call:
// approvals.trackCall(call);
// ```

import { onUnmounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AssistantToolCall } from "laipe-ts";

/**
 * Payload of the `chat:tool_result` Tauri event the Rust backend emits
 * after each tool call finishes (approved, denied, or auto-run).
 */
export interface ToolResultEvent {
  /** The LLM-assigned tool call id (matches `AssistantToolCall.id`). */
  call_id: string;
  /** The tool's name (echoed for convenience / debug). */
  name: string;
  /**
   * The final JSON result string the tool returned. Always present —
   * even on denial we synthesize `{"error": "user_denied", ...}` so
   * the frontend has something to render and the LLM gets feedback.
   */
  result: string;
  /**
   * `true` when the tool ran normally. `false` for denial / policy
   * rejection / backend-side failure. The frontend can show a
   * distinct visual (`error` vs `denied`) based on this.
   */
  success: boolean;
  /**
   * The decision the user made (or the policy applied). One of
   *   - `"approved"`  — user clicked Approve
   *   - `"denied"`    — user clicked Deny (or `permission = "deny"`)
   *   - `"auto"`      — `permission = "auto"`, ran without asking
   * Useful for UI differentiation; `success` already covers the
   * success/failure boolean.
   */
  decision: "approved" | "denied" | "auto";
}

/**
 * A tracked call — the composable holds a reference to the
 * `AssistantToolCall` object so it can mutate `status` / `result` /
 * `error` in place when the backend's result event arrives. Tracking
 * is by the call's `id` (LLM-assigned, matches `ToolCallPartial.id`).
 */
export interface TrackedCall {
  call: AssistantToolCall;
  /** Cleanup callback the host can call to untrack (e.g. on message delete). */
  dispose?: () => void;
}

export function useToolApprovals() {
  const tracked = new Map<string, TrackedCall>();
  let unlisten: UnlistenFn | null = null;
  let initPromise: Promise<void> | null = null;

  /**
   * Lazily register the `chat:tool_result` listener on first use.
   * The Rust side may emit the event before any user click (e.g. when
   * the tool's permission is `"auto"` and the backend runs + returns
   * the result without waiting), so the listener is set up
   * proactively.
   */
  function ensureListener(): Promise<void> {
    if (unlisten) return Promise.resolve();
    if (initPromise) return initPromise;
    initPromise = (async () => {
      unlisten = await listen<ToolResultEvent>("chat:tool_result", (e) => {
        const t = tracked.get(e.payload.call_id);
        if (!t) return; // not tracked — e.g. user already navigated away
        applyResult(t.call, e.payload);
        tracked.delete(e.payload.call_id);
        t.dispose?.();
      });
    })();
    return initPromise;
  }

  /**
   * Track a call so its result event will update it. Call this when
   * the LLM has finished streaming a tool call and a card is
   * mounted, regardless of whether the permission is `"ask"`. For
   * `"auto"` calls the result arrives almost immediately; for `"ask"`
   * calls it arrives after the user clicks Approve/Deny.
   */
  function track(call: AssistantToolCall, dispose?: () => void): void {
    if (!call.id) return;
    tracked.set(call.id, { call, dispose });
    // Kick off the listener registration; we don't need to await it
    // because the listener delivers events asynchronously.
    void ensureListener();
  }

  /** Stop tracking a call (e.g. the user deleted the message). */
  function untrack(callId: string): void {
    const t = tracked.get(callId);
    if (!t) return;
    t.dispose?.();
    tracked.delete(callId);
  }

  /**
   * Approve a pending tool call. Backend will execute the tool and
   * emit `chat:tool_result` (success=true, decision="approved").
   * Optimistically flips the call's status to `"running"` so the UI
   * updates immediately.
   */
  async function approve(callId: string): Promise<void> {
    const t = tracked.get(callId);
    if (t) t.call.status = "running";
    await ensureListener();
    await invoke("approve_tool", { callId });
  }

  /**
   * Deny a pending tool call. Backend will synthesize a denial
   * result (`{"error": "user_denied", ...}`) and emit
   * `chat:tool_result` (success=false, decision="denied") so the
   * LLM sees the rejection and can react. Optimistically flips the
   * call's status to `"denied"`.
   */
  async function deny(callId: string): Promise<void> {
    const t = tracked.get(callId);
    if (t) t.call.status = "denied";
    await ensureListener();
    await invoke("deny_tool", { callId });
  }

  onUnmounted(() => {
    unlisten?.();
    unlisten = null;
    tracked.clear();
    initPromise = null;
  });

  return { track, untrack, approve, deny };
}

/**
 * Apply a `chat:tool_result` payload to a tracked `AssistantToolCall`.
 * Pure function — exported so tests / other hosts can reuse the
 * state-update logic.
 */
export function applyResult(
  call: AssistantToolCall,
  ev: ToolResultEvent,
): void {
  call.result = ev.result;
  if (ev.success) {
    call.status = "done";
    call.error = undefined;
  } else if (ev.decision === "denied") {
    // User/policy rejected — still show the synthesized server response
    // (which carries `{error: "user_denied"}` or similar) so the LLM's
    // rejection feedback is visible to the user too.
    call.status = "denied";
  } else {
    call.status = "error";
    call.error = ev.result;
  }
}
