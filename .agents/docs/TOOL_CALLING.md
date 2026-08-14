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

## Patterns from the field (PlotCraft v0.5+ feedback)

PlotCraft forks `laipe` and ran into four real LLM behavior issues in
production. These are **recommended patterns** your fork can copy verbatim
into your system prompt — they solved 100% of the test cases PlotCraft ran
through with `deepseek-v4-flash` and other models.

### 1. The "1 round 1 tool call" hard rule

LLMs in 2025 are trained to be **helpful** — which means they tend to
"one-stop fill" by stacking tool calls in a single round, or alternating
"here's text, here's a tool call" mid-stream. Both behaviors break the
player's mental model: they want to see one thing at a time and decide.

Add this to your system prompt:

```
**工具调用节奏（硬规则）**:
- 1 round 1 tool call — 一次只发起 1 个 tool, 让玩家先选/答完, 再下一轮发新问题
- **不要**一次发起多个 tool (不要同时调 ask_choose_option + ask_user_question + update_doc_item)
- 想追问 / 让玩家再做选择 / 写入编辑器 → 拆成 N 轮, 每轮 1 个 tool
- **不要**自作主张"一站式服务" — 玩家主导节奏, AI 不要催
```

PlotCraft verified this with deepseek-v4-flash. Before the rule, the
model would emit `update_doc_item` immediately after `ask_choose_option`
got declined — a direct violation of player-led UX. After the rule, the
model waits for an explicit "用 A" / "采用 X" before writing.

### 2. The "update_doc_item 写入硬规则" (playcentric core)

Many LLMs default to "if the user mentioned X, just go write it" — this
violates player-led design. Add a hard rule that distinguishes **asking
about X** from **committing to X**:

```
**update_doc_item 写入硬规则（playcentric 核心）**:
- **绝不**在玩家没明确说"用 A"/"采用 X"/"写成 Y"时调 update_doc_item
- **绝不**把"玩家打字问问题 / 描述想法"当成"玩家要求写入" — 问 ≠ 写
- 玩家在打字问"做成 X 怎么样"/"是不是应该 Y" → 调 ask_user_question 反问 / ask_choose_option 给备选
  → 玩家选/答完, 下一轮 LLM 才能调 update_doc_item 写入
- LLM 永不"猜测玩家意图后直接写入" — 必须等玩家先选/答
```

### 3. The few-shot example pattern (LLM behavioral constraint)

Some models (notably deepseek-v4-flash) ignore abstract rules like "不要
附 preamble text" and still emit a long preamble as `content` before
calling `ask_choose_option`. The fix is **showing** them a correct and
incorrect response, side by side. Add this after your hard rules:

```
**正确响应示例 (few-shot, deepseek 行为约束)**:
玩家点 chip "💡 从立意拆支柱" → 你应该**只**调 ask_choose_option tool, **不要**附 preamble text:
- ✓ 正确: `content=""` (空) + `tool_calls=[{name: "ask_choose_option", arguments: "{\"question\":\"具体问题\",\"options\":[{\"label\":\"X\",\"preview\":\"...\"},{\"label\":\"Y\",\"preview\":\"...\"},{\"label\":\"Z\",\"preview\":\"...\"}]}"}]`
- ✗ 错误 1: `content="好的, 我帮你拆支柱..."` + tool_calls=[] (附客套话 + 没调 tool)
- ✗ 错误 2: `content=""` + `tool_calls=[{arguments: "{\"options\":[...]}"}]` (缺 question 字段, 前端解析失败)
- ✗ 错误 3: `content="从 L1 立意拆 3-5 条..."` (把 chip prompt 复述作为 content 字段 — deepseek 常见错误)
- ✗ 错误 4: `tool_calls=[{arguments: "{\"question\":\"从 L1 立意拆 3-5 条...\",\"options\":[\"A\",\"B\"]}"}]` (question 字段是整段 prompt 复述 + options 是字符串数组不是对象数组)
核心: **content 字段保持空** (不附任何文字 / 客套话 / 复述), 让 tool_call 自己说话.

**ask_choose_option schema 关键约束**:
- `question` 字段 = 1 句具体问题 (**不**是整段 prompt 复述, **不**是 chip label 复述)
- `options` 数组 = 2-5 个对象, 每个对象**必须**有 `label` (≤10 字) + `preview` (完整备选内容)
- options 数组元素**必须**是对象, **不**是字符串
```

PlotCraft verified: with this example block, deepseek-v4-flash started
emitting `content=""` + properly-shaped `tool_calls` reliably.

### 4. The silently-abandon protocol (player-led UX)

When the player **declines** a tool result (cancels the "Do you want to
use A?" prompt), the simplest UI choice is "stop calling the LLM and let
the player type." But the OpenAI/Anthropic protocol requires
`assistant tool_calls` to be paired with a `tool tool_call_id` message —
otherwise the model reports "No tool output found" on the next round.

**Anti-pattern** (PlotCraft v0.4.4+): clear `tool_calls` field and
overwrite `content` with "玩家放弃". This makes the LLM lose the tool
context and may "guess" what to do (e.g. silently write a doc item).

**Recommended pattern** (PlotCraft v0.5+): keep `tool_calls`, overwrite
`content` to short "玩家放弃这批备选，等玩家打字。", and **temporarily**
insert a `role: 'tool'` message into the LLM-bound `messages[]` stream
(not into `chatHistories` — that would double-render in the UI). The
"temp insert" can be a function that scans `chatHistories` for
"silently-abandoned" assistant messages and emits the matching
`role: 'tool'` content right after them. The function takes the
`itemId`, reads `chatHistories[itemId]`, and returns the augmented
array.

UI side: when `role: 'tool'` messages exist, render them **only** as
content of the matching assistant tool-question bubble ("✓ 已答"
label) — not as a standalone bubble. Decorator hint: `if (msg.role
=== 'tool') return []` in your `decorated` computed.

### Source

These patterns come from `D:/Projects/PlotCraft` (the PlotCraft v0.5+
chat UX feedback loop, August 2026). PlotCraft is a `laipe` fork that
implements the patterns in its `src/stores/chat.ts` SYSTEM_PROMPT and
`src/stores/concept.ts` / `world.ts` step-chat paths. The patterns are
LLM-agnostic — verified on deepseek-v4-flash; recommended for any
model that shows "helpful assistant" over-training.
