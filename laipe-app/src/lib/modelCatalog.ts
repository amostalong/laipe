// PlotCraft v0.1 built-in model catalog
//
// 1:1 mirror of PlotCraft `src/lib/modelCatalog.ts` —— 跟 laipe-ts `ModelCatalog` /
// `CatalogModel` / `CatalogProvider` 类型协同 (Tauri `get_model_catalog` 返回
// `ResolvedCatalog`, 已 slim + 过滤 deprecated / no-tool-call models).
//
// v0.1 内置 fallback 列表 —— Tauri catalog 拉不到时用
// 1 条占位 (claude-sonnet-4-5) 跟 PlotCraft 对齐
//
// 也包含 PlotCraft 的 chat selector 段头 helper:
// `groupCustomProviderShortcuts(customProviders)` 给 ModelEffortSelector 用.

import type { ApiFormat, EffortLevel } from "laipe-ts";
import { EFFORT_LABELS } from "./settings";
import type { CustomProvider } from "../composables/useProviderConfig";

/** BuiltinModel 是 fallback catalog (Tauri catalog 拉不到时用)
 *  PlotCraft v0.1.3+ 简化: 只 1 条 claude-sonnet-4-5 占位
 */
export interface BuiltinModel {
  id: string;
  name: string;
  /** provider 分类 (对应后端 `ApiFormat` 路由) */
  provider: "openai" | "anthropic" | "google" | "custom";
  contextWindow: number;
  /** 默认勾选 / 第一次启动时填进 mainModel (v0.2 不用, 留字段兼容) */
  isDefault?: boolean;
  note?: string;
  /** 该模型支持的 reasoning effort 列表 (空 = 不支持 thinking 控制) */
  supportedEfforts?: EffortLevel[];
  /** 该模型默认的 effort */
  defaultEffort?: EffortLevel;
}

export const BUILTIN_MODELS: readonly BuiltinModel[] = [
  {
    id: "claude-sonnet-4-5",
    name: "Claude Sonnet 4.5",
    provider: "anthropic",
    contextWindow: 200_000,
    note: "200K context · MiniMax 官方主推",
    supportedEfforts: ["none", "low", "medium", "high", "xhigh", "max"],
    defaultEffort: "high",
  },
];

/** 按 id 查 model (找不到返回 undefined) */
export function findModel(id: string): BuiltinModel | undefined {
  return BUILTIN_MODELS.find((m) => m.id === id);
}

/** 拿一个 model 的 supported efforts (强制 `none` 永远在第一位) */
export function getSupportedEfforts(model: BuiltinModel | undefined): EffortLevel[] {
  const all: EffortLevel[] = ["none", "low", "medium", "high", "xhigh", "max"];
  if (!model || !model.supportedEfforts || model.supportedEfforts.length === 0) {
    return all;
  }
  const supported = new Set<EffortLevel>(model.supportedEfforts);
  if (!supported.has("none")) {
    return ["none", ...model.supportedEfforts];
  }
  return model.supportedEfforts;
}

/** 拿一个 model 的默认 effort */
export function getDefaultEffort(model: BuiltinModel | undefined): EffortLevel {
  return model?.defaultEffort ?? "none";
}

// === Chat selector grouping (PlotCraft `ModelSelectorGroup`) ===

/** Selector 用的 model group (PlotCraft `ModelSelectorGroup` 镜像) */
export interface ModelSelectorGroup {
  key: string;
  label: string;
  /** 段头 lowercase 还是 normal case (custom provider 段头大写 UPPERCASE) */
  uppercaseLabel: boolean;
  /** custom provider 段头: 单个 model option (用 provider.defaultModel 当 id) */
  customProvider?: { id: string; name: string; defaultModel: string };
}

/** v0.2+ 玩家 enabled 且有 defaultModel 的 custom provider 各自一个段头
 *  段头 label = provider.name (Locus 风格, UI 渲染时大写)
 *  - effective default: defaultModel || models[0].id
 *  - 空 / 没 model → 不出现 (player 看不到, "Add model" 引导)
 */
export function groupCustomProviderShortcuts(
  customProviders: { id: string; name: string; defaultModel: string; enabled: boolean; models?: { id: string }[] }[],
): ModelSelectorGroup[] {
  return customProviders
    .filter((cp) => {
      if (!cp.enabled) return false;
      const effective = cp.defaultModel?.trim() || cp.models?.[0]?.id?.trim() || "";
      return effective !== "";
    })
    .map((cp) => {
      const effective = cp.defaultModel?.trim() || cp.models?.[0]!.id.trim();
      return {
        key: `custom:${cp.id}`,
        label: cp.name,
        uppercaseLabel: true, // Locus DEEPSEEK / WINKY-XXX 大写段头
        customProvider: {
          id: cp.id,
          name: cp.name,
          defaultModel: effective!,
        },
      };
    });
}

// Re-export for chat UI helper convenience
export { EFFORT_LABELS };

/** Cleanup long model id for display (OpenRouter 风格 + 截断) */
export function cleanupModelId(id: string, maxLen = 24): string {
  const slashIdx = id.lastIndexOf("/");
  const cleaned = slashIdx >= 0 ? id.slice(slashIdx + 1) : id;
  if (cleaned.length <= maxLen) return cleaned;
  return cleaned.slice(0, maxLen - 1) + "…";
}

/** 默认 base url per api format (从 settings.ts 拿, 这里只是 alias) */
export { DEFAULT_ENDPOINTS } from "./settings";

/** 把 `apiFormat` 字符串映射到 `BuiltinModel.provider` (用于 catalog 匹配) */
export function builtinProviderFromFormat(fmt: ApiFormat): BuiltinModel["provider"] {
  switch (fmt) {
    case "anthropic_messages":
      return "anthropic";
    case "openai_chat":
    case "openai_responses":
      return "openai";
  }
}

/** Type guard: CustomProvider 用于 groupCustomProviderShortcuts */
export function asCustomProviderShortcuts(
  providers: CustomProvider[],
): { id: string; name: string; defaultModel: string; enabled: boolean; models?: { id: string }[] }[] {
  return providers.map((p) => ({
    id: p.id,
    name: p.name,
    defaultModel: p.defaultModel,
    enabled: p.enabled,
    models: p.models,
  }));
}
