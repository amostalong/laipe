// laipe-app settings wrapper (PlotCraft-equivalent).
//
// 镜像 PlotCraft `src/lib/settings.ts` —— ApiFormat / EffortLevel / labels / endpoints.
// 注意：字段命名 camelCase (跟 PlotCraft / Locus 一致), 跟 v0.1 snake_case 的
// useProviderConfig 不同 (v0.2+ 已迁移, 见 useProviderConfig.ts 的 migrateV1).
//
// PlotCraft 差异 (laipe-app 简化):
// - 没有完整的 Locus `AppConfig` 字段 (laipe-app 是 starter, 不做 Unity / LSP 等)
// - 不接 keychain (apiKey 裸存 localStorage, 由 useProviderConfig 持久化)
// - 没有 PlotCraft `Config.apiKey` / `ui.theme` / `recentProjects` 顶层字段
//   (laipe-app 走 useProviderConfig 替代这些)

import type { ApiFormat, EffortLevel } from "laipe-ts";

// Re-export from laipe-ts (canonical source)
export type { ApiFormat, EffortLevel };

/** Default apiFormat for new providers */
export const DEFAULT_API_FORMAT: ApiFormat = "openai_chat";

/** Default effort (no reasoning / thinking controls) */
export const DEFAULT_EFFORT: EffortLevel = "none";

/** Effort 顺序 (用于 UI 排序 + 推断默认) */
export const EFFORT_ORDER: readonly EffortLevel[] = [
  "none",
  "low",
  "medium",
  "high",
  "xhigh",
  "max",
] as const;

/** Effort 人类可读 label (跟 Locus / PlotCraft CamelCase 一字一致) */
export const EFFORT_LABELS: Record<EffortLevel, string> = {
  none: "None",
  low: "Low",
  medium: "Med",
  high: "High",
  xhigh: "XHigh",
  max: "Max",
};

/** ApiFormat 人类可读 label (UI dropdown 用) */
export const API_FORMAT_LABELS: Record<ApiFormat, string> = {
  openai_chat: "OpenAI Chat Completions",
  openai_responses: "OpenAI Responses API",
  anthropic_messages: "Anthropic Messages",
};

/** ApiFormat → 默认 base URL (玩家首次切到该 format 时建议填的 endpoint) */
export const DEFAULT_ENDPOINTS: Record<ApiFormat, string> = {
  openai_chat: "https://api.openai.com/v1",
  openai_responses: "https://api.openai.com/v1",
  anthropic_messages: "https://api.anthropic.com",
};

/**
 * Map EffortLevel → OpenAI Chat/Responses `reasoning_effort` / `reasoning.effort` value.
 * 1:1 mirror of `laipe_core::EffortLevel::to_openai_effort` and
 * `laipe_ts::protocols::openaiChat::openaiEffortString`.
 *
 * - `none` / `xhigh` / `max` → `null` (字段不写; OpenAI 不支持 xhigh/max)
 * - `low` / `medium` / `high` → 字符串原样
 */
export function openaiEffortString(effort: EffortLevel): "low" | "medium" | "high" | null {
  switch (effort) {
    case "low":
      return "low";
    case "medium":
      return "medium";
    case "high":
      return "high";
    case "none":
    case "xhigh":
    case "max":
      return null;
  }
}

/**
 * Map EffortLevel → Anthropic `thinking.budget_tokens` value.
 * 1:1 mirror of `laipe_core::EffortLevel::to_anthropic_budget` and
 * `laipe_ts::protocols::anthropic::anthropicBudgetForEffort`.
 *
 * - `none` → 0 (字段不写)
 * - `low` → 1024
 * - `medium` → 4096
 * - `high` → 16384
 * - `xhigh` → 32768
 * - `max` → 65536
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
