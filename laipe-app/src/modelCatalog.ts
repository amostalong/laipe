// laipe-app — curated model catalog
//
// Hand-picked common models for the 3 protocols laipe supports.
// For a real product you'd load a remote catalog (e.g. models.dev) and
// cache it on disk; for a starter, a hardcoded list is enough to demo
// the model selector UX without dragging in 100KB+ of model metadata.

import type { ApiFormat, EffortLevel } from "laipe-ts";

export interface ModelInfo {
  /** model id — sent to the LLM API as `model` */
  id: string;
  /** human-readable name */
  name: string;
  /** which protocol(s) this model speaks */
  api_formats: ApiFormat[];
  /** reasoning effort levels the model supports. Empty = unsupported. */
  supported_efforts: EffortLevel[];
  /** default effort when player hasn't picked one. `null` = no reasoning. */
  default_effort: EffortLevel | null;
  /** short hint shown in the dropdown */
  note?: string;
  /** approximate context window in tokens */
  context?: number;
}

/** OpenAI / OpenAI-compatible models (chat completions + responses) */
const OPENAI: ModelInfo[] = [
  {
    id: "gpt-4o",
    name: "GPT-4o",
    api_formats: ["openai_chat", "openai_responses"],
    supported_efforts: ["low", "medium", "high"],
    default_effort: null,
    note: "OpenAI flagship · 128K context",
    context: 128_000,
  },
  {
    id: "gpt-4o-mini",
    name: "GPT-4o mini",
    api_formats: ["openai_chat", "openai_responses"],
    supported_efforts: [],
    default_effort: null,
    note: "Fast + cheap · 128K context",
    context: 128_000,
  },
  {
    id: "o4-mini",
    name: "o4-mini",
    api_formats: ["openai_chat", "openai_responses"],
    supported_efforts: ["low", "medium", "high"],
    default_effort: "medium",
    note: "Reasoning model · 200K context",
    context: 200_000,
  },
  {
    id: "deepseek-chat",
    name: "DeepSeek-V3",
    api_formats: ["openai_chat"],
    supported_efforts: [],
    default_effort: null,
    note: "DeepSeek (OpenAI-compatible endpoint) · 64K context",
    context: 64_000,
  },
  {
    id: "deepseek-reasoner",
    name: "DeepSeek-R1",
    api_formats: ["openai_chat"],
    supported_efforts: [],
    default_effort: null,
    note: "Reasoning · 64K context",
    context: 64_000,
  },
];

/** Anthropic models */
const ANTHROPIC: ModelInfo[] = [
  {
    id: "claude-sonnet-4-5",
    name: "Claude Sonnet 4.5",
    api_formats: ["anthropic_messages"],
    supported_efforts: ["none", "low", "medium", "high", "xhigh", "max"],
    default_effort: null,
    note: "Anthropic flagship · 200K context",
    context: 200_000,
  },
  {
    id: "claude-3-5-haiku-latest",
    name: "Claude 3.5 Haiku",
    api_formats: ["anthropic_messages"],
    supported_efforts: [],
    default_effort: null,
    note: "Fast + cheap · 200K context",
    context: 200_000,
  },
  {
    id: "claude-3-5-sonnet-latest",
    name: "Claude 3.5 Sonnet",
    api_formats: ["anthropic_messages"],
    supported_efforts: [],
    default_effort: null,
    note: "Previous gen · 200K context",
    context: 200_000,
  },
];

export const MODEL_CATALOG: readonly ModelInfo[] = [...OPENAI, ...ANTHROPIC];

/** Models for a given API format (and ordered as curated above). */
export function modelsForFormat(format: ApiFormat): ModelInfo[] {
  return MODEL_CATALOG.filter((m) => m.api_formats.includes(format));
}

/** Look up a model by id, or undefined. */
export function findModel(id: string): ModelInfo | undefined {
  return MODEL_CATALOG.find((m) => m.id === id);
}

/** Strip a long model id (e.g. "openai/gpt-4o-mini" → "gpt-4o-mini") + truncate. */
export function cleanupModelId(id: string, maxLen = 32): string {
  const slash = id.lastIndexOf("/");
  const cleaned = slash >= 0 ? id.slice(slash + 1) : id;
  if (cleaned.length <= maxLen) return cleaned;
  return cleaned.slice(0, maxLen - 1) + "…";
}
