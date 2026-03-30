import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getSetting } from "@/lib/appStore";
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

// ─── Engine configuration map ───────────────────────────────────────────────

export interface EngineConfig {
  command: string;
  engineKey: string;
  label: string;
  hasChunkEvent: boolean;
}

export const ENGINE_CONFIGS: Record<string, EngineConfig> = {
  claude:   { command: "start_claude_stream", engineKey: "claude-code", label: "Claude initializing...", hasChunkEvent: true },
  codex:    { command: "start_codex_run",     engineKey: "codex",       label: "Codex starting...",      hasChunkEvent: true },
  gemini:   { command: "start_gemini_stream", engineKey: "gemini",      label: "Gemini initializing...", hasChunkEvent: true },
  opencode: { command: "start_opencode_run",  engineKey: "opencode",    label: "OpenCode starting...",   hasChunkEvent: false },
};

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
    // Notify if app is not focused
    if (document.hidden) {
      import("@tauri-apps/plugin-notification").then(({ sendNotification, isPermissionGranted }) => {
        isPermissionGranted().then((granted) => {
          if (granted) {
            sendNotification({ title: "tunaFlow", body: "에이전트 응답이 완료되었습니다." });
          }
        });
      }).catch(() => {});
    }
    // Fire-and-forget: compress older messages into long-term memory
    invoke("compress_conversation_memory", { conversationId: threadId }).catch(() => {});

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
      messages: [
        ...state.messages,
        { id: `temp-user-${now}`, conversationId: selectedConversationId, role: "user", content: prompt, timestamp: now, status: "done" },
        { id: `temp-thinking-${now}`, conversationId: selectedConversationId, role: "assistant", content: "", progressContent: config.label, timestamp: now, status: "streaming", engine: config.engineKey, model },
      ],
    }));

    // Helper: replace placeholder with real message on first event
    const replaceOrUpdate = (messageId: string, field: "progressContent" | "content", text: string) => {
      set((state) => {
        const existing = state.messages.find((m) => m.id === messageId);
        if (existing) {
          if (field === "progressContent") {
            const prev = existing.progressContent || "";
            return { messages: state.messages.map((m) => m.id === messageId ? { ...m, progressContent: prev ? `${prev}\n${text}` : text } : m) };
          }
          return { messages: state.messages.map((m) => m.id === messageId ? { ...m, content: text } : m) };
        }
        const withoutPlaceholder = state.messages.filter((m) => !m.id.startsWith("temp-thinking-"));
        return { messages: [...withoutPlaceholder, { id: messageId, conversationId: selectedConversationId, role: "assistant" as const, content: field === "content" ? text : "", progressContent: field === "progressContent" ? text : undefined, timestamp: Date.now(), status: "streaming" as const, engine: config.engineKey, model }] };
      });
    };

    // Event listeners — engine-specific progress/chunk + common completed/error
    const eventPrefix = engine === "claude" ? "claude" : engine;
    const unlistenProgress = await listen<{ messageId: string; text: string }>(
      `${eventPrefix}:progress`, (e) => replaceOrUpdate(e.payload.messageId, "progressContent", e.payload.text),
    );
    const unlistenChunk = config.hasChunkEvent
      ? await listen<{ messageId: string; text: string }>(
          `${eventPrefix}:chunk`, (e) => replaceOrUpdate(e.payload.messageId, "content", e.payload.text),
        )
      : () => {};

    const cleanup = () => { unlistenProgress(); unlistenChunk(); unlistenDone(); unlistenErr(); };

    const unlistenDone = await listen<{ messageId: string; conversationId: string }>("agent:completed", async (e) => {
      if (e.payload.conversationId !== selectedConversationId) return;
      cleanup();
      // Save thinking/progress to DB before reloading (display only, not in context)
      const streamingMsg = get().messages.find((m) => m.id === e.payload.messageId || m.status === "streaming");
      if (streamingMsg?.progressContent) {
        invoke("save_progress_content", { messageId: e.payload.messageId, progressContent: streamingMsg.progressContent }).catch(() => {});
      }
      const messages = await invoke<Message[]>("list_messages", { conversationId: selectedConversationId });
      set({ messages });
      get()._endRun(selectedConversationId);
    });

    const unlistenErr = await listen<{ messageId: string; conversationId: string; error: string }>("agent:error", async (e) => {
      if (e.payload.conversationId !== selectedConversationId) return;
      cleanup();
      set({ error: e.payload.error });
      const messages = await invoke<Message[]>("list_messages", { conversationId: selectedConversationId });
      set({ messages });
      get()._endRun(selectedConversationId);
    });

    try {
      const bo = await loadBudgetOverrides();
      const input: SendWithClaudeInput = {
        projectKey: selectedProjectKey,
        conversationId: selectedConversationId,
        prompt, model, systemPrompt,
        activeSkills: get().activeSkills,
        crossSessionIds: get().crossSessionIds,
        personaFragment: get().personaFragment ?? undefined,
        personaLabel: get().personaLabel ?? undefined,
        ...bo,
      };
      await invoke<{ messageId: string }>(config.command, { input });
    } catch (e) {
      cleanup();
      set((state) => ({
        error: String(e),
        messages: state.messages
          .filter((m) => !m.id.startsWith("temp-thinking-"))
          .map((m) => m.status === "streaming" ? { ...m, status: "error", content: m.content || String(e) } : m),
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
      try { await invoke("cancel_running", { conversationId: target }); } catch { /* best-effort */ }
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
      } catch { /* ignore */ }
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

  let placeholderCleared = false;
  const ulRT = await listen<Message>("roundtable:progress", (event) => {
    const msg = event.payload;
    if (msg.role === "user") return;
    set((state) => {
      if (!placeholderCleared) {
        placeholderCleared = true;
        return { messages: [...state.messages.filter((m) => !m.id.startsWith("temp-thinking-")), msg] };
      }
      return { messages: [...state.messages, msg] };
    });
  });

  const cleanup = () => { ulRT(); ulD(); ulE(); };
  const ulD = await listen<{ conversationId: string }>("agent:completed", async (e) => {
    if (e.payload.conversationId !== selectedConversationId) return;
    cleanup();
    const messages = await invoke<Message[]>("list_messages", { conversationId: selectedConversationId });
    set({ messages }); get()._endRun(selectedConversationId);
  });
  const ulE = await listen<{ conversationId: string; error: string }>("agent:error", async (e) => {
    if (e.payload.conversationId !== selectedConversationId) return;
    cleanup(); set({ error: e.payload.error });
    const messages = await invoke<Message[]>("list_messages", { conversationId: selectedConversationId });
    set({ messages }); get()._endRun(selectedConversationId);
  });

  try {
    await invoke<{ messageId: string }>(command, { input: { conversationId: selectedConversationId, prompt, participants, mode } });
  } catch (e) {
    cleanup(); set({ error: String(e) }); get()._endRun(selectedConversationId);
  }
}
