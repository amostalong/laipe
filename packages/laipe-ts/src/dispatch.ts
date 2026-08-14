// Main streaming entry point. Mirrors `laipe_streaming::pick(cfg.api_format)
// .dispatch(cfg, messages, tools)` on the Rust side, plus an `AbortSignal`
// for browser/Node cancellation (the Rust side uses `CancelHandle` separately).

import type { ChatMessage, ProviderConfig, StreamEvent, ToolDefinition } from "./types.js";
import { streamOpenAiChat } from "./protocols/openaiChat.js";
import { streamOpenAiResponses } from "./protocols/openaiResponses.js";
import { streamAnthropic } from "./protocols/anthropic.js";

export interface DispatchOptions {
  /** Browser/Node-native abort signal. Cancel the stream via `controller.abort()`. */
  signal?: AbortSignal;
}

/**
 * Stream a chat completion from a provider, yielding `StreamEvent`s.
 *
 * Mirrors the Rust `laipe_streaming::pick(cfg.api_format).dispatch(cfg, messages, tools)`,
 * plus an `AbortSignal` for browser/Node cancellation. (The Rust side uses
 * `CancelHandle` separately — see `laipe-tokio`.)
 *
 * Errors before the stream opens throw from the generator. Errors mid-stream
 * are surfaced as `{ type: "error", kind, message }` events.
 *
 * @param config   Provider config (endpoint, key, model, format).
 * @param messages Working message list (caller-owned; not mutated).
 * @param tools    Optional tool schemas. When absent/empty, no `tools` field
 *                 is written to the wire — the upstream has zero knowledge
 *                 that tools exist (matches the Rust `ProviderConfig.tools` contract).
 * @param options  AbortSignal etc.
 * @yields        `StreamEvent` (text delta / tool_calls partials / done / error).
 *
 * @example
 * ```ts
 * for await (const ev of dispatchStream(config, messages, TOOLS, { signal })) {
 *   if (ev.type === "text") console.log(ev.delta);
 *   else if (ev.type === "done") break;
 *   else if (ev.type === "error") throw new Error(ev.message);
 * }
 * ```
 */
export async function* dispatchStream(
  config: ProviderConfig,
  messages: ChatMessage[],
  tools?: ToolDefinition[],
  options: DispatchOptions = {},
): AsyncGenerator<StreamEvent, void, undefined> {
  const { signal } = options;
  switch (config.api_format) {
    case "openai_chat":
      yield* streamOpenAiChat(config, messages, tools, signal);
      break;
    case "openai_responses":
      yield* streamOpenAiResponses(config, messages, tools, signal);
      break;
    case "anthropic_messages":
      yield* streamAnthropic(config, messages, tools, signal);
      break;
  }
}
