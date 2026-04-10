import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { errorMessage } from "@/lib/utils";
import { usePtyStore, isPtyEngine } from "@/stores/ptyStore";
import { getSetting } from "@/lib/appStore";
import { useToolStepsStore } from "@/stores/toolStepsStore";
import { serializeSteps } from "@/lib/toolSteps";
import type {
  SetState,
  GetState,
  QueuedAction,
  Message,
  SendWithClaudeInput,
  RoundtableRunInput,
  RoundtableParticipant,
  RtMode,
} from "./types";

/** Load context budget config from appStore and return fields for SendWithClaudeInput */
async function loadBudgetOverrides(): Promise<{ contextModeOverride?: string; contextBudgetCap?: number }> {
  const cfg = await getSetting<{ mode: string; totalCap: number }>("contextBudgetConfig", { mode: "auto", totalCap: 60000 });
  return {
    contextModeOverride: cfg.mode === "auto" ? undefined : cfg.mode,
    contextBudgetCap: cfg.totalCap === 60000 ? undefined : cfg.totalCap,
  };
}

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
  _endRun: (threadId: string) => void;
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

  _endRun: (threadId: string) => {
    set((state) => {
      const next = state.runningThreadIds.filter((id) => id !== threadId);
      return { runningThreadIds: next };
    });
    // Notify completion — skip if error was set (error handler sends its own notification)
    if (!get().error) {
      import("@/stores/notificationStore").then(({ notify }) => {
        notify("completed", "tunaFlow", "에이전트 응답이 완료되었습니다.", threadId);
      }).catch((e) => console.debug("[notify]", e));
    }
    // Post-completion background tasks — staggered to avoid blocking main thread.
    // These are synchronous Tauri commands that can take seconds (Claude API, rawq embed).
    setTimeout(() => invoke("compress_conversation_memory", { conversationId: threadId }).catch((e) => console.error("[bg] compress_memory failed:", e)), 500);
    setTimeout(() => invoke("refresh_session_links", { conversationId: threadId }).catch((e) => console.error("[bg] refresh_session_links failed:", e)), 1500);
    setTimeout(() => invoke("index_conversation_chunks", { conversationId: threadId }).catch((e) => console.error("[bg] index_chunks failed:", e)), 3000);
    setTimeout(() => {
      const projectKey = get().selectedProjectKey;
      if (projectKey) {
        invoke<{ path?: string }>("get_project", { key: projectKey }).then((p) => {
          if (p?.path) invoke("start_rawq_index", { projectPath: p.path }).catch((e) => console.error("[bg] rawq_index failed:", e));
        }).catch((e) => console.error("[bg] get_project failed:", e));
      }
    }, 5000);

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

  sendWithEngine: async (engine: string, prompt: string, model?: string, systemPrompt?: string) => {
    const { selectedProjectKey, selectedConversationId, runningThreadIds } = get();
    if (!selectedProjectKey || !selectedConversationId) return;

    // PTY shortcut: if a PTY session is active for this engine, route through it
    if (isPtyEngine(engine)) {
      const ptySession = usePtyStore.getState().getSession(engine);
      if (ptySession !== null) {
        await sendViaPty(set, get, prompt, ptySession, selectedConversationId, engine);
        return;
      }
    }

    const config = ENGINE_CONFIGS[engine] ?? ENGINE_CONFIGS.claude;

    if (runningThreadIds.includes(selectedConversationId)) {
      get()._enqueue(selectedConversationId, prompt.slice(0, 30), () =>
        get().sendWithEngine(engine, prompt, model, systemPrompt),
      );
      return;
    }

    get()._startRun(selectedConversationId);
    const now = Date.now();
    set((state) => ({
      error: null, // Clear previous error banner on new send
      messages: [
        ...state.messages,
        { id: `temp-user-${now}`, conversationId: selectedConversationId, role: "user", content: prompt, timestamp: now, status: "done" },
        { id: `temp-thinking-${now}`, conversationId: selectedConversationId, role: "assistant", content: "", timestamp: now, status: "streaming", engine: config.engineKey, model },
      ],
    }));

    // Helper: replace placeholder with real message on first event, update content on subsequent
    const replaceOrUpdate = (messageId: string, text: string) => {
      set((state) => {
        const existing = state.messages.find((m) => m.id === messageId);
        if (existing) {
          return { messages: state.messages.map((m) => m.id === messageId ? { ...m, content: text } : m) };
        }
        // First event with real messageId — replace placeholder
        const withoutPlaceholder = state.messages.filter((m) => !m.id.startsWith("temp-thinking-"));
        return { messages: [...withoutPlaceholder, { id: messageId, conversationId: selectedConversationId, role: "assistant" as const, content: text, timestamp: Date.now(), status: "streaming" as const, engine: config.engineKey, model }] };
      });
    };

    // Event listeners — progress swaps placeholder, chunk updates content
    // Guard: skip UI updates if user navigated away (backend still persists to DB)
    const isStillActive = () => get().selectedConversationId === selectedConversationId;
    const eventPrefix = engine === "claude" ? "claude" : engine;
    const unlistenProgress = await listen<{ messageId: string; conversationId: string; text: string }>(
      `${eventPrefix}:progress`, (e) => {
        if (e.payload.conversationId !== selectedConversationId) return;
        // Parse tool steps from __STEP__ prefix (always, even if navigated away)
        useToolStepsStore.getState().handleProgress(e.payload.messageId, e.payload.text);
        if (!isStillActive()) return;
        // Swap the placeholder to real messageId (content stays empty → typing indicator)
        set((state) => {
          const hasReal = state.messages.some((m) => m.id === e.payload.messageId);
          if (hasReal) return state; // already swapped
          const withoutPlaceholder = state.messages.filter((m) => !m.id.startsWith("temp-thinking-"));
          return { messages: [...withoutPlaceholder, { id: e.payload.messageId, conversationId: selectedConversationId, role: "assistant" as const, content: "", timestamp: Date.now(), status: "streaming" as const, engine: config.engineKey, model }] };
        });
      },
    );
    // Throttle chunk updates to ~5 per second (200ms) to reduce re-renders during streaming
    let pendingChunk: { messageId: string; text: string } | null = null;
    let chunkTimer: ReturnType<typeof setTimeout> | null = null;
    const flushChunk = () => {
      if (pendingChunk && isStillActive()) { replaceOrUpdate(pendingChunk.messageId, pendingChunk.text); }
      pendingChunk = null;
      chunkTimer = null;
    };
    const unlistenChunk = config.hasChunkEvent
      ? await listen<{ messageId: string; conversationId: string; text: string }>(
          `${eventPrefix}:chunk`, (e) => {
            if (e.payload.conversationId !== selectedConversationId) return;
            pendingChunk = e.payload;
            if (!chunkTimer) chunkTimer = setTimeout(flushChunk, 200);
          },
        )
      : () => {};

    const cleanup = () => {
      // Flush any pending throttled chunk before cleanup
      if (chunkTimer) { clearTimeout(chunkTimer); chunkTimer = null; }
      flushChunk();
      unlistenProgress(); unlistenChunk(); unlistenDone(); unlistenErr();
    };

    const unlistenDone = await listen<{ messageId: string; conversationId: string; durationMs?: number; inputTokens?: number; outputTokens?: number; costUsd?: number }>("agent:completed", async (e) => {
      if (e.payload.conversationId !== selectedConversationId) return;
      // Discard pending chunk BEFORE cleanup — DB reload has final content.
      // Without this, flushChunk() calls set(status:'streaming') which races
      // with the later set(messages: enriched) where status='done'.
      pendingChunk = null;
      cleanup();
      // Save tool steps to progressContent for lazy-load display
      const tsStore = useToolStepsStore.getState();
      const steps = tsStore.getSteps(e.payload.messageId);
      if (steps.length > 0) {
        invoke("save_progress_content", { messageId: e.payload.messageId, progressContent: serializeSteps(steps) }).catch((e) => console.debug("[save-steps]", e));
        tsStore.clear(e.payload.messageId);
      }
      // Always reload from DB, apply atomically inside set to avoid race conditions
      const freshMessages = await invoke<Message[]>("list_messages", { conversationId: selectedConversationId });
      // Merge runtime metadata (duration, tokens) into the completed message
      const { messageId, durationMs, inputTokens, outputTokens, costUsd } = e.payload;
      const enriched = freshMessages.map((m) =>
        m.id === messageId ? { ...m, durationMs, inputTokens, outputTokens, costUsd } : m
      );
      set((state) => {
        if (state.selectedConversationId === selectedConversationId) {
          return { messages: enriched };
        }
        const stale = new Set(state._staleConversations);
        stale.add(selectedConversationId);
        return { _staleConversations: stale };
      });
      // Check for tool-request markers in the completed message → auto follow-up.
      // _endRun is deferred until after tool-request handling to prevent idle↔running flicker.
      const lastMsg = enriched.find((m) => m.id === messageId);
      if (lastMsg?.role === "assistant") {
        try {
          const { extractToolRequests } = await import("@/lib/planProposalParser");
          const requests = extractToolRequests(lastMsg.content);
          if (requests.length > 0) {
            const { executeToolRequests } = await import("@/lib/toolRequestHandler");
            const followUp = await executeToolRequests(requests);
            if (followUp) {
              // sendWithEngine calls _startRun internally, so _endRun first to reset
              get()._endRun(selectedConversationId);
              get().sendWithEngine(engine, followUp, model);
              return; // _endRun will be called by the new sendWithEngine's completion
            }
          }
        } catch (err) {
          console.warn("[tool-request]", err);
        }
      }

      get()._endRun(selectedConversationId);
    });

    const unlistenErr = await listen<{ messageId: string; conversationId: string; error: string }>("agent:error", async (e) => {
      if (e.payload.conversationId !== selectedConversationId) return;
      pendingChunk = null;
      cleanup();
      import("@/stores/notificationStore").then(({ notify }) => {
        notify("error", "tunaFlow", `에이전트 오류: ${e.payload.error.slice(0, 100)}`, selectedConversationId);
      }).catch(() => {});
      const freshMessages = await invoke<Message[]>("list_messages", { conversationId: selectedConversationId });
      set((state) => {
        if (state.selectedConversationId === selectedConversationId) {
          return { error: e.payload.error, messages: freshMessages };
        }
        const stale = new Set(state._staleConversations);
        stale.add(selectedConversationId);
        return { _staleConversations: stale };
      });
      get()._endRun(selectedConversationId);
    });

    try {
      const bo = await loadBudgetOverrides();
      // Resolve phase-based workflow skills
      const planPhase = await invoke<string | null>("get_active_plan_phase", { conversationId: selectedConversationId }).catch(() => null);
      const effectiveSkills = get().getEffectiveSkills(planPhase, prompt);
      const input: SendWithClaudeInput = {
        projectKey: selectedProjectKey,
        conversationId: selectedConversationId,
        prompt, model, systemPrompt,
        activeSkills: effectiveSkills,
        crossSessionIds: get().crossSessionIds,
        personaFragment: get().personaFragment ?? undefined,
        personaLabel: get().personaLabel ?? undefined,
        ...bo,
      };
      await invoke<{ messageId: string }>(config.command, { input });
    } catch (e) {
      cleanup();
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
  let pendingRtChunk: Map<string, string> = new Map();
  let rtChunkTimer: ReturnType<typeof setTimeout> | null = null;
  const flushRtChunk = () => {
    rtChunkTimer = null;
    if (!isStillActive() || pendingRtChunk.size === 0) { pendingRtChunk.clear(); return; }
    const batch = new Map(pendingRtChunk);
    pendingRtChunk.clear();
    set((state) => ({
      messages: state.messages.map((m) => {
        const text = batch.get(m.id);
        return text !== undefined ? { ...m, content: text } : m;
      }),
    }));
  };
  const ulChunk = await listen<{ messageId: string; conversationId: string; text: string }>(
    "roundtable:chunk", (e) => {
      if (e.payload.conversationId !== selectedConversationId) return;
      pendingRtChunk.set(e.payload.messageId, e.payload.text);
      if (!rtChunkTimer) rtChunkTimer = setTimeout(flushRtChunk, 200);
    },
  );

  const cleanup = () => {
    if (rtChunkTimer) { clearTimeout(rtChunkTimer); rtChunkTimer = null; }
    pendingRtChunk.clear();
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
      notify("error", "tunaFlow", `에이전트 오류: ${e.payload.error.slice(0, 100)}`, selectedConversationId);
    }).catch(() => {});
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

// ─── PTY interactive mode ────────────────────────────────────────────────────
// Routes a chat message through the active Claude PTY session instead of -p mode.
// Output is streamed via pty:output events, ANSI-stripped, and displayed as a chat message.

async function sendViaPty(
  set: SetState, get: GetState,
  prompt: string, sessionId: number, conversationId: string, engine: string = "claude",
) {
  const { listen: listenEvent } = await import("@tauri-apps/api/event");

  if (get().runningThreadIds.includes(conversationId)) {
    get()._enqueue(conversationId, prompt.slice(0, 30), () =>
      get().sendWithEngine(engine, prompt),
    );
    return;
  }

  get()._startRun(conversationId);
  const now = Date.now();

  // Save user message to DB first
  let userMsgId: string;
  try {
    const userMsg = await invoke<Message>("append_user_message", {
      input: { conversationId, content: prompt },
    });
    userMsgId = userMsg.id;
  } catch {
    userMsgId = `temp-user-${now}`;
  }

  const asstMsgId = `pty-${now}`;

  // Add messages to store
  set((state) => ({
    error: null,
    messages: [
      ...state.messages,
      { id: userMsgId, conversationId, role: "user" as const, content: prompt, timestamp: now, status: "done" as const },
      { id: asstMsgId, conversationId, role: "assistant" as const, content: "", timestamp: now, status: "streaming" as const, engine: (ENGINE_CONFIGS[engine] ?? ENGINE_CONFIGS.claude).engineKey },
    ],
  }));

  const isStillActive = () => get().selectedConversationId === conversationId;

  // Clear any stale capture state from previous run
  usePtyStore.getState().endCapture();
  // Capture starts after prompt is sent (below) — not here, to skip echo

  // PTY mode: show status during streaming, extract response after completion.
  // TUI output is too noisy for real-time chat display.
  let finalized = false;

  // Status update — show "thinking" / "responding" in chat
  const setStatus = (status: string) => {
    if (isStillActive()) {
      set((state) => ({
        messages: state.messages.map((m) =>
          m.id === asstMsgId ? { ...m, progressContent: status } : m
        ),
      }));
    }
  };

  // Outbox file path — agent writes response here, tunaFlow polls for it
  const runId = `${Date.now()}`;
  const outboxPath = `.tunaflow/outbox/${runId}.md`;
  const projectPath = usePtyStore.getState().sessions.values().next().value?.projectPath || "";
  const fullOutboxPath = `${projectPath}/${outboxPath}`;

  // Status indicator via pty:screen (visual only, not for completion)
  const ulScreen = await listenEvent<{ sessionId: number; data: string }>("pty:screen", (e) => {
    if (e.payload.sessionId !== sessionId || finalized) return;
    if (/⏺/.test(e.payload.data)) setStatus("responding...");
    else if (/[✻✢✳✶✽]/.test(e.payload.data)) setStatus("thinking...");
  });

  const finalize = async (cleaned: string) => {
    ulScreen();
    usePtyStore.getState().endCapture();

    // Save assistant message to DB
    try {
      const savedMsg = await invoke<Message>("append_assistant_message", {
        input: { conversationId, content: cleaned, engine: (ENGINE_CONFIGS[engine] ?? ENGINE_CONFIGS.claude).engineKey, model: null, status: "done" },
      });
      // Replace temp ID with real DB ID
      if (isStillActive()) {
        set((state) => ({
          messages: state.messages.map((m) =>
            m.id === asstMsgId ? { ...savedMsg, content: cleaned } : m
          ),
        }));
      }
    } catch (err) {
      console.error("[pty] failed to save message:", err);
      // Still show in UI even if DB save fails
      if (isStillActive()) {
        set((state) => ({
          messages: state.messages.map((m) =>
            m.id === asstMsgId ? { ...m, content: cleaned, status: "done" as const } : m
          ),
        }));
      }
    }

    get()._endRun(conversationId);
  };

  // Send prompt to PTY stdin (append completion marker instruction)
  // Send prompt via bracket paste, then Enter separately after a short delay.
  // Claude Code TUI needs paste to complete before receiving Enter.
  try {
    // 1. Paste the prompt with outbox instruction
    const fullPrompt = `${prompt}\n\n[IMPORTANT: Write your complete response to the file "${outboxPath}". Create the directory if needed. This is how tunaFlow reads your response.]`;
    await invoke("pty_write", { sessionId, data: `\x1b[200~${fullPrompt}\x1b[201~` });
    // 2. Wait for TUI to process paste
    await new Promise((r) => setTimeout(r, 150));
    // 3. Submit with Enter
    await invoke("pty_write", { sessionId, data: "\r" });

    // 4. Poll for outbox file (agent writes response here)
    setStatus("waiting for response...");
    const { readTextFile } = await import("@tauri-apps/plugin-fs");
    for (let attempt = 0; attempt < 120; attempt++) { // Max 2 minutes
      await new Promise((r) => setTimeout(r, 1000));
      if (finalized) break;
      try {
        const content = await readTextFile(fullOutboxPath);
        if (content && content.trim().length > 0) {
          finalized = true;
          await finalize(content.trim());
          return;
        }
      } catch { /* file not yet created */ }
    }

    // Timeout — no file after 2 minutes
    if (!finalized) {
      finalized = true;
      set((state) => ({
        messages: state.messages.map((m) =>
          m.id === asstMsgId ? { ...m, content: "(응답 파일 생성 대기 시간 초과)", status: "error" as const } : m
        ),
      }));
      ulScreen();
      get()._endRun(conversationId);
    }
  } catch (err) {
    ulScreen();
    set((state) => ({
      error: errorMessage(err),
      messages: state.messages.map((m) =>
        m.id === asstMsgId ? { ...m, content: errorMessage(err), status: "error" as const } : m
      ),
    }));
    get()._endRun(conversationId);
  }
}
