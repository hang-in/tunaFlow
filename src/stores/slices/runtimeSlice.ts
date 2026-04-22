import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { errorMessage } from "@/lib/utils";
import { usePtyStore, isPtyEngine } from "@/stores/ptyStore";
import { sendMessageViaPty } from "./ptyMessageSender";
import { createRtChunkBatcher, createSingleChunkThrottler } from "./streamingUtils";
import { resolveModel, createPlaceholders, buildSendInput } from "@/lib/sendPipeline";
import {
  setupStreamLifecycle,
  extractAndPersistFollowup,
  type StreamLifecycleHandle,
} from "./agentStreamHelper";
import type {
  SetState,
  GetState,
  QueuedAction,
  Message,
  RoundtableRunInput,
  RoundtableParticipant,
  RtMode,
} from "./types";

// ─── Engine configuration — canonical source: lib/engineConfig ──────────────
import { ENGINE_CONFIGS } from "@/lib/engineConfig";
export { ENGINE_CONFIGS };
export type { EngineConfig } from "@/lib/engineConfig";

// ─── Slice interface ────────────────────────────────────────────────────────

export interface RuntimeSlice {
  runningThreadIds: string[];
  messageQueue: QueuedAction[];
  error: string | null;
  _startRun: (threadId: string) => void;
  _endRun: (threadId: string, opts?: { silent?: boolean }) => void;
  _enqueue: (threadId: string, label: string, execute: () => Promise<void>) => void;
  cancelOperation: (threadId?: string) => Promise<void>;
  sendMessage: (prompt: string, model?: string, systemPrompt?: string) => Promise<void>;
  sendWithEngine: (engine: string, prompt: string, model?: string, systemPrompt?: string) => Promise<void>;
  sendFollowup: (engine: string, sourceType: string, sourceContent: string, goal?: string) => Promise<void>;
  sendRoundtable: (prompt: string, participants: RoundtableParticipant[], mode?: RtMode) => Promise<void>;
  sendRoundtableFollowup: (prompt: string, participants: RoundtableParticipant[], mode?: RtMode) => Promise<void>;
}

// ─── Slice implementation ───────────────────────────────────────────────────

export const createRuntimeSlice = (set: SetState, get: GetState): RuntimeSlice => ({
  runningThreadIds: [],
  messageQueue: [],
  error: null,

  // ─── Thread run helpers ──────────────────────────────────────────────
  _startRun: (threadId: string) => {
    set((state) => ({
      runningThreadIds: [...state.runningThreadIds.filter((id) => id !== threadId), threadId],
      error: null,
    }));
  },

  _endRun: (threadId: string, opts?: { silent?: boolean }) => {
    set((state) => {
      const next = state.runningThreadIds.filter((id) => id !== threadId);
      return { runningThreadIds: next };
    });
    // Notify completion — skip if error was set (error handler sends its own notification),
    // or if caller already notified (silent: true) to prevent double-notification.
    if (!get().error && !opts?.silent) {
      import("@/stores/notificationStore").then(({ notify }) => {
        const state = get();
        const conv = state.conversations.find((c) => c.id === threadId);
        const engine = state.getConversationEngine(threadId)?.engine;
        const lastAsst = state.messages.filter((m) => m.conversationId === threadId && m.role === "assistant").slice(-1)[0];
        const preview = lastAsst?.content?.replace(/\n+/g, " ").slice(0, 80);
        notify("completed", state.personaLabel ?? "에이전트", "응답 완료", threadId, {
          engine,
          conversationTitle: conv?.customLabel ?? conv?.label,
          preview,
        });
      }).catch((e) => console.debug("[notify]", e));
    }
    // Post-completion background tasks: memory compression, session links, vector indexing, rawq.
    // Delegated to backend command to avoid setTimeout chain and decouple from UI thread.
    invoke("on_run_completed", { conversationId: threadId }).catch((e) =>
      console.error("[bg] on_run_completed failed:", e)
    );

    // Drain next queued action for this thread
    const queue = get().messageQueue;
    const nextIdx = queue.findIndex((q) => q.threadId === threadId);
    if (nextIdx >= 0) {
      const next = queue[nextIdx];
      set({ messageQueue: queue.filter((_, i) => i !== nextIdx) });
      next.execute();
    }
  },

  _enqueue: (threadId: string, label: string, execute: () => Promise<void>) => {
    set((state) => ({
      messageQueue: [...state.messageQueue, { threadId, label, execute }],
    }));
  },

  // ─── Unified engine send ─────────────────────────────────────────────

  sendMessage: async (prompt: string, model?: string, systemPrompt?: string) => {
    await get().sendWithEngine("claude", prompt, model, systemPrompt);
  },

  sendWithEngine: async (engine: string, prompt: string, model?: string, systemPrompt?: string, opts?: { userMessageId?: string; imagePaths?: string[] }) => {
    const { selectedProjectKey, selectedConversationId, runningThreadIds } = get();
    if (!selectedProjectKey || !selectedConversationId) return;

    if (!model) {
      model = resolveModel(get(), selectedConversationId, engine);
      if (!model) {
        console.warn(`[sendWithEngine] model unresolved for engine=${engine} conv=${selectedConversationId.slice(0, 12)}…`);
      }
    }

    // Queue if already running
    if (runningThreadIds.includes(selectedConversationId)) {
      get()._enqueue(selectedConversationId, prompt.slice(0, 30), () =>
        get().sendWithEngine(engine, prompt, model, systemPrompt, opts),
      );
      return;
    }

    // PTY shortcut: opt-in only. PTY is now reserved for the interactive terminal
    // panel (VTE). Main chat send routes through SDK (if API key) or `-p` CLI to
    // avoid Enter-queue hangs under load (e.g. bge-m3 IO contention).
    const { getSetting: getAppSetting } = await import("@/lib/appStore");
    const ptyEnabled = await getAppSetting<boolean>("ptyEnabled", false);
    if (ptyEnabled && isPtyEngine(engine)) {
      const { waitForPtyReady } = await import("@/stores/slices/conversationSlice");
      await waitForPtyReady(selectedConversationId, 20_000);
      const ptySession = usePtyStore.getState().getSession(engine);
      if (ptySession !== null) {
        try {
          await sendMessageViaPty(set, get, prompt, ptySession, selectedConversationId, engine, {
            messageTarget: "messages",
            isActiveCheck: () => get().selectedConversationId === selectedConversationId,
            personaLabel: get().personaLabel ?? undefined,
          });
          return;
        } catch (ptyErr) {
          console.error("[pty] sendViaPty failed, falling back to -p mode:", ptyErr);
          usePtyStore.getState().clearSession(engine as import("@/stores/ptyStore").PtyEngine);
          import("sonner").then(({ toast }) => toast.warning("PTY 오류 — CLI 모드로 전환")).catch(() => {});
        }
      }
    }

    const config = ENGINE_CONFIGS[engine] ?? ENGINE_CONFIGS.claude;

    get()._startRun(selectedConversationId);
    const now = Date.now();
    const persona = get().personaLabel ?? undefined;
    const [firstMsg, thinkingMsg] = createPlaceholders({
      convId: selectedConversationId,
      prompt,
      engineKey: config.engineKey,
      model,
      persona,
      userMessageId: opts?.userMessageId,
      now,
    });
    set((state) => ({
      error: null,
      messages: [...state.messages, firstMsg, thinkingMsg],
    }));

    // Helper: replace placeholder with real message on first event, update content on subsequent
    const isStillActive = () => get().selectedConversationId === selectedConversationId;

    // Replace-or-update used by both placeholder swap and chunk flush. Once
    // the real messageId arrives, subsequent calls update its content; the
    // first call drops the temp-thinking placeholder and inserts the real
    // streaming row in its place.
    const replaceOrUpdate = (messageId: string, text: string) => {
      set((state) => {
        const existing = state.messages.find((m) => m.id === messageId);
        if (existing) {
          return { messages: state.messages.map((m) => m.id === messageId ? { ...m, content: text } : m) };
        }
        const withoutPlaceholder = state.messages.filter((m) => !m.id.startsWith("temp-thinking-"));
        return { messages: [...withoutPlaceholder, { id: messageId, conversationId: selectedConversationId, role: "assistant" as const, content: text, timestamp: Date.now(), status: "streaming" as const, engine: config.engineKey, model, persona }] };
      });
    };

    // Main-chat throttles chunk updates to ~5 per second to reduce re-renders.
    // Branch drawer does not throttle (see threadSlice).
    const chunkThrottle = createSingleChunkThrottler(
      isStillActive,
      (messageId, text) => replaceOrUpdate(messageId, text),
    );

    const eventPrefix = engine === "claude" ? "claude" : engine;
    let lifecycle: StreamLifecycleHandle | undefined;
    const cleanupAll = () => {
      // Discard pending throttled chunk BEFORE listener cleanup — DB reload
      // below has the final content, and a late flush would race the
      // set(status:'done') applied via the completed DB reload.
      chunkThrottle.cleanup();
      lifecycle?.cleanup();
    };

    lifecycle = await setupStreamLifecycle({
      convId: selectedConversationId,
      engineKey: eventPrefix,
      hasChunkEvent: config.hasChunkEvent,
      onProgress: (p) => {
        if (!isStillActive()) return;
        // Swap the placeholder to the real messageId (content stays empty →
        // typing indicator). First progress event is the swap trigger.
        set((state) => {
          const hasReal = state.messages.some((m) => m.id === p.messageId);
          if (hasReal) return state;
          const withoutPlaceholder = state.messages.filter((m) => !m.id.startsWith("temp-thinking-"));
          return { messages: [...withoutPlaceholder, { id: p.messageId, conversationId: selectedConversationId, role: "assistant" as const, content: "", timestamp: Date.now(), status: "streaming" as const, engine: config.engineKey, model, persona }] };
        });
      },
      onChunk: (p) => chunkThrottle.handleChunk(p),
      onCompleted: async (p) => {
        cleanupAll();
        const freshMessages = await invoke<Message[]>("list_messages", { conversationId: selectedConversationId });
        const { messageId, durationMs, inputTokens, outputTokens, costUsd } = p;
        const enriched = freshMessages.map((m) =>
          m.id === messageId ? { ...m, durationMs, inputTokens, outputTokens, costUsd } : m
        );
        // `messages` stays a runtime-owned hot-path write (streaming
        // placeholder swap → chunk patch → final reload). `_staleConversations`
        // is conversationSlice-owned and goes through its action so the
        // slice-boundary rule isn't violated.
        if (get().selectedConversationId === selectedConversationId) {
          set({ messages: enriched });
        } else {
          get().markConversationStale(selectedConversationId);
        }
        // Tool-request follow-up — _endRun deferred until after handling to
        // prevent idle↔running flicker. Main chat recurses via sendWithEngine.
        const lastMsg = enriched.find((m) => m.id === messageId);
        const followup = await extractAndPersistFollowup(lastMsg, selectedConversationId);
        if (followup) {
          get()._endRun(selectedConversationId);
          get().sendWithEngine(
            engine,
            followup.followUp,
            model,
            undefined,
            followup.sysMsgId ? { userMessageId: followup.sysMsgId } : undefined,
          );
          return;
        }
        get()._endRun(selectedConversationId);
      },
      onError: async (p) => {
        cleanupAll();
        import("@/stores/notificationStore").then(({ notify }) => {
          const state = get();
          const conv = state.conversations.find((c) => c.id === selectedConversationId);
          notify("error", state.personaLabel ?? "에이전트", `오류: ${p.error.slice(0, 80)}`, selectedConversationId, {
            engine: state.getConversationEngine(selectedConversationId)?.engine,
            conversationTitle: conv?.customLabel ?? conv?.label,
          });
        }).catch((e) => console.debug("[notify:error]", e));
        const freshMessages = await invoke<Message[]>("list_messages", { conversationId: selectedConversationId });
        if (get().selectedConversationId === selectedConversationId) {
          set({ error: p.error, messages: freshMessages });
        } else {
          get().markConversationStale(selectedConversationId);
        }
        get()._endRun(selectedConversationId);
      },
    });

    try {
      const input = await buildSendInput({
        projectKey: selectedProjectKey,
        conversationId: selectedConversationId,
        prompt,
        engine,
        model,
        systemPrompt,
        personaFragment: get().personaFragment ?? undefined,
        personaLabel: get().personaLabel ?? undefined,
        crossSessionIds: get().crossSessionIds,
        getEffectiveSkills: get().getEffectiveSkills,
        opts,
      });
      await invoke<{ messageId: string }>(config.command, { input });
    } catch (e) {
      cleanupAll();
      set((state) => ({
        error: errorMessage(e),
        messages: state.messages
          .filter((m) => !m.id.startsWith("temp-thinking-"))
          .map((m) => m.status === "streaming" ? { ...m, status: "error", content: m.content || errorMessage(e) } : m),
      }));
      get()._endRun(selectedConversationId);
    }
  },

  sendFollowup: async (engine: string, sourceType: string, sourceContent: string, goal?: string) => {
    const maxLen = sourceType === "artifact" ? 8000 : 2000;
    const truncated = sourceContent.length > maxLen ? sourceContent.slice(0, maxLen) + "\n\n[... truncated]" : sourceContent;
    const goalLine = goal ? `\nGoal: ${goal}` : "";
    const prompt = `[Follow-up: ${sourceType}]${goalLine}\n\n${truncated}\n\n위 내용을 기반으로 작업해주세요.`;
    await get().sendWithEngine(engine, prompt);
  },

  // ─── Roundtable sends ────────────────────────────────────────────────

  sendRoundtable: async (prompt: string, participants: RoundtableParticipant[], mode?: RtMode) => {
    await runRoundtable(set, get, "start_roundtable_run", prompt, participants, mode);
  },

  sendRoundtableFollowup: async (prompt: string, participants: RoundtableParticipant[], mode?: RtMode) => {
    await runRoundtable(set, get, "start_roundtable_followup", prompt, participants, mode);
  },

  cancelOperation: async (threadId?: string) => {
    const target = threadId ?? get().selectedConversationId;

    if (target) {
      try { await invoke("cancel_running", { conversationId: target }); } catch (e) { console.warn("[runtime] cancel_running failed (best-effort):", e); }
    }

    if (!target) {
      set({ runningThreadIds: [], error: null });
      return;
    }

    set((state) => ({
      runningThreadIds: state.runningThreadIds.filter((id) => id !== target),
      error: null,
      messageQueue: state.messageQueue.filter((q) => q.threadId !== target),
    }));

    if (target === get().selectedConversationId) {
      try {
        const messages = await invoke<Message[]>("list_messages", { conversationId: target });
        set({ messages });
      } catch (e) { console.warn("[runtime] message reload after cancel failed:", e); }
    }
  },
});

// ─── Roundtable helper (shared by run + followup) ───────────────────────────

async function runRoundtable(
  set: SetState, get: GetState, command: string,
  prompt: string, participants: RoundtableParticipant[], mode?: RtMode,
) {
  const { selectedConversationId, runningThreadIds } = get();
  if (!selectedConversationId) return;

  if (runningThreadIds.includes(selectedConversationId)) {
    get()._enqueue(selectedConversationId, prompt.slice(0, 30), () =>
      command === "start_roundtable_run"
        ? get().sendRoundtable(prompt, participants, mode)
        : get().sendRoundtableFollowup(prompt, participants, mode),
    );
    return;
  }

  get()._startRun(selectedConversationId);
  const now = Date.now();
  set((state) => ({
    messages: [
      ...state.messages,
      { id: `temp-user-${now}`, conversationId: selectedConversationId, role: "user", content: prompt, timestamp: now, status: "done" },
      { id: `temp-thinking-${now}`, conversationId: selectedConversationId, role: "assistant", content: "", progressContent: "Roundtable starting...", timestamp: now, status: "streaming", engine: "system" },
    ],
  }));

  const isStillActive = () => get().selectedConversationId === selectedConversationId;
  let placeholderCleared = false;
  const ulRT = await listen<Message>("roundtable:progress", (event) => {
    const msg = event.payload;
    if (msg.conversationId !== selectedConversationId) return;
    if (!isStillActive()) return;
    if (msg.role === "user") return;
    set((state) => {
      const idx = state.messages.findIndex((m) => m.id === msg.id);
      if (idx >= 0) {
        // Update existing message (streaming → done, or content update)
        const msgs = [...state.messages];
        msgs[idx] = msg;
        return { messages: msgs };
      }
      if (!placeholderCleared) {
        placeholderCleared = true;
        return { messages: [...state.messages.filter((m) => !m.id.startsWith("temp-thinking-")), msg] };
      }
      return { messages: [...state.messages, msg] };
    });
  });

  // Throttled roundtable:chunk listener for real-time streaming (200ms batching)
  const rtBatcher = createRtChunkBatcher(
    selectedConversationId,
    isStillActive,
    (batch) => set((state) => ({
      messages: state.messages.map((m) => {
        const text = batch.get(m.id);
        return text !== undefined ? { ...m, content: text } : m;
      }),
    })),
  );
  const ulChunk = await listen<{ messageId: string; conversationId: string; text: string }>(
    "roundtable:chunk", rtBatcher.handleChunk,
  );

  const cleanup = () => {
    rtBatcher.cleanup();
    ulRT(); ulChunk(); ulD(); ulE();
  };
  const ulD = await listen<{ conversationId: string }>("agent:completed", async (e) => {
    if (e.payload.conversationId !== selectedConversationId) return;
    cleanup();
    if (isStillActive()) {
      const messages = await invoke<Message[]>("list_messages", { conversationId: selectedConversationId });
      set({ messages });
    }
    get()._endRun(selectedConversationId);
  });
  const ulE = await listen<{ conversationId: string; error: string }>("agent:error", async (e) => {
    if (e.payload.conversationId !== selectedConversationId) return;
    cleanup();
    import("@/stores/notificationStore").then(({ notify }) => {
      const state = get();
      const conv = state.conversations.find((c) => c.id === selectedConversationId);
      notify("error", "Roundtable", `오류: ${e.payload.error.slice(0, 80)}`, selectedConversationId, {
        conversationTitle: conv?.customLabel ?? conv?.label,
      });
    }).catch((e) => console.debug("[notify:rt-error]", e));
    if (isStillActive()) {
      set({ error: e.payload.error });
      const messages = await invoke<Message[]>("list_messages", { conversationId: selectedConversationId });
      set({ messages });
    }
    get()._endRun(selectedConversationId);
  });

  try {
    await invoke<{ messageId: string }>(command, { input: { conversationId: selectedConversationId, prompt, participants, mode } });
  } catch (e) {
    cleanup(); set({ error: errorMessage(e) }); get()._endRun(selectedConversationId);
  }
}

