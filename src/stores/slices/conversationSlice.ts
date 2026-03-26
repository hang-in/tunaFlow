import { invoke } from "@tauri-apps/api/core";
import type {
  SetState,
  GetState,
  Conversation,
  Message,
  Branch,
  Memo,
  Artifact,
  CreateConversationInput,
} from "./types";

export interface ConversationSlice {
  conversations: Conversation[];
  selectedConversationId: string | null;
  messages: Message[];
  createConversation: (input: CreateConversationInput) => Promise<Conversation>;
  deleteConversation: (id: string) => Promise<void>;
  selectConversation: (id: string) => Promise<void>;
  renameConversation: (id: string, customLabel: string) => Promise<void>;
  deleteMessagePair: (messageId: string) => Promise<void>;
}

export const createConversationSlice = (set: SetState, get: GetState): ConversationSlice => ({
  conversations: [],
  selectedConversationId: null,
  messages: [],

  createConversation: async (input: CreateConversationInput) => {
    const conv = await invoke<Conversation>("create_conversation", { input });
    const projectKey = get().selectedProjectKey;
    if (projectKey) {
      const conversations = await invoke<Conversation[]>("list_conversations", {
        projectKey,
      });
      set({ conversations });
    }
    return conv;
  },

  deleteConversation: async (id: string) => {
    try {
      await invoke("delete_conversation", { id });
      const { selectedProjectKey, selectedConversationId } = get();
      // Refresh conversation list
      if (selectedProjectKey) {
        const conversations = await invoke<Conversation[]>("list_conversations", {
          projectKey: selectedProjectKey,
        });
        set({ conversations });
      }
      // Clear selection if deleted conversation was selected
      if (selectedConversationId === id) {
        set({
          selectedConversationId: null,
          messages: [],
          branches: [],
          memos: [],
          artifacts: [],
          crossSessionIds: get().crossSessionIds.filter((cid) => cid !== id),
        });
      } else {
        // Remove from cross-session if it was included
        set({ crossSessionIds: get().crossSessionIds.filter((cid) => cid !== id) });
      }
    } catch (e) {
      set({ error: String(e) });
    }
  },

  selectConversation: async (id: string) => {
    set({ selectedConversationId: id, messages: [], branches: [], memos: [], artifacts: [] });
    import("@/lib/appStore").then(({ setSetting }) => setSetting("lastConversationId", id)).catch(() => {});
    try {
      const [messages, branches, memos, artifacts] = await Promise.all([
        invoke<Message[]>("list_messages", { conversationId: id }),
        invoke<Branch[]>("list_branches", { conversationId: id }),
        invoke<Memo[]>("list_memos_by_conversation", { conversationId: id }),
        invoke<Artifact[]>("list_artifacts", { conversationId: id }),
      ]);
      set({ messages, branches, memos, artifacts, error: null });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  renameConversation: async (id: string, customLabel: string) => {
    const trimmed = customLabel.trim() || undefined;
    set((state) => ({
      conversations: state.conversations.map((c) =>
        c.id === id ? { ...c, customLabel: trimmed } : c
      ),
    }));
    try {
      await invoke("rename_conversation", { id, customLabel });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  deleteMessagePair: async (messageId: string) => {
    const convId = get().selectedConversationId;
    if (!convId) return;
    try {
      await invoke("delete_message_pair", { messageId });
      const messages = await invoke<Message[]>("list_messages", { conversationId: convId });
      set({ messages });
    } catch (e) {
      set({ error: String(e) });
    }
  },
});
