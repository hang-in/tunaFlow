import { invoke } from "@tauri-apps/api/core";
import { errorMessage } from "@/lib/utils";
import { ENGINE_CONFIGS } from "@/lib/engineConfig";
import { usePtyStore, isPtyEngine } from "@/stores/ptyStore";
import { sendMessageViaPty } from "./ptyMessageSender";
import {
  handleToolRequests,
  setupStreamLifecycle,
  extractAndPersistFollowup,
  type StreamLifecycleHandle,
} from "./agentStreamHelper";
import { autoSyncImplCompletion, autoDetectReviewVerdict } from "@/lib/workflow/branchSync";
import { runThreadRoundtable } from "./threadRtRunner";
import { resolveModel, createPlaceholders, buildSendInput } from "@/lib/sendPipeline";
import type {
  SetState,
  GetState,
  Branch,
  Conversation,
  Message,
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
  sendThreadMessage: (prompt: string, engine?: string, model?: string, opts?: { userMessageId?: string }) => Promise<void>;
  sendThreadRoundtable: (prompt: string, participants: RoundtableParticipant[], mode?: RtMode, opts?: { autoSynthesize?: boolean }) => Promise<void>;
  sendThreadRoundtableFollowup: (prompt: string, participants: RoundtableParticipant[], mode?: RtMode, opts?: { autoSynthesize?: boolean }) => Promise<void>;
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

      // If parent conversation is not currently selected, load it via
      // the owner (conversationSlice.selectConversation) instead of
      // bulk-writing into foreign slices. If the parent row is missing
      // from the conversations list (e.g. the panel never opened it),
      // fetch + register it first.
      if (parentConvId && parentConvId !== get().selectedConversationId) {
        if (!get().conversations.some((c) => c.id === parentConvId)) {
          try {
            const parentConv = await invoke<Conversation>("get_conversation", { id: parentConvId });
            get().ensureConversation(parentConv);
          } catch (e) { console.debug("[thread] parent conv fetch:", e); }
        }
        await get().selectConversation(parentConvId);
        // Re-find branch from fresh data (selectConversation reloaded branches)
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
      // Own write: thread drawer state.
      set({
        threadBranchId: branchId,
        threadBranchConvId: branchConvId,
        threadMessages: branchMessages,
        threadBranchLabel: branch?.customLabel ?? branch?.label ?? branchId.slice(0, 12),
        threadParentMessage: parentMsg,
      });
      // Sibling: shadow conversation row registration (needed for RT
      // detection) goes through the owner.
      get().ensureConversation(branchConv);

      // Auto-switch center tab when opening a plan-linked branch
      import("@/lib/api/plans").then(async ({ findPlanByBranch }) => {
        const plan = await findPlanByBranch(branchId);
        if (plan && (plan.implementationBranchId === branchId || plan.reviewBranchId === branchId)) {
          window.dispatchEvent(new CustomEvent("tunaflow:switch-tab", { detail: "workflow" }));
          // Also switch to the correct workflow stage based on plan phase
          const PHASE_TO_STAGE: Record<string, string> = {
            // drafting + subtask_review → "plan-check" 로 통합 (s37)
            drafting: "plan-check", subtask_review: "plan-check", approval: "dev",
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

  sendThreadMessage: async (prompt: string, engine?: string, model?: string, opts?: { userMessageId?: string; imagePaths?: string[] }) => {
    const { threadBranchConvId, threadBranchId, selectedProjectKey } = get();
    if (!threadBranchConvId || !selectedProjectKey || !threadBranchId) return;
    const convId = threadBranchConvId; // narrowed: string (guaranteed by guard above)
    const engineKey = engine ?? "claude";

    if (!model) {
      model = resolveModel(get(), convId, engineKey);
      if (!model) {
        console.warn(`[sendThreadMessage] model unresolved for engine=${engineKey} conv=${convId.slice(0, 12)}…`);
      }
    }

    // Queue if already running
    if (get().runningThreadIds.includes(convId)) {
      get()._enqueue(convId, prompt.slice(0, 30), () =>
        get().sendThreadMessage(prompt, engine, model, opts),
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
                  get()._endRun(convId, { silent: true });
                  // Persist follow-up as system message (auto-generated, not user-typed).
                  const sysMsgId = await invoke<string>("persist_system_msg", {
                    conversationId: convId,
                    content: followUp,
                  }).catch((e) => { console.warn("[pty-thread] persist_system_msg failed:", e); return null; });
                  get().sendThreadMessage(
                    followUp,
                    saved?.engine ?? "claude",
                    saved?.model ?? undefined,
                    sysMsgId ? { userMessageId: sysMsgId } : undefined,
                  );
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
    const threadEngineConfig = ENGINE_CONFIGS[engine ?? "claude"] ?? ENGINE_CONFIGS.claude;
    const [firstMsg, thinkingMsg] = createPlaceholders({
      convId,
      prompt,
      engineKey: threadEngineConfig.engineKey,
      model,
      progressLabel: threadEngineConfig.label,
      userMessageId: opts?.userMessageId,
      now,
    });
    set((state) => ({
      threadMessages: [...state.threadMessages, firstMsg, thinkingMsg],
    }));

    const input = await buildSendInput({
      projectKey: selectedProjectKey,
      conversationId: convId,
      prompt,
      model,
      personaFragment: get().personaFragment ?? undefined,
      personaLabel: get().personaLabel ?? undefined,
      crossSessionIds: get().crossSessionIds,
      getEffectiveSkills: get().getEffectiveSkills,
      opts,
    });

    // Branch drawer writes progress to `progressContent` and chunks to
    // `content` — two fields of the same row — so the spinner text stays
    // visible while streamed content appears beneath it.
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

    // Guard: only update UI if this branch is still the active thread
    // (prevents cross-project contamination when user switches projects
    // mid-stream).
    const isActiveThread = () => get().threadBranchConvId === convId;

    let lifecycle: StreamLifecycleHandle | undefined;

    lifecycle = await setupStreamLifecycle({
      convId,
      engineKey,
      hasChunkEvent: threadEngineConfig.hasChunkEvent,
      onProgress: (p) => {
        if (!isActiveThread()) return;
        replaceOrAdd(p.messageId, "progressContent", p.text);
      },
      onChunk: (p) => {
        if (!isActiveThread()) return;
        replaceOrAdd(p.messageId, "content", p.text);
      },
      onCompleted: async (p) => {
        lifecycle?.cleanup();
        const threadMessages = await invoke<Message[]>("list_messages", { conversationId: convId });
        set({ threadMessages });

        // Tool-request follow-up — _endRun runs with { silent: true } before
        // the recursive send so the UI does not flash idle between turns.
        const lastMsg = threadMessages.find((m) => m.id === p.messageId);
        const followup = await extractAndPersistFollowup(lastMsg, convId);
        let toolRequestHandled = false;
        if (followup) {
          const saved = get().getConversationEngine(convId);
          get()._endRun(convId, { silent: true });
          get().sendThreadMessage(
            followup.followUp,
            saved?.engine ?? "claude",
            saved?.model ?? undefined,
            followup.sysMsgId ? { userMessageId: followup.sysMsgId } : undefined,
          );
          toolRequestHandled = true;
        }

        // Workflow auto-sync — scoped to branch path only (main chat never
        // drives subtask status transitions). Keep as direct calls here so
        // that their future extraction (Finding 1-5) is visible.
        autoSyncImplCompletion(convId, threadMessages);
        autoDetectReviewVerdict(convId, threadMessages);

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

        if (!toolRequestHandled) get()._endRun(convId, { silent: true });
      },
      onError: async (p) => {
        lifecycle?.cleanup();
        set({ error: p.error });
        import("@/stores/notificationStore").then(({ notify }) => {
          const state = get();
          const branch = state.branches.find((b) => `branch:${b.id}` === convId);
          notify("error", state.personaLabel ?? "에이전트", `오류: ${p.error.slice(0, 80)}`, convId, {
            engine: state.getConversationEngine(convId)?.engine,
            conversationTitle: branch?.customLabel ?? branch?.label,
          });
        }).catch((e) => console.debug("[notify:error]", e));
        const threadMessages = await invoke<Message[]>("list_messages", { conversationId: convId });
        set({ threadMessages });
        get()._endRun(convId);
      },
    });

    try {
      const config = ENGINE_CONFIGS[engineKey] ?? ENGINE_CONFIGS.claude;
      await invoke<{ messageId: string }>(config.command, { input });
    } catch (e) {
      lifecycle?.cleanup();
      set((state) => ({ error: errorMessage(e), threadMessages: state.threadMessages.filter((m) => !m.id.startsWith("temp-thinking-")) }));
      get()._endRun(convId);
    }
  },

  sendThreadRoundtable: async (prompt: string, participants: RoundtableParticipant[], mode?: RtMode, opts?: { autoSynthesize?: boolean }) => {
    await runThreadRoundtable(set, get, "start_roundtable_run", prompt, participants, mode, opts);
  },

  sendThreadRoundtableFollowup: async (prompt: string, participants: RoundtableParticipant[], mode?: RtMode, opts?: { autoSynthesize?: boolean }) => {
    await runThreadRoundtable(set, get, "start_roundtable_followup", prompt, participants, mode, opts);
  },
});
