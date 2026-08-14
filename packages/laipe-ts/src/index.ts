// Public surface of laipe-ts. 1:1 mirror of the Rust public API plus the
// browser/Node-native `AbortSignal` for cancellation.

export * from "./types.js";
export * from "./errors.js";
export { SseParser } from "./sse.js";
export type { SseFrame } from "./sse.js";
export { dispatchStream } from "./dispatch.js";
export type { DispatchOptions } from "./dispatch.js";
export { testProvider } from "./test.js";
export {
  askFreeTextSchema,
  askUserQuestionSchema,
  builtinMetaByName,
  builtinTools,
  updateDocItemSchema,
  BUILTIN_TOOL_META,
  BUILTIN_TOOL_NAMES,
} from "./builtin-tools.js";
export type {
  BuiltinToolMeta,
  BuiltinToolName,
  ToolPermission,
  ToolRisk,
} from "./builtin-tools.js";
