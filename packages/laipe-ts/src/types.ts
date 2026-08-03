// 1:1 mirror of crates/laipe-core/src/types.rs and tool.rs.
// Wire format kept snake_case to match Rust serde renames; TS-side identifiers
// stay camelCase. The `role` and `api_format` discriminators use string
// literals so the JSON shape is the same on both sides.

export type ApiFormat = "openai_chat" | "openai_responses" | "anthropic";

export type EffortLevel = "low" | "medium" | "high";

export type ChatRole = "system" | "user" | "assistant" | "tool";

export type ChatStatus = "idle" | "streaming" | "error" | "cancelled";

export interface ChatMessage {
  role: ChatRole;
  content: string;
  /** OpenAI Responses: carry a tool_call_id when role=Tool */
  tool_call_id?: string;
  /** OpenAI Chat Completions: assistant may carry N tool_calls */
  tool_calls?: AssistantToolCall[];
}

export interface ProviderConfig {
  endpoint: string;
  api_key: string;
  model: string;
  api_format: ApiFormat;
  effort?: EffortLevel;
  max_tokens?: number;
  temperature?: number;
  /**
   * Cross-protocol tool schema. If absent or empty, laipe does NOT write a
   * `tools` field on the wire — the upstream then has zero knowledge tools
   * exist (matches the Rust `ProviderConfig.tools` contract).
   */
  tools?: ToolDefinition[];
}

export interface AssistantToolCall {
  id: string;
  type: "function";
  function: AssistantToolCallFunction;
}

export interface AssistantToolCallFunction {
  name: string;
  /** Accumulating JSON arguments string. May be partial. */
  arguments: string;
}

/// What `dispatchStream` yields. Mirrors `laipe_core::StreamEvent` 1:1.
export type StreamEvent =
  | { type: "text"; delta: string }
  | { type: "tool_calls"; partials: ToolCallPartial[] }
  | { type: "done" }
  | { type: "error"; kind: ChatErrorKind; message: string };

// =============================================================================
// Test connection (1:1 mirror of crates/laipe-streaming/src/test.rs TestProviderParams / Result)
// =============================================================================

/**
 * Parameters for the non-streaming connection test.
 *
 * `apiKey` may be empty (local endpoints like Ollama skip auth). The 3-protocol
 * URL path + auth header + body shape are all derived from `apiFormat`.
 */
export interface TestProviderParams {
  /** Base URL — protocol path (`/chat/completions` etc.) is appended. */
  endpoint: string;
  apiKey: string;
  apiFormat: ApiFormat;
  /** Model id — written into the request body. */
  model: string;
}

/**
 * Result of a connection test.
 *
 * `ok = true` means 2xx response AND we extracted a content text fragment
 * (so we know auth + model id both worked, not just that the server is up).
 * `ok = false` carries the failure reason in `error` (human-readable,
 * includes HTTP status + body preview when available).
 */
export interface TestProviderResult {
  ok: boolean;
  /** HTTP status code, when we got a response back at all. */
  status: number | null;
  error: string | null;
  /** First content text fragment from the response (server-truncated to 200 chars). */
  response: string | null;
  /** Echoed input — useful for UI to show "tested X with Y". */
  endpoint: string;
  model: string;
  apiFormat: ApiFormat;
}

// --- tool schema (mirror of crates/laipe-core/src/tool.rs) -----------------

export type ToolType = "function";

export interface ToolDefinition {
  type: ToolType;
  function: ToolFunction;
}

export interface ToolFunction {
  name: string;
  description: string;
  /** JSON Schema describing the function's parameters. */
  parameters: unknown;
}

export interface ToolCallInfo {
  id: string;
  name: string;
  /** Final, JSON-decoded arguments. */
  arguments: unknown;
}

export interface ToolCallPartial {
  /** Streaming order (0-based). */
  index: number;
  id?: string;
  name?: string;
  /** JSON arguments string (raw, partial). Empty on first delta. */
  arguments_delta: string;
}

export interface ToolResult {
  tool_call_id: string;
  content: string;
}

// --- error kinds (mirror of crates/laipe-core/src/error.rs) ----------------

export type ChatErrorKind =
  | "network"
  | "auth"
  | "model_not_found"
  | "bad_request"
  | "rate_limit"
  | "server_error"
  | "stream_protocol"
  | "unknown";

export interface ChatErrorDiag {
  status?: number;
  body?: string;
  /** Upstream `x-request-id` header (for support tickets) */
  request_id?: string;
  /** Internal stage where the error was raised */
  stage?: string;
}
