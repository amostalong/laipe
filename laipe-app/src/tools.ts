// Sample tool definitions for laipe-app.
//
// These are the *schemas* the LLM sees. The Rust side has a parallel
// `execute_tool(name, args_json)` in src-tauri/src/lib.rs that runs the
// actual work. When the LLM decides to call a tool, laipe streams the
// call back to the frontend (`message.tool_calls`), the Rust backend
// executes it, and the result is fed back into the conversation as a
// `role: tool` message before the next turn.
//
// Add new tools in three places:
//   1. here: the schema the LLM uses to decide when to call
//   2. lib.rs `execute_tool`: the implementation
//   3. (optional) lib.rs dispatch: any side-effects (logging, etc.)
//
// Production apps would put the tool set behind env config or per-user
// capability. For the starter we keep it hard-coded to two demo tools.

import type { ToolDefinition } from "laipe-ts";

/**
 * `get_current_time` — server-side UTC clock.
 * Useful for date-relative prompts ("what's today's date?").
 */
const GET_CURRENT_TIME: ToolDefinition = {
  type: "function",
  function: {
    name: "get_current_time",
    description:
      "Return the current UTC time as an RFC-3339 string. Use this whenever the user asks about 'now', 'today', or any time-relative question.",
    parameters: {
      type: "object",
      properties: {},
      required: [],
    },
  },
};

/**
 * `echo` — round-trip the LLM's own arguments back.
 * Useful for debugging the tool-calling pipeline end-to-end: the
 * frontend sees the call, the backend sees the result, the LLM
 * sees its own call repeated.
 */
const ECHO: ToolDefinition = {
  type: "function",
  function: {
    name: "echo",
    description:
      "Echo back the supplied text. Useful for verifying that the tool-calling pipeline is wired correctly end-to-end.",
    parameters: {
      type: "object",
      properties: {
        text: {
          type: "string",
          description: "The text to echo back.",
        },
      },
      required: ["text"],
    },
  },
};

export const TOOLS: ToolDefinition[] = [GET_CURRENT_TIME, ECHO];
