// useChat — generic streaming composable.
//
// Takes a `StreamSource` (default: auto-detect Tauri vs browser) and an
// optional list of tool definitions. Returns a stateful object with
// `status`, `lastError`, `send()`, `cancel()`, etc.
//
// Consumers (laipe-vue components, or your own app code) call `send()` with
// a config, a working message list, and (optionally) tool definitions; the
// composable appends a placeholder assistant message, streams events into
// it via the source, and resolves when the stream is done.
//
// During streaming the placeholder assistant message accumulates
// `tool_calls` as `AssistantToolCall[]` (one entry per streamed call,
// arguments string may be partial). After the stream ends, the message
// is the canonical record of what the assistant did on this turn.

import { ref } from "vue";
import type {
  AssistantToolCall,
  ChatMessage,
  ProviderConfig,
  StreamEvent,
  ToolDefinition,
} from "laipe-ts";
import { defaultStreamSource, type StreamSource } from "../streams";

export type ChatStatus = "idle" | "streaming";

/**
 * React Vue composable for streaming a chat completion through a `StreamSource`.
 *
 * The composable owns the streaming lifecycle: appends a placeholder assistant
 * message to the caller's `messages` array, streams events into it, and
 * resolves when the stream finishes. The caller can `cancel()` mid-stream.
 *
 * During streaming, the placeholder's `tool_calls` array accumulates
 * `AssistantToolCall[]` (one entry per streamed call; `arguments` may be
 * partial until the stream completes).
 *
 * @param source  The transport to stream from. Defaults to `defaultStreamSource()`
 *                (auto-detect Tauri vs browser).
 * @param tools   Tool schemas to make the LLM tool-aware. Pass a static array
 *                for a fixed tool set, or a getter for a reactive list
 *                (e.g. `() => enabledTools.value` so Settings toggles take
 *                effect on the next send).
 * @returns      `{ status, lastError, send, cancel, clearError, tools }`.
 *
 * @example
 * ```ts
 * const { status, send, cancel } = useChat(tauriStream, TOOLS);
 * await send(config, messages);
 * // Or with a reactive tool list:
 * const { send } = useChat(tauriStream, () => enabledToolsList.value);
 * ```
 */
export function useChat(
  source: StreamSource = defaultStreamSource(),
  tools: ToolDefinition[] | (() => ToolDefinition[]) = [],
) {
  const status = ref<ChatStatus>("idle");
  const lastError = ref<string | null>(null);
  let aborter: AbortController | null = null;

  /**
   * Stream a chat completion. The `messages` array is mutated in place
   * (a placeholder assistant message is appended and filled).
   *
   * @param config         - Provider config
   * @param messages       - Working message list (will be appended to)
   * @param conversationId - Optional id propagated to the diagnostic
   *                         recorder so saved error reports can be
   *                         grouped by conversation. Defaults to none.
   * @returns              Resolves when the stream finishes
   */
  async function send(
    config: ProviderConfig,
    messages: ChatMessage[],
    conversationId?: string,
  ): Promise<void> {
    lastError.value = null;
    status.value = "streaming";
    aborter = new AbortController();

    // Append a placeholder assistant message that we'll stream into.
    // We pre-seed `tool_calls: []` so MessageBubble / ToolCallCard can
    // render the list immediately, and the array grows as partials arrive.
    const assistantMsg: ChatMessage = {
      role: "assistant",
      content: "",
      tool_calls: [],
    };
    messages.push(assistantMsg);

    try {
      for await (const ev of source.send(config, messages.slice(0, -1), resolveTools(tools), {
        signal: aborter.signal,
        conversationId,
      })) {
        applyEvent(ev, assistantMsg);
        if (ev.type === "done" || ev.type === "error") break;
      }
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      assistantMsg.content = `[error] ${msg}`;
      lastError.value = msg;
    } finally {
      status.value = "idle";
      aborter = null;
    }
  }

  function resolveTools(
  tools: ToolDefinition[] | (() => ToolDefinition[]),
): ToolDefinition[] {
  return typeof tools === "function" ? tools() : tools;
}

function applyEvent(ev: StreamEvent, assistantMsg: ChatMessage): void {
    if (ev.type === "text") {
      assistantMsg.content += ev.delta;
    } else if (ev.type === "tool_calls") {
      // Merge streaming partials into the assistant's tool_calls array.
      // Partials may arrive for the same `index` multiple times; we
      // upsert by index so arguments accumulate into a single entry.
      const acc = assistantMsg.tool_calls ?? [];
      for (const p of ev.partials) {
        const idx = p.index ?? acc.length;
        const existing = acc[idx];
        if (existing) {
          // Append arguments delta; refine id/name if we now have them.
          existing.function.arguments += p.arguments_delta ?? "";
          if (p.id && !existing.id) existing.id = p.id;
          if (p.name && !existing.function.name) existing.function.name = p.name;
        } else {
          acc[idx] = {
            id: p.id ?? "",
            type: "function",
            function: {
              name: p.name ?? "",
              arguments: p.arguments_delta ?? "",
            },
          };
        }
      }
      assistantMsg.tool_calls = acc;
    } else if (ev.type === "error") {
      assistantMsg.content = `[${ev.kind}] ${ev.message}`;
      lastError.value = ev.message;
    }
  }

  function cancel(): void {
    aborter?.abort();
  }

  function clearError(): void {
    lastError.value = null;
  }

  return { status, lastError, send, cancel, clearError, tools };
}
