// Built-in tool schemas — the 3 canonical patterns laipe ships in lib.
//
// 1:1 mirror of `crates/laipe-core/src/builtin_tools.rs` (the schema
// payloads are the same JSON, the Rust types are TS enums).
//
// Apps usually spread `builtin_tools()` into their own `TOOLS` list:
//
// ```ts
// import { builtin_tools } from "laipe-ts";
// export const TOOLS: ToolDefinition[] = [
//   ...myDemoTools,
//   ...builtin_tools(),
// ];
// ```
//
// Why these 3? They cover the 3 universal tool shapes an LLM agent
// needs for player-led UX:
//  1. **Branching decision** (`ask_user_question`) — surface 2-5
//     options, player picks one, answer goes back as `role: tool`.
//  2. **Open-ended probe** (`ask_free_text`) — ask one question,
//     player types a free-form answer.
//  3. **Document write** (`update_doc_item`) — propose a doc edit,
//     player confirms, edit applied (default permission `ask`).
//
// Default permissions come from PlotCraft v0.5+ production tuning:
// the two read-only / probe tools default to `auto`, the document
// write defaults to `ask`. Apps can override per-tool in
// `AgentSettings.toolPermissions`.

import type { ToolDefinition } from "./types.js";

/** Canonical tool name. Wire-format string MUST match the schema's
 *  `function.name`. Use `BuiltinToolSchema` for type-safe reference. */
export const BUILTIN_TOOL_NAMES = [
  "ask_user_question",
  "ask_free_text",
  "update_doc_item",
] as const;

export type BuiltinToolName = typeof BUILTIN_TOOL_NAMES[number];

/** Default per-tool permission. Apps override via
 *  `AgentSettings.toolPermissions` (laipe-vue) or equivalent. */
export type ToolPermission = "auto" | "ask" | "deny";

/** Coarse risk band for UI hint (Settings panel, etc.). */
export type ToolRisk = "low" | "medium" | "high";

/** Per-tool metadata. */
export interface BuiltinToolMeta {
  readonly name: BuiltinToolName;
  readonly label: string;
  readonly description: string;
  readonly risk: ToolRisk;
  readonly defaultPermission: ToolPermission;
}

/** Static metadata table for the 3 built-in tools. */
export const BUILTIN_TOOL_META: readonly BuiltinToolMeta[] = [
  {
    name: "ask_user_question",
    label: "Ask User Question",
    description:
      "Surface 2-5 mutually-exclusive options; player picks one, answer fed back as a `role: tool` message.",
    risk: "low",
    defaultPermission: "auto",
  },
  {
    name: "ask_free_text",
    label: "Ask Free Text",
    description:
      "Ask one open-ended question; player types a free-form answer in the composer (or an inline input).",
    risk: "low",
    defaultPermission: "auto",
  },
  {
    name: "update_doc_item",
    label: "Update Doc Item",
    description:
      "Propose a document edit (item_id + content + optional mode). Player must confirm before the editor is touched.",
    risk: "medium",
    defaultPermission: "ask",
  },
] as const;

/** Look up metadata for a tool by wire-format name. */
export function builtinMetaByName(
  name: string,
): BuiltinToolMeta | undefined {
  return BUILTIN_TOOL_META.find((m) => m.name === name);
}

// === Schemas ===

/** Schema for `ask_user_question` — the LLM surfaces 2-5 options
 *  for the player to pick from. */
export function askUserQuestionSchema(): ToolDefinition {
  return {
    type: "function",
    function: {
      name: "ask_user_question",
      description:
        "向玩家提出一个多选问题，提供 2-5 个互斥的备选方案让 ta 选。适合给方向、选项、取舍。**只**用于「问问题」场景；不适合问开放性问题（用 ask_free_text）。",
      parameters: {
        type: "object",
        properties: {
          question: {
            type: "string",
            description: "向玩家展示的 1 句问题（会显示在 AltCard 顶部）",
          },
          options: {
            type: "array",
            minItems: 2,
            maxItems: 5,
            items: {
              type: "object",
              properties: {
                label: {
                  type: "string",
                  maxLength: 10,
                  description: "卡片 header (≤10 字)",
                },
                preview: {
                  type: "string",
                  description:
                    "完整备选内容（玩家采用后写入编辑器 / 选中的内容）",
                },
                description: {
                  type: "string",
                  description: "可选 hover-tooltip 详情（不参与主流程）",
                },
              },
              required: ["label", "preview"],
              additionalProperties: false,
            },
            description: "2-5 个互斥备选方案",
          },
        },
        required: ["question", "options"],
        additionalProperties: false,
      },
    },
  };
}

/** Schema for `ask_free_text` — the LLM asks one open-ended question
 *  the player must answer in their own words. */
export function askFreeTextSchema(): ToolDefinition {
  return {
    type: "function",
    function: {
      name: "ask_free_text",
      description:
        "向玩家提出一个需要 ta 自己想清楚的开放问题。**不要**给选项 — 这种问题没有标准答案，要让玩家自己想。适合反思类追问、深度确认。跟 ask_user_question 的区别：ask_user_question 给方向性备选让玩家挑；ask_free_text 是真正需要玩家自己想的开放问题。",
      parameters: {
        type: "object",
        properties: {
          question: {
            type: "string",
            description: "向玩家展示的 1 句开放问题（必填，不要用编号拆多个）",
          },
        },
        required: ["question"],
        additionalProperties: false,
      },
    },
  };
}

/** Schema for `update_doc_item` — the LLM proposes a document edit;
 *  the player must confirm before the editor is touched. */
export function updateDocItemSchema(): ToolDefinition {
  return {
    type: "function",
    function: {
      name: "update_doc_item",
      description:
        "把玩家选定 / 修改后的内容写入文档某一项。**只**在玩家已经明确表达过要这个方案时调（例如玩家问「用 A 改暗版」，或玩家从 ask_user_question 选了一个 option）。**不要**在没确认的情况下主动调这个 — 玩家主导，绝不替玩家做决定。**反思 / 提问 / 解释**类输出**不要**用这个 tool — 反思用 ask_free_text（开放问题）或 ask_user_question（让玩家挑选项）。",
      parameters: {
        type: "object",
        properties: {
          item_id: {
            type: "string",
            description:
              "要写入的文档项 id（由 app 定义；例如 PlotCraft 的概念 7 步：seed / pillars / world-rules / locations / character-functions / three-act / core-fantasy）",
          },
          content: {
            type: "string",
            description: "最终内容（玩家改过的优先于 LLM 原始备选）",
          },
          mode: {
            type: "string",
            enum: ["replace", "append"],
            default: "replace",
            description: "replace = 覆盖当前内容；append = 追加到末尾",
          },
        },
        required: ["item_id", "content"],
        additionalProperties: false,
      },
    },
  };
}

/** All 3 built-in tool schemas in canonical order. */
export function builtinTools(): ToolDefinition[] {
  return [
    askUserQuestionSchema(),
    askFreeTextSchema(),
    updateDocItemSchema(),
  ];
}
