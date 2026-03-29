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
  RoundtableParticipant,
  RtMode,
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
  sendThreadRoundtable: (prompt: string, participants: RoundtableParticipant[], mode?: RtMode) => Promise<void>;
  sendThreadRoundtableFollowup: (prompt: string, participants: RoundtableParticipant[], mode?: RtMode) => Promise<void>;
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

  createBranch: async (conversationId: string, checkpointId?: string, label?: string, mode?: string, parentBranchId?: string) => {
    try {
      const input: CreateBranchInput = { conversationId, checkpointId, label, mode, parentBranchId };
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

      // Determine parent conversation ID from branch or shadow conversation
      let parentConvId = branch?.conversationId ?? null;

      // If branch not in store (e.g. no conversation selected), resolve via shadow conv
      if (!parentConvId) {
        const branchConvId = await invoke<string>("open_branch_stream", { branchId });
        const branchConv = await invoke<Conversation>("get_conversation", { id: branchConvId });
        parentConvId = branchConv.parentId ?? null;
      }

      // If parent conversation is not currently selected, load it first
      if (parentConvId && parentConvId !== get().selectedConversationId) {
        const [messages, branches, memos, artifacts] = await Promise.all([
          invoke<Message[]>("list_messages", { conversationId: parentConvId }),
          invoke<Branch[]>("list_branches", { conversationId: parentConvId }),
          invoke<Memo[]>("list_memos_by_conversation", { conversationId: parentConvId }),
          invoke<Artifact[]>("list_artifacts", { conversationId: parentConvId }),
        ]);
        // Ensure parent conversation is in the conversations list
        let convs = get().conversations;
        if (!convs.some((c) => c.id === parentConvId)) {
          const parentConv = await invoke<Conversation>("get_conversation", { id: parentConvId! });
          convs = [...convs, parentConv];
        }
        set({ selectedConversationId: parentConvId, messages, branches, memos, artifacts, conversations: convs, error: null });
        // Re-find branch from fresh data
        branch = get().branches.find((b) => b.id === branchId);
      }

      const branchConvId = await invoke<string>("open_branch_stream", { branchId });
      const [branchMessages, branchConv] = await Promise.all([
        invoke<Message[]>("list_messages", { conversationId: branchConvId }),
        invoke<Conversation>("get_conversation", { id: branchConvId }),
      ]);
      // Find parent message using branch.checkpointId
      const parentMsg = branch?.checkpointId
        ? get().messages.find((m) => m.id === branch.checkpointId) ?? null
        : null;
      set((state) => ({
        threadBranchId: branchId,
        threadBranchConvId: branchConvId,
        threadMessages: branchMessages,
        threadBranchLabel: branch?.customLabel ?? branch?.label ?? branchId.slice(0, 12),
        threadParentMessage: parentMsg,
        // Add shadow conversation to conversations array (needed for RT detection)
        conversations: state.conversations.some((c) => c.id === branchConvId)
          ? state.conversations
          : [...state.conversations, branchConv],
      }));
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

  sendThreadRoundtable: async (prompt: string, participants: RoundtableParticipant[], mode?: RtMode) => {
    const { threadBranchConvId } = get();
    if (!threadBranchConvId) return;
    if (get().runningThreadIds.includes(threadBranchConvId)) {
      get()._enqueue(threadBranchConvId, prompt.slice(0, 30), () => get().sendThreadRoundtable(prompt, participants, mode));
      return;
    }
    get()._startRun(threadBranchConvId);
    const now = Date.now();
    set((state) => ({
      threadMessages: [
        ...state.threadMessages,
        { id: `temp-user-${now}`, conversationId: threadBranchConvId, role: "user", content: prompt, timestamp: now, status: "done" },
        { id: `temp-thinking-${now}`, conversationId: threadBranchConvId, role: "assistant", content: "", progressContent: "Roundtable starting...", timestamp: now, status: "streaming", engine: "system" },
      ],
    }));

    const { listen } = await import("@tauri-apps/api/event");
    let placeholderCleared = false;
    const ulRT = await listen<Message>("roundtable:progress", (event) => {
      const msg = event.payload;
      if (msg.role === "user") return;
      set((state) => {
        if (!placeholderCleared) {
          placeholderCleared = true;
          return { threadMessages: [...state.threadMessages.filter((m) => !m.id.startsWith("temp-thinking-")), msg] };
        }
        return { threadMessages: [...state.threadMessages, msg] };
      });
    });
    const cleanup = () => { ulRT(); ulD(); ulE(); };
    const ulD = await listen<{ conversationId: string }>("agent:completed", async (e) => {
      if (e.payload.conversationId !== threadBranchConvId) return;
      cleanup();
      const threadMessages = await invoke<Message[]>("list_messages", { conversationId: threadBranchConvId });
      set({ threadMessages }); get()._endRun(threadBranchConvId);
    });
    const ulE = await listen<{ conversationId: string; error: string }>("agent:error", async (e) => {
      if (e.payload.conversationId !== threadBranchConvId) return;
      cleanup(); set({ error: e.payload.error });
      const threadMessages = await invoke<Message[]>("list_messages", { conversationId: threadBranchConvId });
      set({ threadMessages }); get()._endRun(threadBranchConvId);
    });

    try {
      await invoke<{ messageId: string }>("start_roundtable_run", { input: { conversationId: threadBranchConvId, prompt, participants, mode } });
    } catch (e) {
      cleanup(); set({ error: String(e) }); get()._endRun(threadBranchConvId);
    }
  },

  sendThreadRoundtableFollowup: async (prompt: string, participants: RoundtableParticipant[], mode?: RtMode) => {
    const { threadBranchConvId } = get();
    if (!threadBranchConvId) return;
    if (get().runningThreadIds.includes(threadBranchConvId)) {
      get()._enqueue(threadBranchConvId, prompt.slice(0, 30), () => get().sendThreadRoundtableFollowup(prompt, participants, mode));
      return;
    }
    get()._startRun(threadBranchConvId);
    const now = Date.now();
    set((state) => ({
      threadMessages: [
        ...state.threadMessages,
        { id: `temp-user-${now}`, conversationId: threadBranchConvId, role: "user", content: prompt, timestamp: now, status: "done" },
        { id: `temp-thinking-${now}`, conversationId: threadBranchConvId, role: "assistant", content: "", progressContent: "Roundtable starting...", timestamp: now, status: "streaming", engine: "system" },
      ],
    }));

    const { listen } = await import("@tauri-apps/api/event");
    let placeholderCleared = false;
    const ulRT = await listen<Message>("roundtable:progress", (event) => {
      const msg = event.payload;
      if (msg.role === "user") return;
      set((state) => {
        if (!placeholderCleared) {
          placeholderCleared = true;
          return { threadMessages: [...state.threadMessages.filter((m) => !m.id.startsWith("temp-thinking-")), msg] };
        }
        return { threadMessages: [...state.threadMessages, msg] };
      });
    });
    const cleanup = () => { ulRT(); ulD(); ulE(); };
    const ulD = await listen<{ conversationId: string }>("agent:completed", async (e) => {
      if (e.payload.conversationId !== threadBranchConvId) return;
      cleanup();
      const threadMessages = await invoke<Message[]>("list_messages", { conversationId: threadBranchConvId });
      set({ threadMessages }); get()._endRun(threadBranchConvId);
    });
    const ulE = await listen<{ conversationId: string; error: string }>("agent:error", async (e) => {
      if (e.payload.conversationId !== threadBranchConvId) return;
      cleanup(); set({ error: e.payload.error });
      const threadMessages = await invoke<Message[]>("list_messages", { conversationId: threadBranchConvId });
      set({ threadMessages }); get()._endRun(threadBranchConvId);
    });

    try {
      await invoke<{ messageId: string }>("start_roundtable_followup", { input: { conversationId: threadBranchConvId, prompt, participants, mode } });
    } catch (e) {
      cleanup(); set({ error: String(e) }); get()._endRun(threadBranchConvId);
    }
  },
});
