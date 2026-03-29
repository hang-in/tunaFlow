import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
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

export interface RuntimeSlice {
  isRunning: boolean;
  runningThreadIds: string[];
  messageQueue: QueuedAction[];
  error: string | null;
  _startRun: (threadId: string) => void;
  _endRun: (threadId: string) => void;
  _enqueue: (threadId: string, label: string, execute: () => Promise<void>) => void;
  cancelOperation: (threadId?: string) => Promise<void>;
  sendMessage: (prompt: string, model?: string, systemPrompt?: string) => Promise<void>;
  sendWithCodex: (prompt: string, model?: string) => Promise<void>;
  sendWithGemini: (prompt: string, model?: string) => Promise<void>;
  sendWithOpencode: (prompt: string, model?: string) => Promise<void>;
  sendFollowup: (engine: string, sourceType: string, sourceContent: string, goal?: string) => Promise<void>;
  sendRoundtable: (prompt: string, participants: RoundtableParticipant[], mode?: RtMode) => Promise<void>;
  sendRoundtableFollowup: (prompt: string, participants: RoundtableParticipant[], mode?: RtMode) => Promise<void>;
}

export const createRuntimeSlice = (set: SetState, get: GetState): RuntimeSlice => ({
  isRunning: false,
  runningThreadIds: [],
  messageQueue: [],
  error: null,

  // ─── Thread run helpers ──────────────────────────────────────────────
  _startRun: (threadId: string) => {
    set((state) => ({
      isRunning: true,
      runningThreadIds: [...state.runningThreadIds.filter((id) => id !== threadId), threadId],
      error: null,
    }));
  },

  _endRun: (threadId: string) => {
    set((state) => {
      const next = state.runningThreadIds.filter((id) => id !== threadId);
      return { isRunning: next.length > 0, runningThreadIds: next };
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

  sendMessage: async (prompt: string, model?: string, systemPrompt?: string) => {
    const { selectedProjectKey, selectedConversationId, runningThreadIds } = get();
    if (!selectedProjectKey || !selectedConversationId) return;

    if (runningThreadIds.includes(selectedConversationId)) {
      get()._enqueue(selectedConversationId, prompt.slice(0, 30), () =>
        get().sendMessage(prompt, model, systemPrompt),
      );
      return;
    }

    get()._startRun(selectedConversationId);
    const now = Date.now();
    set((state) => ({
      messages: [
        ...state.messages,
        { id: `temp-user-${now}`, conversationId: selectedConversationId, role: "user", content: prompt, timestamp: now, status: "done" },
        { id: `temp-thinking-${now}`, conversationId: selectedConversationId, role: "assistant", content: "", progressContent: "Claude initializing...", timestamp: now, status: "streaming", engine: "claude-code", model },
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
        return { messages: [...withoutPlaceholder, { id: messageId, conversationId: selectedConversationId, role: "assistant" as const, content: field === "content" ? text : "", progressContent: field === "progressContent" ? text : undefined, timestamp: Date.now(), status: "streaming" as const, engine: "claude-code", model }] };
      });
    };

    const unlistenProgress = await listen<{ messageId: string; text: string }>("claude:progress", (e) => replaceOrUpdate(e.payload.messageId, "progressContent", e.payload.text));
    const unlistenChunk = await listen<{ messageId: string; text: string }>("claude:chunk", (e) => replaceOrUpdate(e.payload.messageId, "content", e.payload.text));

    const cleanup = () => { unlistenProgress(); unlistenChunk(); unlistenDone(); unlistenErr(); };

    const unlistenDone = await listen<{ messageId: string; conversationId: string }>("agent:completed", async (e) => {
      if (e.payload.conversationId !== selectedConversationId) return;
      cleanup();
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
      const input: SendWithClaudeInput = {
        projectKey: selectedProjectKey,
        conversationId: selectedConversationId,
        prompt, model, systemPrompt,
        activeSkills: get().activeSkills,
        crossSessionIds: get().crossSessionIds,
        personaFragment: get().personaFragment ?? undefined,
      };
      await invoke<{ messageId: string }>("start_claude_stream", { input });
      // Command returns immediately — events drive the rest
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

  sendWithCodex: async (prompt: string, model?: string) => {
    const { selectedProjectKey, selectedConversationId, runningThreadIds } = get();
    if (!selectedProjectKey || !selectedConversationId) return;
    if (runningThreadIds.includes(selectedConversationId)) {
      get()._enqueue(selectedConversationId, prompt.slice(0, 30), () => get().sendWithCodex(prompt, model));
      return;
    }
    get()._startRun(selectedConversationId);
    const now = Date.now();
    set((state) => ({
      messages: [
        ...state.messages,
        { id: `temp-user-${now}`, conversationId: selectedConversationId, role: "user", content: prompt, timestamp: now, status: "done" },
        { id: `temp-thinking-${now}`, conversationId: selectedConversationId, role: "assistant", content: "", progressContent: "Codex starting...", timestamp: now, status: "streaming", engine: "codex", model },
      ],
    }));

    const ulP = await listen<{ messageId: string; text: string }>("codex:progress", (e) => {
      set((state) => {
        const existing = state.messages.find((m) => m.id === e.payload.messageId);
        if (existing) {
          const prev = existing.progressContent || "";
          return { messages: state.messages.map((m) => m.id === e.payload.messageId ? { ...m, progressContent: prev ? `${prev}\n${e.payload.text}` : e.payload.text } : m) };
        }
        const withoutPlaceholder = state.messages.filter((m) => !m.id.startsWith("temp-thinking-"));
        return { messages: [...withoutPlaceholder, { id: e.payload.messageId, conversationId: selectedConversationId, role: "assistant" as const, content: "", progressContent: e.payload.text, timestamp: Date.now(), status: "streaming" as const, engine: "codex", model }] };
      });
    });
    const ulC = await listen<{ messageId: string; text: string }>("codex:chunk", (e) => {
      set((state) => {
        const existing = state.messages.find((m) => m.id === e.payload.messageId);
        if (existing) {
          return { messages: state.messages.map((m) => m.id === e.payload.messageId ? { ...m, content: e.payload.text } : m) };
        }
        const withoutPlaceholder = state.messages.filter((m) => !m.id.startsWith("temp-thinking-"));
        return { messages: [...withoutPlaceholder, { id: e.payload.messageId, conversationId: selectedConversationId, role: "assistant" as const, content: e.payload.text, timestamp: Date.now(), status: "streaming" as const, engine: "codex", model }] };
      });
    });
    const cleanup = () => { ulP(); ulC(); ulD(); ulE(); };
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
      await invoke<{ messageId: string }>("start_codex_run", { input: { projectKey: selectedProjectKey, conversationId: selectedConversationId, prompt, model, activeSkills: get().activeSkills, crossSessionIds: get().crossSessionIds, personaFragment: get().personaFragment ?? undefined } });
    } catch (e) {
      cleanup();
      set((state) => ({ error: String(e), messages: state.messages.filter((m) => !m.id.startsWith("temp-thinking-")) }));
      get()._endRun(selectedConversationId);
    }
  },

  sendWithGemini: async (prompt: string, model?: string) => {
    const { selectedProjectKey, selectedConversationId, runningThreadIds } = get();
    if (!selectedProjectKey || !selectedConversationId) return;
    if (runningThreadIds.includes(selectedConversationId)) {
      get()._enqueue(selectedConversationId, prompt.slice(0, 30), () => get().sendWithGemini(prompt, model));
      return;
    }
    get()._startRun(selectedConversationId);
    const now = Date.now();
    set((state) => ({
      messages: [
        ...state.messages,
        { id: `temp-user-${now}`, conversationId: selectedConversationId, role: "user", content: prompt, timestamp: now, status: "done" },
        { id: `temp-thinking-${now}`, conversationId: selectedConversationId, role: "assistant", content: "", progressContent: "Gemini initializing...", timestamp: now, status: "streaming", engine: "gemini", model },
      ],
    }));

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
        return { messages: [...withoutPlaceholder, { id: messageId, conversationId: selectedConversationId, role: "assistant" as const, content: field === "content" ? text : "", progressContent: field === "progressContent" ? text : undefined, timestamp: Date.now(), status: "streaming" as const, engine: "gemini", model }] };
      });
    };

    const ulP = await listen<{ messageId: string; text: string }>("gemini:progress", (e) => replaceOrUpdate(e.payload.messageId, "progressContent", e.payload.text));
    const ulC = await listen<{ messageId: string; text: string }>("gemini:chunk", (e) => replaceOrUpdate(e.payload.messageId, "content", e.payload.text));
    const cleanup = () => { ulP(); ulC(); ulD(); ulE(); };
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
      const input: SendWithClaudeInput = { projectKey: selectedProjectKey, conversationId: selectedConversationId, prompt, model, activeSkills: get().activeSkills, crossSessionIds: get().crossSessionIds, personaFragment: get().personaFragment ?? undefined };
      await invoke<{ messageId: string }>("start_gemini_stream", { input });
    } catch (e) {
      cleanup();
      set((state) => ({ error: String(e), messages: state.messages.filter((m) => !m.id.startsWith("temp-thinking-")).map((m) => m.status === "streaming" ? { ...m, status: "error", content: m.content || String(e) } : m) }));
      get()._endRun(selectedConversationId);
    }
  },

  sendWithOpencode: async (prompt: string, model?: string) => {
    const { selectedProjectKey, selectedConversationId, runningThreadIds } = get();
    if (!selectedProjectKey || !selectedConversationId) return;
    if (runningThreadIds.includes(selectedConversationId)) {
      get()._enqueue(selectedConversationId, prompt.slice(0, 30), () => get().sendWithOpencode(prompt, model));
      return;
    }
    get()._startRun(selectedConversationId);
    const now = Date.now();
    set((state) => ({
      messages: [
        ...state.messages,
        { id: `temp-user-${now}`, conversationId: selectedConversationId, role: "user", content: prompt, timestamp: now, status: "done" },
        { id: `temp-thinking-${now}`, conversationId: selectedConversationId, role: "assistant", content: "", progressContent: "OpenCode starting...", timestamp: now, status: "streaming", engine: "opencode", model },
      ],
    }));

    const ulP = await listen<{ messageId: string; text: string }>("opencode:progress", (e) => {
      set((state) => {
        const existing = state.messages.find((m) => m.id === e.payload.messageId);
        if (existing) {
          const prev = existing.progressContent || "";
          return { messages: state.messages.map((m) => m.id === e.payload.messageId ? { ...m, progressContent: prev ? `${prev}\n${e.payload.text}` : e.payload.text } : m) };
        }
        const withoutPlaceholder = state.messages.filter((m) => !m.id.startsWith("temp-thinking-"));
        return { messages: [...withoutPlaceholder, { id: e.payload.messageId, conversationId: selectedConversationId, role: "assistant" as const, content: "", progressContent: e.payload.text, timestamp: Date.now(), status: "streaming" as const, engine: "opencode", model }] };
      });
    });
    const cleanup = () => { ulP(); ulD(); ulE(); };
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
      await invoke<{ messageId: string }>("start_opencode_run", { input: { projectKey: selectedProjectKey, conversationId: selectedConversationId, prompt, model, activeSkills: get().activeSkills, crossSessionIds: get().crossSessionIds, personaFragment: get().personaFragment ?? undefined } });
    } catch (e) {
      cleanup();
      set((state) => ({ error: String(e), messages: state.messages.filter((m) => !m.id.startsWith("temp-thinking-")) }));
      get()._endRun(selectedConversationId);
    }
  },

  sendFollowup: async (engine: string, sourceType: string, sourceContent: string, goal?: string) => {
    const truncated = sourceContent.length > 800 ? sourceContent.slice(0, 800) + "..." : sourceContent;
    const goalLine = goal ? `\nGoal: ${goal}` : "";
    const prompt = `[Follow-up: ${sourceType}]${goalLine}\n\n${truncated}\n\n위 내용을 기반으로 작업해주세요.`;

    if (engine === "codex") {
      await get().sendWithCodex(prompt);
    } else if (engine === "gemini") {
      await get().sendWithGemini(prompt);
    } else if (engine === "opencode") {
      await get().sendWithOpencode(prompt);
    } else {
      await get().sendMessage(prompt);
    }
  },

  sendRoundtable: async (prompt: string, participants: RoundtableParticipant[], mode?: RtMode) => {
    const { selectedConversationId, runningThreadIds } = get();
    if (!selectedConversationId) return;
    if (runningThreadIds.includes(selectedConversationId)) {
      get()._enqueue(selectedConversationId, prompt.slice(0, 30), () => get().sendRoundtable(prompt, participants, mode));
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
      await invoke<{ messageId: string }>("start_roundtable_run", { input: { conversationId: selectedConversationId, prompt, participants, mode } });
    } catch (e) {
      cleanup(); set({ error: String(e) }); get()._endRun(selectedConversationId);
    }
  },

  sendRoundtableFollowup: async (prompt: string, participants: RoundtableParticipant[], mode?: RtMode) => {
    const { selectedConversationId, runningThreadIds } = get();
    if (!selectedConversationId) return;
    if (runningThreadIds.includes(selectedConversationId)) {
      get()._enqueue(selectedConversationId, prompt.slice(0, 30), () => get().sendRoundtableFollowup(prompt, participants, mode));
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

    let placeholderCleared2 = false;
    const ulRT = await listen<Message>("roundtable:progress", (event) => {
      const msg = event.payload;
      if (msg.role === "user") return;
      set((state) => {
        if (!placeholderCleared2) {
          placeholderCleared2 = true;
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
      await invoke<{ messageId: string }>("start_roundtable_followup", { input: { conversationId: selectedConversationId, prompt, participants, mode } });
    } catch (e) {
      cleanup(); set({ error: String(e) }); get()._endRun(selectedConversationId);
    }
  },

  cancelOperation: async (threadId?: string) => {
    const target = threadId ?? get().selectedConversationId;

    // Backend cancel flag — thread-aware
    if (target) {
      try {
        await invoke("cancel_running", { conversationId: target });
      } catch {
        // Best-effort
      }
    }

    if (!target) {
      // 대상 불명 — 전체 초기화 (fallback)
      set({ isRunning: false, runningThreadIds: [], error: null });
      return;
    }

    // 해당 thread만 running에서 제거
    set((state) => {
      const next = state.runningThreadIds.filter((id) => id !== target);
      return { isRunning: next.length > 0, runningThreadIds: next, error: null };
    });

    // 해당 thread의 queue도 비움 (cancel이면 대기 중인 것도 무의미)
    set((state) => ({
      messageQueue: state.messageQueue.filter((q) => q.threadId !== target),
    }));

    // 현재 보고 있는 conversation이면 메시지 새로고침
    if (target === get().selectedConversationId) {
      try {
        const messages = await invoke<Message[]>("list_messages", {
          conversationId: target,
        });
        set({ messages });
      } catch {
        // 무시
      }
    }
  },
});
