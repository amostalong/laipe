// Anthropic Messages API streaming implementation.
// Wire: POST {endpoint}/v1/messages, body { model, messages, system?, max_tokens, stream: true, tools? }
// Stream: SSE event-based:
//   event: message_start
//   event: content_block_start          (type=tool_use)
//   event: content_block_delta          (delta.type=text_delta | input_json_delta)
//   event: content_block_stop
//   event: message_delta                (stop_reason)
//   event: message_stop                 (end)

import type {
  ChatMessage,
  EffortLevel,
  ProviderConfig,
  StreamEvent,
  ToolCallPartial,
  ToolDefinition,
} from "../types.js";
import type { ChatErrorKind } from "../types.js";
import { SseParser } from "../sse.js";
import { ChatStreamError, errorKindFromStatus } from "../errors.js";

interface AnthropicContentBlockStart {
  index?: number;
  content_block?: {
    type?: string;
    id?: string;
    name?: string;
  };
}
interface AnthropicContentBlockDelta {
  index?: number;
  delta?: {
    type?: string;
    text?: string;
    partial_json?: string;
  };
}

export async function* streamAnthropic(
  config: ProviderConfig,
  messages: ChatMessage[],
  tools: ToolDefinition[] | undefined,
  signal: AbortSignal | undefined,
): AsyncGenerator<StreamEvent, void, undefined> {
  const url = `${config.endpoint.replace(/\/$/, "")}/v1/messages`;
  const { system, rest } = splitSystemMessages(messages);
  const body: Record<string, unknown> = {
    model: config.model,
    messages: rest.map(toAnthropicMessage),
    max_tokens: config.max_tokens ?? 1024,
    stream: true,
  };
  if (system) body.system = system;
  if (tools && tools.length > 0) {
    body.tools = tools.map((t) => ({
      name: t.function.name,
      description: t.function.description,
      input_schema: t.function.parameters,
    }));
  }
  if (config.temperature !== undefined) body.temperature = config.temperature;
  if (config.effort) {
    const budget = anthropicBudgetForEffort(config.effort);
    if (budget > 0) {
      body.thinking = { type: "enabled", budget_tokens: budget };
    }
  }

  const res = await fetch(url, {
    method: "POST",
    headers: {
      "content-type": "application/json",
      "x-api-key": config.api_key,
      "anthropic-version": "2023-06-01",
      "anthropic-dangerous-direct-browser-access": "true",
    },
    body: JSON.stringify(body),
    signal,
  });

  if (!res.ok) {
    const errBody = await res.text().catch(() => "");
    const kind: ChatErrorKind = errorKindFromStatus(res.status);
    throw new ChatStreamError(kind, `upstream returned ${res.status}: ${errBody.slice(0, 800)}`, {
      status: res.status,
      body: errBody.slice(0, 1024),
    });
  }
  if (!res.body) throw new ChatStreamError("stream_protocol", "no response body");

  const reader = res.body.getReader();
  const decoder = new TextDecoder();
  const parser = new SseParser();
  // Map content-block index -> tool-call partial
  const toolPartials = new Map<number, ToolCallPartial>();
  let nextToolIndex = 0;

  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      const chunk = decoder.decode(value, { stream: true });
      const frames = parser.feed(chunk);
      for (const frame of frames) {
        const event = frame.event;
        let parsed: unknown;
        try {
          parsed = frame.data ? JSON.parse(frame.data) : null;
        } catch {
          continue;
        }
        if (event === "content_block_start") {
          const d = parsed as AnthropicContentBlockStart;
          if (d?.content_block?.type === "tool_use" && typeof d.index === "number") {
            toolPartials.set(d.index, {
              index: nextToolIndex++,
              id: d.content_block.id,
              name: d.content_block.name,
              arguments_delta: "",
            });
          }
        } else if (event === "content_block_delta") {
          const d = parsed as AnthropicContentBlockDelta;
          if (d?.delta?.type === "text_delta" && typeof d.delta.text === "string") {
            yield { type: "text", delta: d.delta.text };
          } else if (d?.delta?.type === "input_json_delta" && typeof d.index === "number") {
            const partial = toolPartials.get(d.index);
            if (partial && typeof d.delta.partial_json === "string") {
              partial.arguments_delta += d.delta.partial_json;
            }
          }
        } else if (event === "message_stop") {
          if (toolPartials.size > 0) {
            yield { type: "tool_calls", partials: [...toolPartials.values()] };
            toolPartials.clear();
          }
          yield { type: "done" };
          return;
        }
      }
    }
    if (toolPartials.size > 0) {
      yield { type: "tool_calls", partials: [...toolPartials.values()] };
    }
    yield { type: "done" };
  } catch (e: unknown) {
    if (isAbortError(e)) {
      yield { type: "error", kind: "unknown", message: "aborted" };
      return;
    }
    throw e;
  } finally {
    reader.releaseLock();
  }
}

function splitSystemMessages(messages: ChatMessage[]): { system?: string; rest: ChatMessage[] } {
  const systemParts: string[] = [];
  const rest: ChatMessage[] = [];
  for (const m of messages) {
    if (m.role === "system") systemParts.push(m.content);
    else rest.push(m);
  }
  return { system: systemParts.length > 0 ? systemParts.join("\n\n") : undefined, rest };
}

function toAnthropicMessage(m: ChatMessage): Record<string, unknown> {
  if (m.role === "tool") {
    return {
      role: "user",
      content: [
        {
          type: "tool_result",
          tool_use_id: m.tool_call_id,
          content: m.content,
        },
      ],
    };
  }
  if (m.role === "assistant" && m.tool_calls && m.tool_calls.length > 0) {
    return {
      role: "assistant",
      content: m.tool_calls.map((tc) => ({
        type: "tool_use",
        id: tc.id,
        name: tc.function.name,
        input: safeJsonParse(tc.function.arguments),
      })),
    };
  }
  return { role: m.role, content: m.content };
}

function safeJsonParse(s: string): unknown {
  try {
    return JSON.parse(s);
  } catch {
    return {};
  }
}

function isAbortError(e: unknown): boolean {
  return (
    typeof e === "object" &&
    e !== null &&
    "name" in e &&
    (e as { name: string }).name === "AbortError"
  );
}

/**
 * Map `EffortLevel` to Anthropic `thinking.budget_tokens`. None → 0 (caller omits the field).
 * 1:1 mirror of `laipe_core::EffortLevel::to_anthropic_budget`.
 */
export function anthropicBudgetForEffort(effort: EffortLevel): number {
  switch (effort) {
    case "none":
      return 0;
    case "low":
      return 1024;
    case "medium":
      return 4096;
    case "high":
      return 16384;
    case "xhigh":
      return 32768;
    case "max":
      return 65536;
  }
}
