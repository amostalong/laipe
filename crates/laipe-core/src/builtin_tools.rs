//! Built-in tool schemas — the 3 canonical patterns laipe ships in lib.
//!
//! These are the JSON schemas the LLM sees (sent on the wire as the
//! `tools` field, translated per protocol by `laipe-streaming`). They
//! correspond 1:1 to the 3 patterns in `docs/TOOL_CALLING.md §Built-in
//! patterns`. Apps register a handler for each (Rust side: see
//! `laipe-app/src-tauri/src/lib.rs execute_tool`); the schema is just
//! what the LLM uses to decide when to call.
//!
//! Why these 3? They cover the 3 universal tool shapes an LLM agent
//! needs for player-led UX:
//! 1. **Branching decision** (`ask_user_question`) — surface 2-5
//!    options, player picks one, answer goes back as `role: tool`.
//! 2. **Open-ended probe** (`ask_free_text`) — ask one question, player
//!    types a free-form answer.
//! 3. **Document write** (`update_doc_item`) — propose a doc edit,
//!    player confirms, edit applied (default permission `ask`).
//!
//! Pattern (4) and onward (silently-abandon protocol, 1 round 1 tool
//! call, few-shot example) are system-prompt-level concerns, not
//! schema-level — see `docs/TOOL_CALLING.md §Patterns from the field`
//! for those.
//!
//! Default permissions (`auto` / `ask` / `deny`) come from PlotCraft v0.5+
//! production tuning: the two read-only / probe tools default to `auto`
//! (LLM is allowed to fire them without a confirmation click), and the
//! document write defaults to `ask` (player must confirm before the
//! editor is touched). Apps can override per-tool in
//! `AgentSettings.toolPermissions` (laipe-vue wires this through).
//!
//! The `BuiltinTool::as_str()` field is the discriminator — it MUST
//! match the schema's `function.name` exactly. The TS layer mirrors
//! this in `packages/laipe-ts/src/builtin-tools.ts`.

use serde::{Deserialize, Serialize};

use crate::tool::{ToolDefinition, ToolFunction, ToolType};

/// Canonical tool name (one per built-in schema). Kept as a Rust enum
/// (not a `&'static str`) so call sites can do exhaustive matching
/// and the compiler catches typos when a new tool is added.
///
/// Wire-format name is via `BuiltinTool::as_str()` — must match
/// `ToolDefinition.function.name`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BuiltinTool {
    AskUserQuestion,
    AskFreeText,
    UpdateDocItem,
}

impl BuiltinTool {
    /// Wire-format name. Must match `ToolDefinition.function.name`.
    pub const fn as_str(self) -> &'static str {
        match self {
            BuiltinTool::AskUserQuestion => "ask_user_question",
            BuiltinTool::AskFreeText => "ask_free_text",
            BuiltinTool::UpdateDocItem => "update_doc_item",
        }
    }
}

impl std::fmt::Display for BuiltinTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Default per-tool permission. Apps override via
/// `AgentSettings.toolPermissions` (laipe-vue) or equivalent.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolPermission {
    /// LLM-emitted call runs immediately; no player click required.
    /// Default for read-only / probe tools.
    #[default]
    Auto,
    /// LLM-emitted call parks on a oneshot until the player clicks
    /// Approve. Default for write tools.
    Ask,
    /// LLM-emitted call is rejected with a denial result.
    Deny,
}

/// Per-tool metadata: name + display label + description + risk +
/// default permission. Drives the `ToolsSettings` panel (laipe-vue).
#[derive(Debug, Clone)]
pub struct BuiltinToolMeta {
    pub name: BuiltinTool,
    pub label: &'static str,
    pub description: &'static str,
    pub risk: ToolRisk,
    pub default_permission: ToolPermission,
}

/// Coarse risk band for UI hint (Settings panel, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolRisk {
    /// Read-only or probe: changes nothing on disk.
    Low,
    /// Writes a project artifact (doc item, file, etc.). Needs the
    /// default `ask` permission at minimum.
    Medium,
    /// Destructive or external side-effect (delete, network call,
    /// etc.). Apps should default to `ask` or `deny`.
    High,
}

/// Static metadata table for the 3 built-in tools.
pub const BUILTIN_TOOL_META: &[BuiltinToolMeta] = &[
    BuiltinToolMeta {
        name: BuiltinTool::AskUserQuestion,
        label: "Ask User Question",
        description: "Surface 2-5 mutually-exclusive options; player picks one, answer fed back as a `role: tool` message.",
        risk: ToolRisk::Low,
        default_permission: ToolPermission::Auto,
    },
    BuiltinToolMeta {
        name: BuiltinTool::AskFreeText,
        label: "Ask Free Text",
        description: "Ask one open-ended question; player types a free-form answer in the composer (or an inline input).",
        risk: ToolRisk::Low,
        default_permission: ToolPermission::Auto,
    },
    BuiltinToolMeta {
        name: BuiltinTool::UpdateDocItem,
        label: "Update Doc Item",
        description: "Propose a document edit (item_id + content + optional mode). Player must confirm before the editor is touched.",
        risk: ToolRisk::Medium,
        default_permission: ToolPermission::Ask,
    },
];

/// Look up metadata for a tool by wire-format name.
///
/// Returns `None` if the name is not a built-in (apps can register
/// additional custom tools with arbitrary names).
pub fn builtin_meta_by_name(name: &str) -> Option<&'static BuiltinToolMeta> {
    BUILTIN_TOOL_META.iter().find(|m| m.name.as_str() == name)
}

// === Schemas ===
//
// All 3 follow the canonical OpenAI Chat Completions tool shape
// (`{type: "function", function: {name, description, parameters}}`).
// Wire translation to OpenAI Responses / Anthropic happens inside
// `laipe-streaming/build_openai_responses_body` / `build_anthropic_request_body`.

/// Schema for `ask_user_question` — the LLM surfaces 2-5 options for
/// the player to pick from.
///
/// `options` items MUST be objects with `label` (≤10 chars, shown as
/// the card header) and `preview` (the full option body). `description`
/// is optional (hover-tooltip detail).
pub fn ask_user_question_schema() -> ToolDefinition {
    ToolDefinition {
        kind: ToolType::Function,
        function: ToolFunction {
            name: BuiltinTool::AskUserQuestion.as_str().to_string(),
            description: concat!(
                "向玩家提出一个多选问题，提供 2-5 个互斥的备选方案让 ta 选。",
                "适合给方向、选项、取舍。",
                "**只**用于「问问题」场景；不适合问开放性问题（用 ask_free_text）。",
            )
            .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "question": {
                        "type": "string",
                        "description": "向玩家展示的 1 句问题（会显示在 AltCard 顶部）"
                    },
                    "options": {
                        "type": "array",
                        "minItems": 2,
                        "maxItems": 5,
                        "items": {
                            "type": "object",
                            "properties": {
                                "label": {
                                    "type": "string",
                                    "maxLength": 10,
                                    "description": "卡片 header (≤10 字)"
                                },
                                "preview": {
                                    "type": "string",
                                    "description": "完整备选内容（玩家采用后写入编辑器 / 选中的内容）"
                                },
                                "description": {
                                    "type": "string",
                                    "description": "可选 hover-tooltip 详情（不参与主流程）"
                                }
                            },
                            "required": ["label", "preview"],
                            "additionalProperties": false,
                        },
                        "description": "2-5 个互斥备选方案"
                    }
                },
                "required": ["question", "options"],
                "additionalProperties": false,
            }),
        },
    }
}

/// Schema for `ask_free_text` — the LLM asks one open-ended question
/// the player must answer in their own words (no options provided).
pub fn ask_free_text_schema() -> ToolDefinition {
    ToolDefinition {
        kind: ToolType::Function,
        function: ToolFunction {
            name: BuiltinTool::AskFreeText.as_str().to_string(),
            description: concat!(
                "向玩家提出一个需要 ta 自己想清楚的开放问题。",
                "**不要**给选项 — 这种问题没有标准答案，要让玩家自己想。",
                "适合反思类追问、深度确认。",
                "跟 ask_user_question 的区别：ask_user_question 给方向性备选让玩家挑；ask_free_text 是真正需要玩家自己想的开放问题。",
            )
            .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "question": {
                        "type": "string",
                        "description": "向玩家展示的 1 句开放问题（必填，不要用编号拆多个）"
                    }
                },
                "required": ["question"],
                "additionalProperties": false,
            }),
        },
    }
}

/// Schema for `update_doc_item` — the LLM proposes a document edit;
/// the player must confirm before the editor is touched.
///
/// `mode`:
///  - `replace` (default) — overwrites the current content of the
///    `item_id`
///  - `append` — appends to the current content (kept as the last
///    paragraph / section)
pub fn update_doc_item_schema() -> ToolDefinition {
    ToolDefinition {
        kind: ToolType::Function,
        function: ToolFunction {
            name: BuiltinTool::UpdateDocItem.as_str().to_string(),
            description: concat!(
                "把玩家选定 / 修改后的内容写入文档某一项。",
                "**只**在玩家已经明确表达过要这个方案时调（例如玩家问「用 A 改暗版」，或玩家从 ask_user_question 选了一个 option）。",
                "**不要**在没确认的情况下主动调这个 — 玩家主导，绝不替玩家做决定。",
                "**反思 / 提问 / 解释**类输出**不要**用这个 tool — 反思用 ask_free_text（开放问题）或 ask_user_question（让玩家挑选项）。",
            )
            .to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "item_id": {
                        "type": "string",
                        "description": "要写入的文档项 id（由 app 定义；例如 PlotCraft 的概念 7 步：seed / pillars / world-rules / locations / character-functions / three-act / core-fantasy）"
                    },
                    "content": {
                        "type": "string",
                        "description": "最终内容（玩家改过的优先于 LLM 原始备选）"
                    },
                    "mode": {
                        "type": "string",
                        "enum": ["replace", "append"],
                        "default": "replace",
                        "description": "replace = 覆盖当前内容；append = 追加到末尾"
                    }
                },
                "required": ["item_id", "content"],
                "additionalProperties": false,
            }),
        },
    }
}

/// Return all 3 built-in tool schemas in canonical order. Apps usually
/// spread this into their own `TOOLS` list:
///
/// ```ignore
/// use laipe_core::builtin_tools::builtin_tools;
/// const MY_TOOLS: &[ToolDefinition] = &[
///     &GET_CURRENT_TIME, &ECHO,
///     ..builtin_tools_vec(),
/// ];
/// ```
///
/// (Caller-side concat is easier in a `const` context than
/// `Vec::new()` of a const value; that's why this returns `Vec`
/// instead of `&'static [ToolDefinition]`.)
pub fn builtin_tools() -> Vec<ToolDefinition> {
    vec![
        ask_user_question_schema(),
        ask_free_text_schema(),
        update_doc_item_schema(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- BuiltinTool::as_str() ---

    #[test]
    fn as_str_matches_wire_format() {
        assert_eq!(BuiltinTool::AskUserQuestion.as_str(), "ask_user_question");
        assert_eq!(BuiltinTool::AskFreeText.as_str(), "ask_free_text");
        assert_eq!(BuiltinTool::UpdateDocItem.as_str(), "update_doc_item");
    }

    #[test]
    fn as_str_is_consistent_with_display() {
        for t in [
            BuiltinTool::AskUserQuestion,
            BuiltinTool::AskFreeText,
            BuiltinTool::UpdateDocItem,
        ] {
            assert_eq!(t.as_str(), format!("{}", t).as_str());
        }
    }

    // --- builtin_meta_by_name ---

    #[test]
    fn meta_by_name_finds_known_tools() {
        assert!(builtin_meta_by_name("ask_user_question").is_some());
        assert!(builtin_meta_by_name("ask_free_text").is_some());
        assert!(builtin_meta_by_name("update_doc_item").is_some());
    }

    #[test]
    fn meta_by_name_returns_none_for_unknown() {
        assert!(builtin_meta_by_name("not_a_tool").is_none());
        assert!(builtin_meta_by_name("").is_none());
        // 大小写敏感 — 协议是 wire-format 严格匹配
        assert!(builtin_meta_by_name("Ask_User_Question").is_none());
    }

    #[test]
    fn meta_default_permissions_match_design() {
        // ask_user_question / ask_free_text = Auto (read-only / probe)
        // update_doc_item = Ask (write, 玩家确认)
        let ask_uq = builtin_meta_by_name("ask_user_question").unwrap();
        assert_eq!(ask_uq.default_permission, ToolPermission::Auto);
        assert_eq!(ask_uq.risk, ToolRisk::Low);

        let ask_ft = builtin_meta_by_name("ask_free_text").unwrap();
        assert_eq!(ask_ft.default_permission, ToolPermission::Auto);
        assert_eq!(ask_ft.risk, ToolRisk::Low);

        let upd = builtin_meta_by_name("update_doc_item").unwrap();
        assert_eq!(upd.default_permission, ToolPermission::Ask);
        assert_eq!(upd.risk, ToolRisk::Medium);
    }

    // --- builtin_tools() ---

    #[test]
    fn builtin_tools_returns_three_in_canonical_order() {
        let tools = builtin_tools();
        assert_eq!(tools.len(), 3);
        assert_eq!(tools[0].function.name, "ask_user_question");
        assert_eq!(tools[1].function.name, "ask_free_text");
        assert_eq!(tools[2].function.name, "update_doc_item");
    }

    #[test]
    fn builtin_tools_names_match_meta_table() {
        let tools = builtin_tools();
        let meta_names: Vec<&str> = BUILTIN_TOOL_META.iter().map(|m| m.name.as_str()).collect();
        let tool_names: Vec<&str> = tools.iter().map(|t| t.function.name.as_str()).collect();
        assert_eq!(meta_names, tool_names);
    }

    // --- Schemas: 参数结构 + JSON 合法 ---

    #[test]
    fn all_schemas_serialize_to_valid_json() {
        for t in builtin_tools() {
            let s = serde_json::to_string(&t).expect("schema must serialize");
            let back: ToolDefinition = serde_json::from_str(&s).expect("schema must round-trip");
            assert_eq!(back.function.name, t.function.name);
        }
    }

    #[test]
    fn ask_user_question_schema_requires_question_and_options() {
        let s = ask_user_question_schema();
        let params = &s.function.parameters;
        assert_eq!(params["type"], "object");
        let required = params["required"]
            .as_array()
            .expect("required must be array");
        let required_strs: Vec<&str> = required.iter().filter_map(|v| v.as_str()).collect();
        assert!(required_strs.contains(&"question"));
        assert!(required_strs.contains(&"options"));

        // options: 2-5
        let options = &params["properties"]["options"];
        assert_eq!(options["minItems"], 2);
        assert_eq!(options["maxItems"], 5);

        // items: label + preview required
        let items = &options["items"];
        let item_required: Vec<&str> = items["required"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert!(item_required.contains(&"label"));
        assert!(item_required.contains(&"preview"));

        // label maxLength 10
        assert_eq!(items["properties"]["label"]["maxLength"], 10);

        // additionalProperties: false 严格
        assert_eq!(params["additionalProperties"], false);
        assert_eq!(items["additionalProperties"], false);
    }

    #[test]
    fn ask_free_text_schema_requires_only_question() {
        let s = ask_free_text_schema();
        let params = &s.function.parameters;
        let required: Vec<&str> = params["required"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert_eq!(required, vec!["question"]);
        assert_eq!(params["additionalProperties"], false);
    }

    #[test]
    fn update_doc_item_schema_requires_item_id_and_content() {
        let s = update_doc_item_schema();
        let params = &s.function.parameters;
        let required: Vec<&str> = params["required"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert!(required.contains(&"item_id"));
        assert!(required.contains(&"content"));

        // mode: enum replace/append, default replace
        let mode = &params["properties"]["mode"];
        let mode_enum: Vec<&str> = mode["enum"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert_eq!(mode_enum, vec!["replace", "append"]);
        assert_eq!(mode["default"], "replace");

        assert_eq!(params["additionalProperties"], false);
    }

    #[test]
    fn all_schema_names_match_enum_as_str() {
        let tools = builtin_tools();
        for t in tools {
            // name 在 schema 跟 enum 都对得上
            let meta = builtin_meta_by_name(&t.function.name)
                .expect("every schema must have a meta entry");
            assert_eq!(meta.name.as_str(), t.function.name);
        }
    }

    // --- ToolPermission default ---

    #[test]
    fn tool_permission_default_is_auto() {
        assert_eq!(ToolPermission::default(), ToolPermission::Auto);
    }
}
