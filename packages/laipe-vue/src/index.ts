// laipe-vue — Vue 3 components for building agent clients on laipe.
//
// Three layers, each usable independently:
//
//   Primitives (no state, pure presentation):
//     - MessageBubble — a single chat message
//     - MessageInput  — input row with send/stop button
//     - EmptyState    — onboarding state with sample prompts
//     - IconButton    — small icon-only button
//
//   Composites (assemble primitives):
//     - ChatView      — message list + input
//     - Sidebar       — multi-conversation list
//     - SettingsModal — provider config form
//
//   Batteries-included:
//     - AiChatPanel   — full state-managed chat UI in one component
//
//   Streams (injectable sources of chat events):
//     - tauriStream, fetchStream, mockStream, defaultStreamSource()
//
//   Composables (state + business logic):
//     - useChat, useConfig, useConversations
//
// Usage:
//
//   Quick path (one-liner):
//     <AiChatPanel :config="cfg" @error="onError" />
//
//   Custom composition (full control):
//     <ChatView :messages="msgs" :status="status" @send="onSend" @cancel="onCancel" />
//     <Sidebar :conversations="..." :current-id="..." @select="..." />
//     <SettingsModal v-model:open="open" v-model="config" />

// Components
export {
  AiChatPanel,
  ChatView,
  Sidebar,
  SettingsModal,
  ConsolePanel,
  TabsBar,
  MessageBubble,
  MessageInput,
  EmptyState,
  IconButton,
  ToolCallCard,
} from "./components";

// Composables
export {
  useChat,
  useConfig,
  useConversations,
  setConfigStorage,
  whenConfigReady,
  localStorageConfig,
} from "./composables";
export type { ChatStatus, ConfigStorage, AgentSettings } from "./composables";
export type { Conversation } from "./composables/useConversations";

// Streams
export {
  tauriStream,
  fetchStream,
  mockStream,
  defaultStreamSource,
} from "./streams";
export type { StreamSource } from "./streams";

// Tabs
export { useTabs } from "./lib/tabs";
export type { Tab, TabsState } from "./lib/tabs";

// Debug console
export {
  initConsole,
  clearConsole,
  refreshConsole,
  installConsoleHook,
  useConsoleEntries,
  saveReport,
  getDiagnosticConfig,
  setDiagnosticConfig,
} from "./console";
export type {
  ChatErrorKind,
  ConsoleEntry,
  ConsoleLevel,
  ConsoleSource,
  DiagnosticConfig,
  SavedReport,
} from "./console";
