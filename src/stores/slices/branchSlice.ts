import { invoke } from "@tauri-apps/api/core";
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
    const { threadBranchConvId, threadBranchId, selectedProjectKey, activeSkills, crossSessionIds } = get();
    if (!threadBranchConvId || !selectedProjectKey || !threadBranchId) return;

    // Add to runningThreadIds for thread-aware tracking
    get()._startRun(threadBranchConvId);

    const now = Date.now();
    set((state) => ({
      threadMessages: [
        ...state.threadMessages,
        { id: `temp-user-${now}`, conversationId: threadBranchConvId, role: "user", content: prompt, timestamp: now, status: "done" },
        { id: `temp-thinking-${now}`, conversationId: threadBranchConvId, role: "assistant", content: "", progressContent: `${engine ?? "claude"} starting...`, timestamp: now, status: "streaming", engine: engine ?? "claude", model },
      ],
    }));

    const input: SendWithClaudeInput = {
      projectKey: selectedProjectKey,
      conversationId: threadBranchConvId,
      prompt,
      model,
      activeSkills,
      crossSessionIds,
    };

    // Event listeners for streaming updates
    const { listen } = await import("@tauri-apps/api/event");
    const engineKey = engine ?? "claude";
    const progressEvent = `${engineKey}:progress`;
    const chunkEvent = `${engineKey}:chunk`;

    const replaceOrAdd = (messageId: string, field: "content" | "progressContent", text: string) => {
      set((state) => {
        const existing = state.threadMessages.find((m) => m.id === messageId);
        if (existing) {
          return { threadMessages: state.threadMessages.map((m) => m.id === messageId ? { ...m, [field]: text } : m) };
        }
        const withoutPlaceholder = state.threadMessages.filter((m) => !m.id.startsWith("temp-thinking-"));
        return { threadMessages: [...withoutPlaceholder, { id: messageId, conversationId: threadBranchConvId!, role: "assistant" as const, content: field === "content" ? text : "", progressContent: field === "progressContent" ? text : undefined, timestamp: Date.now(), status: "streaming" as const, engine: engineKey, model }] };
      });
    };

    const ulP = await listen<{ messageId: string; text: string }>(progressEvent, (e) => replaceOrAdd(e.payload.messageId, "progressContent", e.payload.text));
    const ulC = chunkEvent ? await listen<{ messageId: string; text: string }>(chunkEvent, (e) => replaceOrAdd(e.payload.messageId, "content", e.payload.text)) : () => {};
    const cleanup = () => { ulP(); ulC(); ulD(); ulE(); };

    const ulD = await listen<{ conversationId: string }>("agent:completed", async (e) => {
      if (e.payload.conversationId !== threadBranchConvId) return;
      cleanup();
      const threadMessages = await invoke<Message[]>("list_messages", { conversationId: threadBranchConvId! });
      set({ threadMessages }); get()._endRun(threadBranchConvId!);
    });
    const ulE = await listen<{ conversationId: string; error: string }>("agent:error", async (e) => {
      if (e.payload.conversationId !== threadBranchConvId) return;
      cleanup(); set({ error: e.payload.error });
      const threadMessages = await invoke<Message[]>("list_messages", { conversationId: threadBranchConvId! });
      set({ threadMessages }); get()._endRun(threadBranchConvId!);
    });

    try {
      const cmd = engineKey === "codex" ? "start_codex_run"
        : engineKey === "gemini" ? "start_gemini_stream"
        : engineKey === "opencode" ? "start_opencode_run"
        : "start_claude_stream";
      await invoke<{ messageId: string }>(cmd, { input });
    } catch (e) {
      cleanup();
      set((state) => ({ error: String(e), threadMessages: state.threadMessages.filter((m) => !m.id.startsWith("temp-thinking-")) }));
      get()._endRun(threadBranchConvId);
    }
  },
});
