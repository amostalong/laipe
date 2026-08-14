// StreamSource — abstract interface for "where do chat events come from?"
//
// Why this exists
// ===============
//
// laipe-vue components should not care whether chat events come from:
//   - a Tauri Rust backend (production default)
//   - a direct browser fetch to the LLM API (browser-only fallback)
//   - a mock that synthesizes responses (testing)
//   - a future WebSocket / IPC bridge
//
// Each implementation of StreamSource yields the same `StreamEvent` shape
// (from laipe-ts), so consumers can swap them transparently.
//
// Usage
// =====
//
//   import { useChat, tauriStream } from "laipe-vue";
//   const { send, cancel, status } = useChat(tauriStream);
//
//   import { useChat, fetchStream } from "laipe-vue";
//   const { send, cancel, status } = useChat(fetchStream);
//
//   import { useChat, mockStream } from "laipe-vue";
//   const { send, cancel, status } = useChat(mockStream);

import type {
  ChatErrorKind,
  ChatMessage,
  ProviderConfig,
  StreamEvent,
  ToolCallPartial,
  ToolDefinition,
  ToolPermission,
} from "laipe-ts";
import type { UnlistenFn } from "@tauri-apps/api/event";

export interface StreamSource {
  /**
   * Stream a chat completion. Yields `StreamEvent`s as they arrive.
   * Resolves cleanly when the stream ends with `Done` or `Error`.
   * Throws on pre-stream errors (e.g. 401 from the upstream).
   *
   * @param config   - Provider config
   * @param messages - Working message list (caller-owned; may be mutated)
   * @param tools    - Optional tool definitions. When present, the
   *                   backend / upstream will see the LLM as tool-aware.
   *                   Mirrors the `dispatchStream` arg order from laipe-ts.
   * @param options  - AbortSignal + conversationId for diagnostic
   *                   context, plus `toolPermissions` to tell the
   *                   backend how to gate each tool (auto / ask / deny).
   *                   `conversationId` is propagated to the diagnostic
   *                   recorder so saved error reports can be grouped
   *                   by conversation.
   */
  send(
    config: ProviderConfig,
    messages: ChatMessage[],
    tools?: ToolDefinition[],
    options?: {
      signal?: AbortSignal;
      conversationId?: string;
      toolPermissions?: Record<string, ToolPermission>;
    },
  ): AsyncGenerator<StreamEvent, void, undefined>;
}

// ============================================================================
// tauriStream — the production default
// ============================================================================

/**
 * Tauri-based stream: invokes the Rust `chat` command and listens to the
 * Tauri events the backend emits. Requires the `@tauri-apps/api` runtime
 * (i.e. must be running inside a Tauri webview).
 */
export const tauriStream: StreamSource = {
  async *send(config, messages, tools, options) {
    // Dynamic imports so this module can be loaded in non-Tauri contexts
    // (the bundler won't try to resolve @tauri-apps/* at build time in
    // browser-only mode if we don't reference it).
    const { invoke } = await import("@tauri-apps/api/core");
    const { listen } = await import("@tauri-apps/api/event");

    const queue: StreamEvent[] = [];
    let pending: (() => void) | null = null;
    const wake = () => {
      const p = pending;
      pending = null;
      p?.();
    };
    const push = (ev: StreamEvent) => {
      queue.push(ev);
      wake();
    };

    const unlisteners: UnlistenFn[] = [];
    unlisteners.push(
      await listen<string>("chat:chunk", (e) => {
        push({ type: "text", delta: e.payload });
      }),
    );
    unlisteners.push(
      await listen<ToolCallPartial[]>("chat:tool_calls", (e) => {
        // Forward the full partials (id / name / accumulating arguments)
        // to the consumer. The Rust side serializes `ToolCallPartial` 1:1.
        push({ type: "tool_calls", partials: e.payload });
      }),
    );
    unlisteners.push(
      await listen("chat:done", () => {
        push({ type: "done" });
      }),
    );
    unlisteners.push(
      await listen<{ kind: ChatErrorKind; message: string }>(
        "chat:error",
        (e) => {
          push({
            type: "error",
            kind: e.payload.kind,
            message: e.payload.message,
          });
        },
      ),
    );
    unlisteners.push(
      await listen("chat:cancelled", () => {
        // Backend tells us we were cancelled; emit as an error so the
        // consumer can clear streaming state.
        push({
          type: "error",
          kind: "unknown",
          message: "cancelled",
        });
      }),
    );
    unlisteners.push(
      await listen<{
        tool_call_id: string;
        name: string;
        arguments: string;
      }>("chat:tool_needs_approval", (e) => {
        // Rust is asking the user to approve a tool call (the tool's
        // permission is `ask`). Forward as a StreamEvent so useChat can
        // mark the corresponding AssistantToolCall as `pending_approval`
        // and render the Approve/Deny buttons.
        push({
          type: "tool_pending_approval",
          tool_call_id: e.payload.tool_call_id,
          name: e.payload.name,
          arguments: e.payload.arguments,
        });
      }),
    );
    // `chat:tool_result` is *not* listened to here on purpose —
    // `useToolApprovals` is the canonical owner of the result state
    // (it holds a reference to the AssistantToolCall and mutates
    // status / result / error in place). Listening here would just
    // duplicate work and risk the two paths diverging.

    // Kick off the Rust side. We don't await it; we wait for events
    // instead. If the invoke itself throws (e.g. IPC error), surface it
    // as an event so the consumer doesn't hang.
    void invoke("chat", {
      cfg: config,
      messages,
      tools,
      conversationId: options?.conversationId ?? null,
      toolPermissions: options?.toolPermissions ?? {},
    }).catch((e: unknown) => {
      const msg = e instanceof Error ? e.message : String(e);
      push({ type: "error", kind: "unknown", message: msg });
    });

    try {
      while (true) {
        while (queue.length > 0) {
          const ev = queue.shift()!;
          if (ev.type === "done" || ev.type === "error") {
            return;
          }
          yield ev;
        }
        if (options?.signal?.aborted) {
          // We were cancelled but no event arrived. Emit a synthetic
          // cancellation error.
          yield {
            type: "error",
            kind: "unknown",
            message: "aborted",
          };
          return;
        }
        await new Promise<void>((r) => {
          pending = r;
        });
      }
    } finally {
      unlisteners.forEach((u) => u());
    }
  },
};

// ============================================================================
// fetchStream — browser-only fallback (uses laipe-ts directly)
// ============================================================================

/**
 * Browser-based stream: calls `laipe-ts`'s `dispatchStream` directly. Only
 * works when the endpoint is CORS-permissive (most aren't). Useful for
 * demos, testing, and web-only deployments.
 */
export const fetchStream: StreamSource = {
  async *send(config, messages, tools, options) {
    const { dispatchStream } = await import("laipe-ts");
    yield* dispatchStream(config, messages, tools, { signal: options?.signal });
  },
};

// ============================================================================
// mockStream — for tests, demos, and offline development
// ============================================================================

/**
 * Mock stream that echoes the last user message with a small delay per
 * "word". Useful for component tests and offline UI development.
 * Ignores `tools` — never declares tool calls.
 */
export const mockStream: StreamSource = {
  async *send(_config, messages, _tools, options) {
    const last = [...messages].reverse().find((m) => m.role === "user");
    const text = last?.content ?? "(no input)";
    const words = text.split(/(\s+)/);
    yield { type: "text", delta: "▍ " };
    for (const w of words) {
      if (options?.signal?.aborted) {
        yield { type: "error", kind: "unknown", message: "aborted" };
        return;
      }
      yield { type: "text", delta: w };
      await new Promise((r) => setTimeout(r, 30));
    }
    yield { type: "text", delta: " ▌" };
    yield { type: "done" };
  },
};

/** The default stream source: Tauri in production, fetch in browser-only. */
export function defaultStreamSource(): StreamSource {
  // Detect Tauri runtime via the global it sets on the window.
  if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
    return tauriStream;
  }
  return fetchStream;
}
