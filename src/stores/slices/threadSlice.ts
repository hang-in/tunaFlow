import { invoke } from "@tauri-apps/api/core";
import { errorMessage } from "@/lib/utils";
import { ENGINE_CONFIGS } from "@/lib/engineConfig";
import { useToolStepsStore } from "@/stores/toolStepsStore";
import { serializeSteps } from "@/lib/toolSteps";
import type {
  SetState,
  GetState,
  Branch,
  Conversation,
  Message,
  Memo,
  Artifact,
  SendWithClaudeInput,
  RoundtableParticipant,
  RtMode,
} from "./types";

export interface RtParticipantStatus {
  name: string;
  engine: string;
  model: string | null;
  round: number;
  status: "running" | "done" | "error";
  updatedAt: number;
}

export interface ThreadSlice {
  threadBranchId: string | null;
  threadBranchConvId: string | null;
  threadMessages: Message[];
  threadBranchLabel: string | null;
  threadParentMessage: Message | null;
  rtParticipantStatuses: Map<string, RtParticipantStatus>;
  rtStatusConversationId: string | null;
  openThread: (branchId: string) => Promise<void>;
  closeThread: () => void;
  sendThreadMessage: (prompt: string, engine?: string, model?: string) => Promise<void>;
  sendThreadRoundtable: (prompt: string, participants: RoundtableParticipant[], mode?: RtMode) => Promise<void>;
  sendThreadRoundtableFollowup: (prompt: string, participants: RoundtableParticipant[], mode?: RtMode) => Promise<void>;
}

export const createThreadSlice = (set: SetState, get: GetState): ThreadSlice => ({
  threadBranchId: null,
  threadBranchConvId: null,
  threadMessages: [],
  threadBranchLabel: null,
  rtParticipantStatuses: new Map(),
  rtStatusConversationId: null,
  threadParentMessage: null,

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
      // For depth>1 branches, the checkpoint message lives in the parent branch's shadow conversation
      let parentMsg: Message | null = null;
      if (branch?.checkpointId) {
        // First try main conversation messages
        parentMsg = get().messages.find((m) => m.id === branch.checkpointId) ?? null;
        // If not found and branch has a parent branch, load from parent's shadow conversation
        if (!parentMsg && branch.parentBranchId) {
          const parentShadowId = `branch:${branch.parentBranchId}`;
          try {
            const parentBranchMsgs = await invoke<Message[]>("list_messages", { conversationId: parentShadowId });
            parentMsg = parentBranchMsgs.find((m) => m.id === branch.checkpointId) ?? null;
          } catch (e) { console.warn("[thread] parent branch message load failed:", e); }
        }
      }
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

      // Auto-switch center tab when opening a plan-linked branch
      import("@/lib/api/plans").then(async ({ findPlanByBranch }) => {
        const plan = await findPlanByBranch(branchId);
        if (plan && (plan.implementationBranchId === branchId || plan.reviewBranchId === branchId)) {
          window.dispatchEvent(new CustomEvent("tunaflow:switch-tab", { detail: "plan" }));
          // Also switch to the correct workflow stage based on plan phase
          const PHASE_TO_STAGE: Record<string, string> = {
            drafting: "plan", subtask_review: "subtask", approval: "approved",
            implementation: "dev", rework: "dev", review: "review", done: "decision",
          };
          const stage = PHASE_TO_STAGE[plan.phase];
          if (stage) {
            window.dispatchEvent(new CustomEvent("tunaflow:switch-stage", { detail: stage }));
          }
        }
      }).catch((e) => console.debug("[plan-tab]", e));
    } catch (e) {
      const msg = errorMessage(e);
      // If branch was already deleted, silently reload branches
      if (msg.includes("not found") || msg.includes("Not found")) {
        const convId = get().selectedConversationId;
        if (convId) {
          invoke<Branch[]>("list_branches", { conversationId: convId })
            .then((branches) => set({ branches }))
            .catch((e) => console.debug("[branch-reload]", e));
        }
      } else {
        set({ error: msg });
      }
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
    const convId = threadBranchConvId; // narrowed: string (guaranteed by guard above)

    // Add to runningThreadIds for thread-aware tracking
    get()._startRun(convId);

    const now = Date.now();
    set((state) => ({
      threadMessages: [
        ...state.threadMessages,
        { id: `temp-user-${now}`, conversationId: convId, role: "user", content: prompt, timestamp: now, status: "done" },
        { id: `temp-thinking-${now}`, conversationId: convId, role: "assistant", content: "", progressContent: (ENGINE_CONFIGS[engine ?? "claude"] ?? ENGINE_CONFIGS.claude).label, timestamp: now, status: "streaming", engine: (ENGINE_CONFIGS[engine ?? "claude"] ?? ENGINE_CONFIGS.claude).engineKey, model },
      ],
    }));

    const { getSetting } = await import("@/lib/appStore");
    const budgetCfg = await getSetting<{ mode: string; totalCap: number }>("contextBudgetConfig", { mode: "auto", totalCap: 60000 });
    // Resolve phase-based workflow skills
    const planPhase = await invoke<string | null>("get_active_plan_phase", { conversationId: convId }).catch(() => null);
    const effectiveSkills = get().getEffectiveSkills(planPhase, prompt);
    const input: SendWithClaudeInput = {
      projectKey: selectedProjectKey,
      conversationId: convId,
      prompt,
      model,
      activeSkills: effectiveSkills,
      crossSessionIds,
      personaFragment: get().personaFragment ?? undefined,
      personaLabel: get().personaLabel ?? undefined,
      contextModeOverride: budgetCfg.mode === "auto" ? undefined : budgetCfg.mode,
      contextBudgetCap: budgetCfg.totalCap === 60000 ? undefined : budgetCfg.totalCap,
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
        return { threadMessages: [...withoutPlaceholder, { id: messageId, conversationId: convId, role: "assistant" as const, content: field === "content" ? text : "", progressContent: field === "progressContent" ? text : undefined, timestamp: Date.now(), status: "streaming" as const, engine: engineKey, model }] };
      });
    };

    const ulP = await listen<{ messageId: string; conversationId: string; text: string }>(progressEvent, (e) => {
      if (e.payload.conversationId !== convId) return;
      useToolStepsStore.getState().handleProgress(e.payload.messageId, e.payload.text);
      replaceOrAdd(e.payload.messageId, "progressContent", e.payload.text);
    });
    const ulC = chunkEvent ? await listen<{ messageId: string; conversationId: string; text: string }>(chunkEvent, (e) => {
      if (e.payload.conversationId !== convId) return;
      replaceOrAdd(e.payload.messageId, "content", e.payload.text);
    }) : () => {};
    const cleanup = () => { ulP(); ulC(); ulD(); ulE(); };

    const ulD = await listen<{ messageId: string; conversationId: string }>("agent:completed", async (e) => {
      if (e.payload.conversationId !== convId) return;
      cleanup();
      // Save tool steps
      const tsStore = useToolStepsStore.getState();
      const steps = tsStore.getSteps(e.payload.messageId);
      if (steps.length > 0) {
        invoke("save_progress_content", { messageId: e.payload.messageId, content: serializeSteps(steps) }).catch((e) => console.debug("[save-steps]", e));
        tsStore.clear(e.payload.messageId);
      }
      const threadMessages = await invoke<Message[]>("list_messages", { conversationId: convId });
      set({ threadMessages });
      // Check for tool-request markers → auto follow-up in thread
      const lastMsg = threadMessages.find((m) => m.id === e.payload.messageId);
      if (lastMsg?.role === "assistant") {
        import("@/lib/planProposalParser").then(async ({ extractToolRequests }) => {
          const requests = extractToolRequests(lastMsg.content);
          if (requests.length > 0) {
            const { executeToolRequests } = await import("@/lib/toolRequestHandler");
            const followUp = await executeToolRequests(requests);
            if (followUp) {
              const saved = get().getConversationEngine(convId);
              get().sendThreadMessage(followUp, saved?.engine ?? "claude", saved?.model ?? undefined);
            }
          }
        }).catch((e) => console.warn("[tool-request]", e));
      }
      // Auto-detect review verdict after tool-request handling
      autoDetectReviewVerdict(convId, threadMessages);
      get()._endRun(convId);
    });
    const ulE = await listen<{ conversationId: string; error: string }>("agent:error", async (e) => {
      if (e.payload.conversationId !== convId) return;
      cleanup(); set({ error: e.payload.error });
      const threadMessages = await invoke<Message[]>("list_messages", { conversationId: convId });
      set({ threadMessages }); get()._endRun(convId);
    });

    try {
      const config = ENGINE_CONFIGS[engineKey] ?? ENGINE_CONFIGS.claude;
      await invoke<{ messageId: string }>(config.command, { input });
    } catch (e) {
      cleanup();
      set((state) => ({ error: errorMessage(e), threadMessages: state.threadMessages.filter((m) => !m.id.startsWith("temp-thinking-")) }));
      get()._endRun(convId);
    }
  },

  sendThreadRoundtable: async (prompt: string, participants: RoundtableParticipant[], mode?: RtMode) => {
    await runThreadRoundtable(set, get, "start_roundtable_run", prompt, participants, mode);
  },

  sendThreadRoundtableFollowup: async (prompt: string, participants: RoundtableParticipant[], mode?: RtMode) => {
    await runThreadRoundtable(set, get, "start_roundtable_followup", prompt, participants, mode);
  },
});

// ─── Thread RT helper (shared by run + followup) ────────────────────────────

async function runThreadRoundtable(
  set: SetState, get: GetState, command: string,
  prompt: string, participants: RoundtableParticipant[], mode?: RtMode,
) {
  const { threadBranchConvId } = get();
  if (!threadBranchConvId) return;
  if (get().runningThreadIds.includes(threadBranchConvId)) {
    get()._enqueue(threadBranchConvId, prompt.slice(0, 30), () =>
      command === "start_roundtable_run"
        ? get().sendThreadRoundtable(prompt, participants, mode)
        : get().sendThreadRoundtableFollowup(prompt, participants, mode),
    );
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
  set({ rtParticipantStatuses: new Map(), rtStatusConversationId: threadBranchConvId });
  let placeholderCleared = false;

  const ulPS = await listen<{ conversationId: string; name: string; engine: string; model?: string; round: number; status: string }>(
    "roundtable:participant_status", (e) => {
      if (e.payload.conversationId !== threadBranchConvId) return;
      const { name, engine, model, round, status } = e.payload;
      set((state) => {
        const next = new Map(state.rtParticipantStatuses);
        next.set(name, { name, engine, model: model ?? null, round, status: status as RtParticipantStatus["status"], updatedAt: Date.now() });
        return { rtParticipantStatuses: next };
      });
    },
  );
  const ulRT = await listen<Message>("roundtable:progress", (event) => {
    const msg = event.payload;
    if (msg.conversationId !== threadBranchConvId) return;
    if (msg.role === "user") return;
    set((state) => {
      if (state.threadMessages.some((m) => m.id === msg.id)) return state;
      if (!placeholderCleared) {
        placeholderCleared = true;
        return { threadMessages: [...state.threadMessages.filter((m) => !m.id.startsWith("temp-thinking-")), msg] };
      }
      return { threadMessages: [...state.threadMessages, msg] };
    });
  });
  const cleanup = () => { ulPS(); ulRT(); ulD(); ulE(); };
  const ulD = await listen<{ conversationId: string }>("agent:completed", async (e) => {
    if (e.payload.conversationId !== threadBranchConvId) return;
    cleanup();
    if (get().threadBranchConvId === threadBranchConvId) {
      const threadMessages = await invoke<Message[]>("list_messages", { conversationId: threadBranchConvId });
      set({ threadMessages });
    }
    // Auto-detect review verdict after RT completion
    if (get().threadBranchConvId === threadBranchConvId) {
      const latestMsgs = await invoke<Message[]>("list_messages", { conversationId: threadBranchConvId });
      autoDetectReviewVerdict(threadBranchConvId, latestMsgs);
    }
    setTimeout(() => set({ rtParticipantStatuses: new Map(), rtStatusConversationId: null }), 2000);
    get()._endRun(threadBranchConvId);
  });
  const ulE = await listen<{ conversationId: string; error: string }>("agent:error", async (e) => {
    if (e.payload.conversationId !== threadBranchConvId) return;
    cleanup();
    if (get().threadBranchConvId === threadBranchConvId) {
      set({ error: e.payload.error });
      const threadMessages = await invoke<Message[]>("list_messages", { conversationId: threadBranchConvId });
      set({ threadMessages, rtParticipantStatuses: new Map(), rtStatusConversationId: null });
    }
    get()._endRun(threadBranchConvId);
  });

  try {
    await invoke<{ messageId: string }>(command, { input: { conversationId: threadBranchConvId, prompt, participants, mode } });
  } catch (e) {
    cleanup(); set({ error: errorMessage(e), rtParticipantStatuses: new Map(), rtStatusConversationId: null }); get()._endRun(threadBranchConvId);
  }
}

// ─── Auto-detect review verdict from completed branch ──────────────────────
// Called after agent:completed in both single-agent and RT review flows.
// Extracts branchId from shadow conversation ID, finds linked plan,
// and auto-processes verdict if found.

async function autoDetectReviewVerdict(shadowConvId: string, messages: Message[]) {
  if (!shadowConvId.startsWith("branch:")) return;
  const branchId = shadowConvId.slice("branch:".length);

  try {
    const { findPlanByBranch } = await import("@/lib/api/plans");
    const plan = await findPlanByBranch(branchId);
    if (!plan || plan.reviewBranchId !== branchId) return;
    if (plan.phase !== "review") return;

    const { scanMessagesForMarkers, processReviewVerdict } = await import("@/lib/workflowOrchestration");
    const markers = scanMessagesForMarkers(messages);
    if (!markers.reviewVerdict) return;

    const { toast } = await import("sonner");
    const verdict = markers.reviewVerdict.verdict;
    await processReviewVerdict(plan, markers.reviewVerdict);

    if (verdict === "pass") {
      toast.success("Review 통과 — Plan 완료 처리됨");
    } else if (verdict === "fail") {
      toast.warning("Review 실패 — Rework 단계로 전환됨");
    } else {
      toast.info("Review 조건부 통과 — 사용자 판단 필요");
    }
  } catch (e) {
    console.warn("[verdict-autodetect]", e);
  }
}
