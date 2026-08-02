// useConversations — multi-conversation state, persisted to localStorage.
// Each conversation has its own message history; the ProviderConfig is
// global (in useConfig) and shared across conversations.

import { ref, computed, watch } from "vue";
import type { ChatMessage } from "laipe-ts";

const STORAGE_KEY = "laipe.conversations.v1";

export interface Conversation {
  id: string;
  title: string;
  createdAt: number;
  updatedAt: number;
  messages: ChatMessage[];
}

function newId(): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }
  return `c_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 10)}`;
}

function loadConversations(): Conversation[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw) as Conversation[];
      if (Array.isArray(parsed)) return parsed;
    }
  } catch {
    /* fall through */
  }
  return [];
}

const conversations = ref<Conversation[]>(loadConversations());
const currentId = ref<string | null>(conversations.value[0]?.id ?? null);

watch(
  conversations,
  (v) => {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(v));
    } catch {
      /* private mode */
    }
  },
  { deep: true },
);

function deriveTitle(messages: ChatMessage[]): string {
  const firstUser = messages.find((m) => m.role === "user");
  if (!firstUser) return "New chat";
  const text = firstUser.content.trim().split("\n")[0] ?? "New chat";
  return text.length > 40 ? `${text.slice(0, 40)}…` : text;
}

/**
 * Reactive multi-conversation state with localStorage persistence.
 *
 * One global conversation list (singleton — all `useConversations()` calls
 * share the same data). Each conversation has its own message history; the
 * `ProviderConfig` is global (in `useConfig`) and shared across conversations.
 *
 * The default storage is `localStorage` (sync, ships with the starter).
 * To swap to a different storage (Tauri command, SQLite, etc.) implement
 * a `ConfigStorage`-style adapter and patch the watcher — the same
 * pattern used in `useConfig`. v0.2 will add a `ConversationStorage`
 * interface mirroring `ConfigStorage`.
 *
 * @returns `{ conversations, currentId, current, create, select, remove, clearAll, setMessages, touch }`
 *
 * @example
 * ```ts
 * const conv = useConversations();
 * conv.create();
 * conv.setMessages([{ role: 'user', content: 'hi' }]);
 * // ...later, in another component:
 * const sameConv = useConversations();  // shares state
 * console.log(sameConv.current.value?.messages);
 * ```
 */
export function useConversations() {
  const current = computed<Conversation | null>(() => {
    if (!currentId.value) return null;
    return conversations.value.find((c) => c.id === currentId.value) ?? null;
  });

  function create(): Conversation {
    const conv: Conversation = {
      id: newId(),
      title: "New chat",
      createdAt: Date.now(),
      updatedAt: Date.now(),
      messages: [],
    };
    conversations.value.unshift(conv);
    currentId.value = conv.id;
    return conv;
  }

  function select(id: string): void {
    if (conversations.value.some((c) => c.id === id)) {
      currentId.value = id;
    }
  }

  function remove(id: string): void {
    const idx = conversations.value.findIndex((c) => c.id === id);
    if (idx === -1) return;
    conversations.value.splice(idx, 1);
    if (currentId.value === id) {
      currentId.value = conversations.value[0]?.id ?? null;
    }
  }

  function clearAll(): void {
    conversations.value = [];
    currentId.value = null;
  }

  /**
   * Update the message list of the current conversation. If no conversation
   * exists, create one. Returns the (possibly new) conversation id.
   */
  function setMessages(messages: ChatMessage[]): string {
    let conv = current.value;
    if (!conv) {
      conv = create();
    }
    conv.messages = messages;
    conv.updatedAt = Date.now();
    if (conv.title === "New chat" && messages.some((m) => m.role === "user")) {
      conv.title = deriveTitle(messages);
    }
    return conv.id;
  }

  function touch(): void {
    const conv = current.value;
    if (conv) conv.updatedAt = Date.now();
  }

  return {
    conversations,
    currentId,
    current,
    create,
    select,
    remove,
    clearAll,
    setMessages,
    touch,
  };
}
