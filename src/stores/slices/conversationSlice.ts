import { invoke } from "@tauri-apps/api/core";
import { errorMessage } from "@/lib/utils";
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
      set({ error: errorMessage(e) });
    }
  },

  selectConversation: async (id: string) => {
    // Save current conversation's engine state before switching
    const prevConvId = get().selectedConversationId;
    if (prevConvId) {
      // Engine state will be saved by NewMessageInput via saveConversationEngine
      // (already handled on profile/engine change — no action needed here)
    }

    // Close drawer/thread if open — conversation is the primary view
    set({
      selectedConversationId: id,
      messages: [], branches: [], memos: [], artifacts: [],
      threadBranchId: null, threadBranchConvId: null, threadMessages: [],
      threadBranchLabel: null, threadParentMessage: null,
    });
    import("@/lib/appStore").then(({ setSetting }) => setSetting("lastConversationId", id)).catch((e) => console.debug("[settings]", e));

    // NOTE: per-conversation engine/model restore is handled by
    // NewMessageInput's restore useEffect (effectiveConvForRestore dependency).
    // Do NOT call selectProfile here — it triggers profile useEffect which
    // races with restore useEffect and overrides the saved model.

    try {
      const [messages, branches, memos, artifacts] = await Promise.all([
        invoke<Message[]>("list_messages", { conversationId: id }),
        invoke<Branch[]>("list_branches", { conversationId: id }),
        invoke<Memo[]>("list_memos_by_conversation", { conversationId: id }),
        invoke<Artifact[]>("list_artifacts", { conversationId: id }),
      ]);
      // Clear stale mark if it was set (agent completed while user was away)
      const stale = get()._staleConversations;
      if (stale?.has(id)) {
        const next = new Set(stale);
        next.delete(id);
        set({ messages, branches, memos, artifacts, error: null, _staleConversations: next });
      } else {
        set({ messages, branches, memos, artifacts, error: null });
      }
    } catch (e) {
      set({ error: errorMessage(e) });
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
      set({ error: errorMessage(e) });
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
      set({ error: errorMessage(e) });
    }
  },
});
