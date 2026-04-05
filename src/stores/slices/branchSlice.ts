import { invoke } from "@tauri-apps/api/core";
import { errorMessage } from "@/lib/utils";
import type {
  SetState,
  GetState,
  Branch,
  Conversation,
  Message,
  Memo,
  Artifact,
  CreateBranchInput,
  AdoptBranchInput,
} from "./types";

export interface BranchSlice {
  branches: Branch[];
  activeBranchId: string | null;
  parentConversationId: string | null;
  loadBranches: (conversationId: string) => Promise<void>;
  createBranch: (conversationId: string, checkpointId?: string, label?: string, mode?: string) => Promise<void>;
  deleteBranch: (branchId: string) => Promise<void>;
  renameBranch: (branchId: string, customLabel: string) => Promise<void>;
  linkGitBranch: (branchId: string, gitBranch: string | null) => Promise<void>;
  adoptBranch: (branchId: string, conversationId: string) => Promise<void>;
  openBranchStream: (branchId: string) => Promise<void>;
  closeBranchStream: () => Promise<void>;
}

export const createBranchSlice = (set: SetState, get: GetState): BranchSlice => ({
  branches: [],
  activeBranchId: null,
  parentConversationId: null,

  loadBranches: async (conversationId: string) => {
    try {
      const branches = await invoke<Branch[]>("list_branches", { conversationId });
      set({ branches });
    } catch (e) {
      set({ error: errorMessage(e) });
    }
  },

  createBranch: async (conversationId: string, checkpointId?: string, label?: string, mode?: string, parentBranchId?: string) => {
    try {
      const input: CreateBranchInput = { conversationId, checkpointId, label, mode, parentBranchId };
      const created = await invoke<Branch>("create_branch", { input });
      // Use the root conversation ID from the created branch (backend resolves shadow convs)
      const rootConvId = created.conversationId;
      const branches = await invoke<Branch[]>("list_branches", { conversationId: rootConvId });
      set({ branches });
    } catch (e) {
      set({ error: errorMessage(e) });
    }
  },

  deleteBranch: async (branchId: string) => {
    const { selectedConversationId, parentConversationId, activeBranchId, threadBranchId } = get();
    try {
      await invoke("delete_branch", { id: branchId });

      // Reload branches for the correct parent conversation
      const convId = parentConversationId ?? selectedConversationId;
      if (convId && !convId.startsWith("branch:")) {
        const branches = await invoke<Branch[]>("list_branches", { conversationId: convId });
        set({ branches });
      }

      // If the deleted branch was open in full view, go back to parent
      if (activeBranchId === branchId && parentConversationId) {
        const { selectConversation } = get();
        await selectConversation(parentConversationId);
        return;
      }

      // Close thread drawer if the deleted branch was open
      if (threadBranchId === branchId) {
        set({
          threadBranchId: null,
          threadBranchConvId: null,
          threadMessages: [],
          threadBranchLabel: null,
          threadParentMessage: null,
        });
      }
    } catch (e) {
      set({ error: errorMessage(e) });
    }
  },

  renameBranch: async (branchId: string, customLabel: string) => {
    const trimmed = customLabel.trim() || undefined;
    set((state) => {
      const updates: Partial<typeof state> = {
        branches: state.branches.map((b) =>
          b.id === branchId ? { ...b, customLabel: trimmed } : b
        ),
      };
      // Also update threadBranchLabel if this branch is currently open in drawer
      if (state.threadBranchId === branchId) {
        const branch = state.branches.find((b) => b.id === branchId);
        updates.threadBranchLabel = trimmed ?? branch?.label ?? state.threadBranchLabel;
      }
      return updates;
    });
    try {
      await invoke("rename_branch", { id: branchId, customLabel });
    } catch (e) {
      set({ error: errorMessage(e) });
    }
  },

  linkGitBranch: async (branchId: string, gitBranch: string | null) => {
    set((state) => ({
      branches: state.branches.map((b) => b.id === branchId ? { ...b, gitBranch: gitBranch ?? undefined } : b),
    }));
    try {
      await invoke("link_git_branch", { id: branchId, gitBranch });
    } catch (e) {
      set({ error: errorMessage(e) });
    }
  },

  adoptBranch: async (branchId: string, conversationId: string) => {
    try {
      const input: AdoptBranchInput = { branchId, conversationId };
      await invoke("adopt_branch", { input });
      const [messages, branches] = await Promise.all([
        invoke<Message[]>("list_messages", { conversationId }),
        invoke<Branch[]>("list_branches", { conversationId }),
      ]);
      set({ messages, branches });
    } catch (e) {
      const msg = errorMessage(e);
      if (msg.includes("empty_branch")) {
        const { ask } = await import("@tauri-apps/plugin-dialog");
        if (await ask("빈 브랜치입니다. 삭제하시겠습니까?", { title: "빈 브랜치", kind: "warning" })) {
          // Close drawer/thread if this branch is open
          if (get().threadBranchId === branchId) {
            set({ threadBranchId: null, threadBranchConvId: null, threadMessages: [], threadBranchLabel: null, threadParentMessage: null });
          }
          await invoke("delete_branch", { id: branchId });
          const branches = await invoke<Branch[]>("list_branches", { conversationId });
          set({ branches });
        }
      } else {
        set({ error: msg });
      }
    }
  },

  openBranchStream: async (branchId: string) => {
    const { selectedConversationId } = get();
    if (!selectedConversationId) return;
    try {
      // Ensure shadow conversation row exists, get branch conv id
      const branchConvId = await invoke<string>("open_branch_stream", { branchId });
      const [branchMessages, branchConv] = await Promise.all([
        invoke<Message[]>("list_messages", { conversationId: branchConvId }),
        invoke<Conversation>("get_conversation", { id: branchConvId }),
      ]);
      // Add shadow conversation to conversations array so ChatPanel can find it
      set((state) => ({
        parentConversationId: selectedConversationId,
        activeBranchId: branchId,
        selectedConversationId: branchConvId,
        conversations: state.conversations.some((c) => c.id === branchConvId)
          ? state.conversations
          : [...state.conversations, branchConv],
        messages: branchMessages,
        error: null,
      }));
    } catch (e) {
      set({ error: errorMessage(e) });
    }
  },

  closeBranchStream: async () => {
    const { parentConversationId } = get();
    if (!parentConversationId) return;
    set({ activeBranchId: null, parentConversationId: null });
    await get().selectConversation(parentConversationId);
  },
});
