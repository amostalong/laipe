// OpenAI Chat Completions streaming implementation.
// Wire: POST {endpoint}/chat/completions, body { model, messages, stream, tools? }
// Stream: SSE `data: {choices: [{delta: {content|tool_calls}, finish_reason}]}\n\n`
//         terminated by `data: [DONE]`.

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

interface OpenAiChunk {
  choices?: Array<{
    delta?: {
      content?: string | null;
      tool_calls?: Array<{
        index?: number;
        id?: string;
        function?: { name?: string; arguments?: string };
      }>;
    };
    finish_reason?: string | null;
  }>;
}

export async function* streamOpenAiChat(
  config: ProviderConfig,
  messages: ChatMessage[],
  tools: ToolDefinition[] | undefined,
  signal: AbortSignal | undefined,
): AsyncGenerator<StreamEvent, void, undefined> {
  const url = `${config.endpoint.replace(/\/$/, "")}/chat/completions`;
  const body: Record<string, unknown> = {
    model: config.model,
    messages: messages.map(toOpenAiMessage),
    stream: true,
  };
  if (tools && tools.length > 0) body.tools = tools;
  if (config.temperature !== undefined) body.temperature = config.temperature;
  if (config.max_tokens !== undefined) body.max_tokens = config.max_tokens;
  if (config.effort) body.reasoning_effort = config.effort;

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
  const pending: ToolCallPartial[] = [];

  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      const chunk = decoder.decode(value, { stream: true });
      const frames = parser.feed(chunk);
      for (const frame of frames) {
        if (frame.data === "[DONE]") {
          if (pending.length > 0) {
            yield { type: "tool_calls", partials: pending.splice(0) };
          }
          yield { type: "done" };
          return;
        }
        let parsed: OpenAiChunk;
        try {
          parsed = JSON.parse(frame.data) as OpenAiChunk;
        } catch {
          continue; // tolerate non-JSON heartbeats etc.
        }
        const choice = parsed.choices?.[0];
        if (!choice) continue;
        const delta = choice.delta ?? {};

        if (typeof delta.content === "string" && delta.content.length > 0) {
          yield { type: "text", delta: delta.content };
        }
        if (Array.isArray(delta.tool_calls)) {
          for (const tc of delta.tool_calls) {
            const idx = tc.index ?? 0;
            let partial = pending.find((p) => p.index === idx);
            if (!partial) {
              partial = { index: idx, arguments_delta: "" };
              pending.push(partial);
            }
            if (tc.id) partial.id = tc.id;
            if (tc.function?.name) partial.name = tc.function.name;
            if (tc.function?.arguments) partial.arguments_delta += tc.function.arguments;
          }
        }
        if (choice.finish_reason && pending.length > 0) {
          yield { type: "tool_calls", partials: pending.splice(0) };
        }
      }
    }
    // Stream ended without [DONE] — treat as done.
    if (pending.length > 0) {
      yield { type: "tool_calls", partials: pending.splice(0) };
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

function toOpenAiMessage(m: ChatMessage): Record<string, unknown> {
  const out: Record<string, unknown> = { role: m.role, content: m.content };
  if (m.tool_call_id !== undefined) out.tool_call_id = m.tool_call_id;
  if (m.tool_calls !== undefined) out.tool_calls = m.tool_calls;
  return out;
}

function isAbortError(e: unknown): boolean {
  return (
    typeof e === "object" &&
    e !== null &&
    "name" in e &&
    (e as { name: string }).name === "AbortError"
  );
}
