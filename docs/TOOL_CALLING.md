# Tool calling

`laipe` carries one tool schema in memory (`ToolDefinition`) and translates
to the right wire shape at request-build time. Consumers always see the
same accumulating-partials pattern, regardless of protocol.

## Internal shape (what your app writes)

```rust
pub struct ToolDefinition {
    pub kind: ToolType,    // always Function today
    pub function: ToolFunction,
}

pub struct ToolFunction {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,  // JSON Schema
}
```

This is **literally the OpenAI Chat Completions shape**. laipe keeps it
internally because it's the most common, and translates to the other two
at the wire.

## Wire translation

| Upstream | Outbound `tools` field |
|---|---|
| OpenAI Chat Completions | `[{type: "function", function: {name, description, parameters}}]` (pass-through) |
| OpenAI Responses | `[{type, name, description, parameters}]` — **flattened**, no nested `function` |
| Anthropic | `[{name, description, input_schema: parameters}]` — `input_schema` instead of `parameters` |

If `tools` is `None` or empty, **laipe does not write a `tools` field on the
wire at all**. The upstream has zero knowledge that tools exist. This is
intentional — passing `tools: []` to most upstreams causes them to behave
as if the model can call a function with no schema, which is a footgun.

## Inbound: tool call partials

The upstream streams tool calls in fragments:

```
delta.tool_calls: [
  { index: 0, id: "call_abc", function: { name: "get_weather", arguments: "" } }
]
delta.tool_calls: [
  { index: 0, function: { arguments: "{\"loc" } }
]
delta.tool_calls: [
  { index: 0, function: { arguments: "ation\": \"SF\"}" } }
]
```

`laipe` flattens all three into a stream of `ToolCallPartial` events:

```rust
pub struct ToolCallPartial {
    pub index: u32,                  // accumulate by this
    pub id: Option<String>,          // arrives on first delta
    pub name: Option<String>,        // arrives on first delta
    pub arguments_delta: String,     // raw JSON fragment, concatenate
}
```

The consumer concatenates `arguments_delta` per `index` until `Done` is
received, then `JSON.parse()`s the result and dispatches.

## Inbound: tool result echo

The player's reply to a tool call goes back as a `ChatMessage` with
`role: ChatRole::Tool` and a `tool_call_id`. laipe then translates per
protocol:

| Upstream | `ChatMessage { role: Tool, tool_call_id, content }` becomes |
|---|---|
| OpenAI Chat Completions | `{ role: "tool", tool_call_id, content }` (pass-through) |
| OpenAI Responses | `{ type: "function_call_output", id, call_id, tool_call_id, output }` (3 ids for cross-version compat) |
| Anthropic | `{ role: "user", content: [{ type: "tool_result", tool_use_id, content }] }` |

## Assistant messages with tool calls

When the assistant emits a tool call, the next `messages[]` entry needs to
carry it. For multi-round tool calling, the consumer re-feeds the assistant
message into `dispatch()`:

```rust
// After Done, if any ToolCallInfo arrived:
messages.push(ChatMessage {
    role: ChatRole::Assistant,
    content: String::new(),
    tool_calls: Some(/* AssistantToolCall[...] reconstructed from accumulated partials */),
});

// Then the player's answer:
messages.push(ChatMessage {
    role: ChatRole::Tool,
    tool_call_id: Some(info.id),
    content: info.arguments.to_string(),  // or however your tool resolves
});

// Then dispatch round 2.
let mut rx = pick(cfg.api_format).dispatch(&cfg, &messages, tools).await?;
```

Wire translation happens inside `build_openai_request_body` /
`build_anthropic_request_body` — your app never sees protocol JSON.

## Built-in patterns

`laipe` ships three canonical tool patterns your app can drop in:

- **`ask_user_question`** — LLM surfaces 2-4 options, player picks one,
  answer fed back as a `role: "tool"` message. Good for branching
  decisions ("which character should I focus on next?").
- **`ask_free_text`** — LLM asks an open-ended question, player types
  into an inline input. **Bypasses the chat composer**: the bubble
  embeds the input directly and the composer is disabled until the
  player submits. Good for "tell me more about the villain".
- **`update_doc_item`** — LLM proposes a doc edit, player confirms, edit
  applied. Default permission is `ask` (player must approve before the
  editor is touched). Good for "add this character to the world doc".

These are JSON schemas laipe includes in `packages/laipe-ts` /
`crates/laipe-core` — your app just needs to register handlers.
