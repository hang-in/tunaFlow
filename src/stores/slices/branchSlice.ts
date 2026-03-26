import { invoke } from "@tauri-apps/api/core";
import type {
  SetState,
  GetState,
  Branch,
  Message,
  Memo,
  Artifact,
  CreateBranchInput,
  AdoptBranchInput,
  SendWithClaudeInput,
} from "./types";

export interface BranchSlice {
  branches: Branch[];
  activeBranchId: string | null;
  parentConversationId: string | null;
  threadBranchId: string | null;
  threadBranchConvId: string | null;
  threadMessages: Message[];
  threadBranchLabel: string | null;
  threadParentMessage: Message | null;
  loadBranches: (conversationId: string) => Promise<void>;
  createBranch: (conversationId: string, checkpointId?: string, label?: string, mode?: string) => Promise<void>;
  deleteBranch: (branchId: string) => Promise<void>;
  renameBranch: (branchId: string, customLabel: string) => Promise<void>;
  adoptBranch: (branchId: string, conversationId: string) => Promise<void>;
  openBranchStream: (branchId: string) => Promise<void>;
  closeBranchStream: () => Promise<void>;
  openThread: (branchId: string) => Promise<void>;
  closeThread: () => void;
  sendThreadMessage: (prompt: string, engine?: string, model?: string) => Promise<void>;
}

export const createBranchSlice = (set: SetState, get: GetState): BranchSlice => ({
  branches: [],
  activeBranchId: null,
  parentConversationId: null,
  threadBranchId: null,
  threadBranchConvId: null,
  threadMessages: [],
  threadBranchLabel: null,
  threadParentMessage: null,

  loadBranches: async (conversationId: string) => {
    try {
      const branches = await invoke<Branch[]>("list_branches", { conversationId });
      set({ branches });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  createBranch: async (conversationId: string, checkpointId?: string, label?: string, mode?: string) => {
    try {
      const input: CreateBranchInput = { conversationId, checkpointId, label, mode };
      await invoke<Branch>("create_branch", { input });
      const branches = await invoke<Branch[]>("list_branches", { conversationId });
      set({ branches });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  deleteBranch: async (branchId: string) => {
    const { selectedConversationId, threadBranchId } = get();
    try {
      await invoke("delete_branch", { id: branchId });
      if (selectedConversationId) {
        const branches = await invoke<Branch[]>("list_branches", {
          conversationId: selectedConversationId,
        });
        set({ branches });
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
      set({ error: String(e) });
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
      set({ error: String(e) });
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
      const msg = String(e);
      if (msg.includes("empty_branch")) {
        if (window.confirm("빈 브랜치입니다. 삭제하시겠습니까?")) {
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
      // Ensure shadow conversations row exists, get branch conv id
      const branchConvId = await invoke<string>("open_branch_stream", { branchId });
      const branchMessages = await invoke<Message[]>("list_messages", {
        conversationId: branchConvId,
      });
      set({
        parentConversationId: selectedConversationId,
        activeBranchId: branchId,
        selectedConversationId: branchConvId,
        messages: branchMessages,
        branches: [],
        error: null,
      });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  closeBranchStream: async () => {
    const { parentConversationId } = get();
    if (!parentConversationId) return;
    set({ activeBranchId: null, parentConversationId: null });
    await get().selectConversation(parentConversationId);
  },

  openThread: async (branchId: string) => {
    try {
      // Find the branch — may come from store or need DB lookup
      let branch = get().branches.find((b) => b.id === branchId);

      // If branch's parent conversation is not currently selected, switch to it first
      if (branch?.conversationId && branch.conversationId !== get().selectedConversationId) {
        // Select the parent conversation to load its messages/branches
        const parentConvId = branch.conversationId;
        const [messages, branches, memos, artifacts] = await Promise.all([
          invoke<Message[]>("list_messages", { conversationId: parentConvId }),
          invoke<Branch[]>("list_branches", { conversationId: parentConvId }),
          invoke<Memo[]>("list_memos_by_conversation", { conversationId: parentConvId }),
          invoke<Artifact[]>("list_artifacts", { conversationId: parentConvId }),
        ]);
        set({ selectedConversationId: parentConvId, messages, branches, memos, artifacts, error: null });
        // Re-find branch from fresh data
        branch = branches.find((b) => b.id === branchId);
      }

      const branchConvId = await invoke<string>("open_branch_stream", { branchId });
      const branchMessages = await invoke<Message[]>("list_messages", {
        conversationId: branchConvId,
      });
      // Find parent message using branch.checkpointId
      const parentMsg = branch?.checkpointId
        ? get().messages.find((m) => m.id === branch.checkpointId) ?? null
        : null;
      set({
        threadBranchId: branchId,
        threadBranchConvId: branchConvId,
        threadMessages: branchMessages,
        threadBranchLabel: branch?.customLabel ?? branch?.label ?? branchId.slice(0, 12),
        threadParentMessage: parentMsg,
      });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  closeThread: () => {
    set({
      threadBranchId: null,
      threadBranchConvId: null,
      threadMessages: [],
      threadBranchLabel: null,
      threadParentMessage: null,
    });
  },

  sendThreadMessage: async (prompt: string, engine?: string, model?: string) => {
    const { threadBranchConvId, selectedProjectKey } = get();
    if (!threadBranchConvId || !selectedProjectKey) return;

    const tempMsg: Message = {
      id: `temp-thread-${Date.now()}`,
      conversationId: threadBranchConvId,
      role: "user",
      content: prompt,
      timestamp: Date.now(),
      status: "done",
    };
    set((state) => ({
      isRunning: true,
      threadMessages: [...state.threadMessages, tempMsg],
    }));

    try {
      const input: SendWithClaudeInput = {
        projectKey: selectedProjectKey,
        conversationId: threadBranchConvId,
        prompt,
        model,
      };
      const engineKey = engine ?? "claude";
      if (engineKey === "codex") {
        await invoke<Message>("send_with_codex", { input });
      } else if (engineKey === "gemini") {
        await invoke<Message>("send_with_gemini", { input });
      } else if (engineKey === "opencode") {
        await invoke<Message>("send_with_opencode", { input });
      } else {
        await invoke<Message>("stream_with_claude", { input });
      }
      const threadMessages = await invoke<Message[]>("list_messages", {
        conversationId: threadBranchConvId,
      });
      set({ threadMessages, isRunning: false });
    } catch (e) {
      set({ error: String(e), isRunning: false });
    }
  },
});
