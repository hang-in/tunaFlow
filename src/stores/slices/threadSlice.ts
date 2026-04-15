import { invoke } from "@tauri-apps/api/core";
import { errorMessage } from "@/lib/utils";
import { ENGINE_CONFIGS } from "@/lib/engineConfig";
import { usePtyStore, isPtyEngine } from "@/stores/ptyStore";
import { sendMessageViaPty } from "./ptyMessageSender";
import { useToolStepsStore } from "@/stores/toolStepsStore";
import { handleToolRequests, saveToolSteps } from "./agentStreamHelper";
import { autoSyncImplCompletion, autoDetectReviewVerdict } from "@/lib/workflow/branchSync";
import { runThreadRoundtable } from "./threadRtRunner";
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
  drawerPinned: boolean;
  rtParticipantStatuses: Map<string, RtParticipantStatus>;
  rtStatusConversationId: string | null;
  openThread: (branchId: string) => Promise<void>;
  closeThread: () => void;
  toggleDrawerPin: () => void;
  sendThreadMessage: (prompt: string, engine?: string, model?: string) => Promise<void>;
  sendThreadRoundtable: (prompt: string, participants: RoundtableParticipant[], mode?: RtMode) => Promise<void>;
  sendThreadRoundtableFollowup: (prompt: string, participants: RoundtableParticipant[], mode?: RtMode) => Promise<void>;
}

export const createThreadSlice = (set: SetState, get: GetState): ThreadSlice => ({
  threadBranchId: null,
  threadBranchConvId: null,
  threadMessages: [],
  threadBranchLabel: null,
  drawerPinned: false,
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
          window.dispatchEvent(new CustomEvent("tunaflow:switch-tab", { detail: "workflow" }));
          // Also switch to the correct workflow stage based on plan phase
          const PHASE_TO_STAGE: Record<string, string> = {
            drafting: "plan", subtask_review: "subtask", approval: "dev",
            implementation: "dev", rework: "dev", review: "review", done: "done",
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
      drawerPinned: false,
    });
  },

  toggleDrawerPin: () => {
    set((state) => ({ drawerPinned: !state.drawerPinned }));
  },

  sendThreadMessage: async (prompt: string, engine?: string, model?: string) => {
    const { threadBranchConvId, threadBranchId, selectedProjectKey } = get();
    if (!threadBranchConvId || !selectedProjectKey || !threadBranchId) return;
    const convId = threadBranchConvId; // narrowed: string (guaranteed by guard above)
    const engineKey = engine ?? "claude";

    // Queue if already running
    if (get().runningThreadIds.includes(convId)) {
      get()._enqueue(convId, prompt.slice(0, 30), () =>
        get().sendThreadMessage(prompt, engine, model),
      );
      return;
    }

    // PTY path: opt-in only. See runtimeSlice.ts for rationale.
    const { getSetting: getAppSetting } = await import("@/lib/appStore");
    const ptyEnabled = await getAppSetting<boolean>("ptyEnabled", false);
    if (ptyEnabled && isPtyEngine(engineKey)) {
      const ptySession = usePtyStore.getState().getSession(engineKey);
      if (ptySession !== null) {
        try {
          await sendMessageViaPty(set, get, prompt, ptySession, convId, engineKey, {
            messageTarget: "threadMessages",
            isActiveCheck: () => get().threadBranchConvId === convId,
            personaLabel: get().personaLabel ?? undefined,
            onCompleted: async (savedMsg, text) => {
              // Reload thread messages from DB
              const threadMessages = await invoke<Message[]>("list_messages", { conversationId: convId });
              set({ threadMessages });
              // Tool-request markers → auto follow-up
              let toolRequestHandled = false;
              if (savedMsg.role === "assistant") {
                const followUp = await handleToolRequests(savedMsg.role === "assistant" ? { ...savedMsg, content: text } : savedMsg);
                if (followUp) {
                  const saved = get().getConversationEngine(convId);
                  get()._endRun(convId);
                  get().sendThreadMessage(followUp, saved?.engine ?? "claude", saved?.model ?? undefined);
                  toolRequestHandled = true;
                }
              }
              // Auto-sync implementation subtasks + detect completion
              autoSyncImplCompletion(convId, threadMessages);
              autoDetectReviewVerdict(convId, threadMessages);
              // Notify
              import("@/stores/notificationStore").then(({ notify }) => {
                const state = get();
                const branch = state.branches.find((b) => `branch:${b.id}` === convId);
                const engine = state.getConversationEngine(convId)?.engine;
                const lastAsst = threadMessages.filter((m) => m.role === "assistant").slice(-1)[0];
                notify("completed", state.personaLabel ?? "에이전트", "응답 완료", convId, {
                  engine,
                  conversationTitle: branch?.customLabel ?? branch?.label,
                  preview: lastAsst?.content?.replace(/\n+/g, " ").slice(0, 80),
                });
              }).catch((e) => console.debug("[notify]", e));
              return toolRequestHandled; // true = caller handles _endRun
            },
          });
          return;
        } catch (ptyErr) {
          console.error("[pty] thread PTY failed, falling back to -p mode:", ptyErr);
          usePtyStore.getState().clearSession(engineKey as import("@/stores/ptyStore").PtyEngine);
          import("sonner").then(({ toast }) => toast.warning("PTY 오류 — CLI 모드로 전환")).catch(() => {});
          // Fall through to -p mode below
        }
      }
    }

    // -p mode fallback
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
    const userProfile = await getSetting("userProfile", null as unknown as object).catch(() => null);
    // Resolve phase-based workflow skills
    const planPhase = await invoke<string | null>("get_active_plan_phase", { conversationId: convId }).catch(() => null);
    const effectiveSkills = get().getEffectiveSkills(planPhase, prompt);
    const input: SendWithClaudeInput = {
      projectKey: selectedProjectKey,
      conversationId: convId,
      prompt,
      model,
      activeSkills: effectiveSkills,
      crossSessionIds: get().crossSessionIds,
      personaFragment: get().personaFragment ?? undefined,
      personaLabel: get().personaLabel ?? undefined,
      contextModeOverride: budgetCfg.mode === "auto" ? undefined : budgetCfg.mode,
      contextBudgetCap: budgetCfg.totalCap === 60000 ? undefined : budgetCfg.totalCap,
      userProfileJson: userProfile ? JSON.stringify(userProfile) : undefined,
    };

    // Event listeners for streaming updates
    const { listen } = await import("@tauri-apps/api/event");
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

    // Guard: only update UI if this branch is still the active thread (prevents cross-project contamination)
    const isActiveThread = () => get().threadBranchConvId === convId;

    const ulP = await listen<{ messageId: string; conversationId: string; text: string }>(progressEvent, (e) => {
      if (e.payload.conversationId !== convId) return;
      useToolStepsStore.getState().handleProgress(e.payload.messageId, e.payload.text);
      if (!isActiveThread()) return;
      replaceOrAdd(e.payload.messageId, "progressContent", e.payload.text);
    });
    const ulC = chunkEvent ? await listen<{ messageId: string; conversationId: string; text: string }>(chunkEvent, (e) => {
      if (e.payload.conversationId !== convId) return;
      if (!isActiveThread()) return;
      replaceOrAdd(e.payload.messageId, "content", e.payload.text);
    }) : () => {};
    const cleanup = () => { ulP(); ulC(); ulD(); ulE(); };

    const ulD = await listen<{ messageId: string; conversationId: string }>("agent:completed", async (e) => {
      if (e.payload.conversationId !== convId) return;
      cleanup();
      await saveToolSteps(e.payload.messageId);
      const threadMessages = await invoke<Message[]>("list_messages", { conversationId: convId });
      set({ threadMessages });
      // Check for tool-request markers → auto follow-up in thread.
      // _endRun is deferred until after tool-request handling to prevent idle↔running flicker.
      const lastMsg = threadMessages.find((m) => m.id === e.payload.messageId);
      let toolRequestHandled = false;
      const followUp = await handleToolRequests(lastMsg);
      if (followUp) {
        const saved = get().getConversationEngine(convId);
        get()._endRun(convId);
        get().sendThreadMessage(followUp, saved?.engine ?? "claude", saved?.model ?? undefined);
        toolRequestHandled = true;
      }
      // Auto-sync implementation subtasks + detect completion
      autoSyncImplCompletion(convId, threadMessages);
      // Auto-detect review verdict after tool-request handling
      autoDetectReviewVerdict(convId, threadMessages);
      // Notify thread completion
      import("@/stores/notificationStore").then(({ notify }) => {
        const state = get();
        const branch = state.branches.find((b) => `branch:${b.id}` === convId);
        const engine = state.getConversationEngine(convId)?.engine;
        const lastAsst = threadMessages.filter((m) => m.role === "assistant").slice(-1)[0];
        notify("completed", state.personaLabel ?? "에이전트", "응답 완료", convId, {
          engine,
          conversationTitle: branch?.customLabel ?? branch?.label,
          preview: lastAsst?.content?.replace(/\n+/g, " ").slice(0, 80),
        });
      }).catch((e) => console.debug("[notify:completed]", e));
      if (!toolRequestHandled) get()._endRun(convId);
    });
    const ulE = await listen<{ conversationId: string; error: string }>("agent:error", async (e) => {
      if (e.payload.conversationId !== convId) return;
      cleanup(); set({ error: e.payload.error });
      import("@/stores/notificationStore").then(({ notify }) => {
        const state = get();
        const branch = state.branches.find((b) => `branch:${b.id}` === convId);
        notify("error", state.personaLabel ?? "에이전트", `오류: ${e.payload.error.slice(0, 80)}`, convId, {
          engine: state.getConversationEngine(convId)?.engine,
          conversationTitle: branch?.customLabel ?? branch?.label,
        });
      }).catch((e) => console.debug("[notify:error]", e));
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
