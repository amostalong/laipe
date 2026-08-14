// Non-streaming test/ping of an endpoint + api_key + model combo (browser side).
//
// 1:1 mirror of crates/laipe-streaming/src/test.rs `test_provider`. We keep
// the two implementations in lock-step because laipe-app (Vue 3 + Vite,
// no Rust side) hits the TS port while Tauri-based consumers (laipe-app's
// Tauri build, FinaBoard, PlotCraft) hit the Rust port. Same wire shape,
// same error semantics — pick whichever your build can reach.
//
// ## Protocol handling
//
// - OpenAI Chat    → POST {endpoint}/chat/completions
// - OpenAI Responses → POST {endpoint}/v1/responses
// - Anthropic      → POST {endpoint}/v1/messages
//
// Auth header is `Authorization: Bearer <key>` for the two OpenAI variants
// and `x-api-key: <key>` + `anthropic-version: 2023-06-01` for Anthropic.
// The body is `{model, messages: [{role: user, content: "hi"}], max_tokens: 1,
// stream: false}` (Anthropic gets the user message in `messages` directly —
// no system role sent on a ping).

import type { ApiFormat, TestProviderParams, TestProviderResult } from "./types.js";

const MAX_RESPONSE_PREVIEW = 200;

/**
 * Hit the endpoint once with a minimal request and report whether auth +
 * model id both work end-to-end.
 *
 * Returns a structured result for every code path. Network errors,
 * non-2xx status, invalid JSON, and "200 but no extractable text" all
 * flow through `ok = false` with a descriptive `error` string.
 */
export async function testProvider(
  params: TestProviderParams,
  options?: { signal?: AbortSignal; fetchImpl?: typeof fetch },
): Promise<TestProviderResult> {
  const endpoint = params.endpoint.trim();
  const model = params.model.trim();
  const apiFormat = params.apiFormat;
  const apiKey = params.apiKey;
  const fetchImpl = options?.fetchImpl ?? globalThis.fetch;

  if (!fetchImpl) {
    return {
      ok: false,
      status: null,
      error: "fetch is not available in this environment",
      response: null,
      endpoint,
      model,
      apiFormat,
    };
  }

  const url = `${endpoint.replace(/\/+$/, "")}${testEndpointPath(apiFormat)}`;
  const body = buildTestBody(apiFormat, model);
  const headers: Record<string, string> = {
    "Content-Type": "application/json",
  };
  if (apiKey) {
    if (apiFormat === "anthropic_messages") {
      headers["x-api-key"] = apiKey;
      headers["anthropic-version"] = "2023-06-01";
    } else {
      headers["Authorization"] = `Bearer ${apiKey}`;
    }
  }

  let resp: Response;
  try {
    resp = await fetchImpl(url, {
      method: "POST",
      headers,
      body: JSON.stringify(body),
      signal: options?.signal,
    });
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    return {
      ok: false,
      status: null,
      error: `request failed: ${msg}`,
      response: null,
      endpoint,
      model,
      apiFormat,
    };
  }

  const status = resp.status;
  const bodyText = await resp.text();

  if (!resp.ok) {
    return {
      ok: false,
      status,
      error: `HTTP ${status}: ${truncate(bodyText, 500)}`,
      response: null,
      endpoint,
      model,
      apiFormat,
    };
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(bodyText);
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    return {
      ok: false,
      status,
      error: `invalid JSON response: ${msg} (body: ${truncate(bodyText, 200)})`,
      response: null,
      endpoint,
      model,
      apiFormat,
    };
  }

  const text = extractResponseText(apiFormat, parsed);
  const preview = text ? truncate(text, MAX_RESPONSE_PREVIEW) : null;

  if (!preview) {
    return {
      ok: false,
      status,
      error: `HTTP ${status} but no extractable content text. Check that model id is correct.`,
      response: null,
      endpoint,
      model,
      apiFormat,
    };
  }

  return {
    ok: true,
    status,
    error: null,
    response: preview,
    endpoint,
    model,
    apiFormat,
  };
}

// --- internals ---

function buildTestBody(
  apiFormat: ApiFormat,
  model: string,
): Record<string, unknown> {
  const messages = [{ role: "user", content: "hi" }];
  switch (apiFormat) {
    case "openai_chat":
      return {
        model,
        messages,
        max_tokens: 1,
        stream: false,
      };
    case "openai_responses":
      return {
        model,
        input: messages,
        max_tokens: 1,
        stream: false,
      };
    case "anthropic_messages":
      return {
        model,
        max_tokens: 1,
        messages,
      };
  }
}

function testEndpointPath(apiFormat: ApiFormat): string {
  switch (apiFormat) {
    case "openai_chat":
      return "/chat/completions";
    case "openai_responses":
      return "/v1/responses";
    case "anthropic_messages":
      return "/v1/messages";
  }
}

function extractResponseText(
  apiFormat: ApiFormat,
  body: unknown,
): string | null {
  if (!body || typeof body !== "object") return null;
  const b = body as Record<string, unknown>;
  switch (apiFormat) {
    case "openai_chat": {
      const choices = b.choices;
      if (!Array.isArray(choices) || !choices[0]) return null;
      const first = choices[0] as Record<string, unknown>;
      const message = first.message as Record<string, unknown> | undefined;
      const content = message?.content;
      return typeof content === "string" ? content : null;
    }
    case "openai_responses": {
      const output = b.output;
      if (!Array.isArray(output) || !output[0]) return null;
      const first = output[0] as Record<string, unknown>;
      const content = first.content;
      if (!Array.isArray(content) || !content[0]) return null;
      const firstContent = content[0] as Record<string, unknown>;
      const text = firstContent.text;
      return typeof text === "string" ? text : null;
    }
    case "anthropic_messages": {
      const content = b.content;
      if (!Array.isArray(content) || !content[0]) return null;
      const first = content[0] as Record<string, unknown>;
      const text = first.text;
      return typeof text === "string" ? text : null;
    }
  }
}

function truncate(s: string, max: number): string {
  // Code-point safe — emoji + CJK take 1 slot each.
  return Array.from(s).slice(0, max).join("");
}
