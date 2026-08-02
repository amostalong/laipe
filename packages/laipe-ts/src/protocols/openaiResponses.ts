// OpenAI Responses API streaming implementation.
// Wire: POST {endpoint}/responses, body { model, input, stream: true, tools? }
// Stream: SSE event-based:
//   event: response.output_text.delta
//   data: {"delta": "..."}
//   event: response.function_call_arguments.delta
//   data: {"item_id": "...", "output_index": N, "delta": "..."}
//   event: response.completed
//   data: { ... }
// No [DONE] sentinel — `response.completed` marks the end.

import type {
  ChatMessage,
  ProviderConfig,
  StreamEvent,
  ToolCallPartial,
  ToolDefinition,
} from "../types.js";
import type { ChatErrorKind } from "../types.js";
import { SseParser } from "../sse.js";
import { ChatStreamError, errorKindFromStatus } from "../errors.js";

interface ResponsesTextDelta {
  delta?: string;
}
interface ResponsesToolCallDelta {
  item_id?: string;
  output_index?: number;
  delta?: string;
}
interface ResponsesFunctionCallAdded {
  item_id?: string;
  output_index?: number;
  name?: string;
}

export async function* streamOpenAiResponses(
  config: ProviderConfig,
  messages: ChatMessage[],
  tools: ToolDefinition[] | undefined,
  signal: AbortSignal | undefined,
): AsyncGenerator<StreamEvent, void, undefined> {
  const url = `${config.endpoint.replace(/\/$/, "")}/responses`;
  const body: Record<string, unknown> = {
    model: config.model,
    input: messages.map(toResponsesInput),
    stream: true,
  };
  if (tools && tools.length > 0) {
    // Responses API: tools are flat, not nested under `function`.
    body.tools = tools.map((t) => ({
      type: t.type,
      name: t.function.name,
      description: t.function.description,
      parameters: t.function.parameters,
    }));
  }
  if (config.temperature !== undefined) body.temperature = config.temperature;
  if (config.max_tokens !== undefined) body.max_tokens = config.max_tokens;

  const res = await fetch(url, {
    method: "POST",
    headers: {
      "content-type": "application/json",
      authorization: `Bearer ${config.api_key}`,
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
  // Map item_id -> partial. Responses identifies tool calls by item_id rather
  // than by index, so we keep a lookup.
  const partialsByItemId = new Map<string, ToolCallPartial>();
  let nextIndex = 0;

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
        if (event === "response.output_text.delta") {
          const d = parsed as ResponsesTextDelta;
          if (typeof d?.delta === "string" && d.delta.length > 0) {
            yield { type: "text", delta: d.delta };
          }
        } else if (event === "response.output_item.added") {
          const d = parsed as ResponsesFunctionCallAdded;
          if (d?.item_id && !partialsByItemId.has(d.item_id)) {
            partialsByItemId.set(d.item_id, {
              index: nextIndex++,
              id: d.item_id,
              name: d.name,
              arguments_delta: "",
            });
          }
        } else if (event === "response.function_call_arguments.delta") {
          const d = parsed as ResponsesToolCallDelta;
          if (d?.item_id && typeof d.delta === "string") {
            let partial = partialsByItemId.get(d.item_id);
            if (!partial) {
              partial = { index: nextIndex++, id: d.item_id, arguments_delta: "" };
              partialsByItemId.set(d.item_id, partial);
            }
            partial.arguments_delta += d.delta;
          }
        } else if (event === "response.completed" || event === "response.done") {
          if (partialsByItemId.size > 0) {
            yield { type: "tool_calls", partials: [...partialsByItemId.values()] };
            partialsByItemId.clear();
          }
          yield { type: "done" };
          return;
        }
      }
    }
    if (partialsByItemId.size > 0) {
      yield { type: "tool_calls", partials: [...partialsByItemId.values()] };
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

function toResponsesInput(m: ChatMessage): Record<string, unknown> {
  if (m.role === "tool") {
    return { type: "function_call_output", call_id: m.tool_call_id, output: m.content };
  }
  return { role: m.role, content: m.content };
}

function isAbortError(e: unknown): boolean {
  return (
    typeof e === "object" &&
    e !== null &&
    "name" in e &&
    (e as { name: string }).name === "AbortError"
  );
}
