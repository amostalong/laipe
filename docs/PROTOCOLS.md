# Protocols

`laipe` speaks three LLM wire formats. All three are produced from the same
in-memory `ChatMessage` / `ToolDefinition` / `StreamEvent` shape — laipe does
the protocol translation at the boundary.

| | **OpenAI Chat Completions** | **OpenAI Responses** | **Anthropic Messages** |
|---|---|---|---|
| Endpoint | `POST /v1/chat/completions` | `POST /v1/responses` | `POST /v1/messages` |
| Auth header | `Authorization: Bearer …` | `Authorization: Bearer …` | `x-api-key: …` + `anthropic-version: 2023-06-01` |
| SSE chunk shape | `data: {json}\n\n` | `event: response.…\ndata: {json}\n\n` | `event: message.…\ndata: {json}\n\n` |
| Stream end marker | `data: [DONE]` | `event: response.completed` | `event: message_stop` |
| Tool schema field | `tools: [{type, function: {name, description, parameters}}]` | `tools: [{type, name, description, parameters}]` (flat, no nested `function`) | `tools: [{name, description, input_schema: parameters}]` |
| Tool result message | `role: "tool", tool_call_id, content` | `type: "function_call_output", id, call_id, tool_call_id, output` (3 ids for cross-version compat) | `role: "user"` content block `{type: "tool_result", tool_use_id, content}` |
| Assistant tool call shape | `tool_calls: [{id, type, function: {name, arguments}}]` on assistant msg | `type: "function_call", id, call_id, name, arguments` (3 ids) | `content: [..., {type: "tool_use", id, name, input}]` |
| Reasoning effort | `reasoning_effort` field | `reasoning: {effort}` | `thinking: {type: "enabled", budget_tokens}` |
| `messages` field | `messages: [...]` | `input: [...]` | `messages: [...]` |

## What laipe normalizes

Despite the table, your app only ever sees:

```rust
// in
ChatMessage { role, content, tool_call_id?, tool_calls? }
ToolDefinition { kind, function: { name, description, parameters } }
ProviderConfig { endpoint, api_key, model, api_format, tools?, effort?, ... }

// out
StreamEvent::Text(String)              // text delta, append-only
StreamEvent::ToolCalls(Vec<ToolCallPartial>)
StreamEvent::Done
```

The `OpenAiChat` / `OpenAiResponses` / `Anthropic` variants of `ApiFormat`
pick the right serializer. You don't touch protocol JSON.

## When to pick which

- **OpenAI Chat Completions** — the universal default. Most third-party
  "OpenAI-compatible" providers (DeepSeek, GLM, OpenRouter, llama.cpp's
  server, vLLM, etc.) speak this. Use it unless you specifically need
  Responses-only features.
- **OpenAI Responses** — newer. Required for some newer OpenAI features
  (built-in tools like web_search, file_search). Use when the user
  explicitly opts in via Settings.
- **Anthropic** — required for Claude. Not OpenAI-compatible at the wire
  level; the SSE shape and tool schema are different enough that laipe
  maintains a real implementation, not a flag.
