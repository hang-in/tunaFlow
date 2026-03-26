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

    // Queue if this thread is already running
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

    // Subscribe to progress events (thinking/tool steps — shown as plain text during streaming)
    const unlistenProgress = await listen<{ messageId: string; text: string }>(
      "claude:progress",
      (event) => {
        const { messageId, text } = event.payload;
        set((state) => {
          const existing = state.messages.find((m) => m.id === messageId);
          if (existing) {
            // Append to progressContent
            const prev = existing.progressContent || "";
            const updated = prev ? `${prev}\n${text}` : text;
            return {
              messages: state.messages.map((m) =>
                m.id === messageId ? { ...m, progressContent: updated } : m
              ),
            };
          }
          // First progress event: replace thinking placeholder
          const withoutPlaceholder = state.messages.filter(
            (m) => !m.id.startsWith("temp-thinking-"),
          );
          return {
            messages: [...withoutPlaceholder, {
              id: messageId,
              conversationId: selectedConversationId,
              role: "assistant" as const,
              content: "",
              progressContent: text,
              timestamp: Date.now(),
              status: "streaming" as const,
              engine: "claude-code",
              model,
            }],
          };
        });
      },
    );

    // Subscribe to streaming chunks (final answer text)
    const unlisten = await listen<{ messageId: string; text: string }>(
      "claude:chunk",
      (event) => {
        const { messageId, text } = event.payload;
        set((state) => {
          const existing = state.messages.find((m) => m.id === messageId);
          if (existing) {
            return {
              messages: state.messages.map((m) =>
                m.id === messageId ? { ...m, content: text } : m
              ),
            };
          }
          // First chunk: replace thinking placeholder with real streaming message
          const withoutPlaceholder = state.messages.filter(
            (m) => !m.id.startsWith("temp-thinking-"),
          );
          const streamingMsg: Message = {
            id: messageId,
            conversationId: selectedConversationId,
            role: "assistant",
            content: text,
            timestamp: Date.now(),
            status: "streaming",
            engine: "claude-code",
            model,
          };
          return { messages: [...withoutPlaceholder, streamingMsg] };
        });
      }
    );

    try {
      const input: SendWithClaudeInput = {
        projectKey: selectedProjectKey,
        conversationId: selectedConversationId,
        prompt,
        model,
        systemPrompt,
        activeSkills: get().activeSkills,
        crossSessionIds: get().crossSessionIds,
      };
      const assistantMsg = await invoke<Message>("stream_with_claude", { input });
      // Load final messages from DB to replace temp + streaming states
      const messages = await invoke<Message[]>("list_messages", {
        conversationId: selectedConversationId,
      });
      set({ messages });
      get()._endRun(selectedConversationId);
    } catch (e) {
      set((state) => ({
        error: String(e),
        messages: state.messages
          .filter((m) => !m.id.startsWith("temp-thinking-"))
          .map((m) => m.status === "streaming" ? { ...m, status: "error", content: m.content || String(e) } : m),
      }));
      get()._endRun(selectedConversationId);
    } finally {
      unlisten();
      unlistenProgress();
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
        { id: `temp-thinking-${now}`, conversationId: selectedConversationId, role: "assistant", content: "", progressContent: "", timestamp: now, status: "streaming", engine: "codex", model },
      ],
    }));
    try {
      const input: SendWithClaudeInput = { projectKey: selectedProjectKey, conversationId: selectedConversationId, prompt, model };
      await invoke<Message>("send_with_codex", { input });
      const messages = await invoke<Message[]>("list_messages", { conversationId: selectedConversationId });
      set({ messages });
      get()._endRun(selectedConversationId);
    } catch (e) {
      set((state) => ({
        error: String(e),
        messages: state.messages.filter((m) => !m.id.startsWith("temp-thinking-")),
      }));
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
        { id: `temp-thinking-${now}`, conversationId: selectedConversationId, role: "assistant", content: "", progressContent: "", timestamp: now, status: "streaming", engine: "gemini", model },
      ],
    }));
    try {
      const input: SendWithClaudeInput = { projectKey: selectedProjectKey, conversationId: selectedConversationId, prompt, model };
      await invoke<Message>("send_with_gemini", { input });
      const messages = await invoke<Message[]>("list_messages", { conversationId: selectedConversationId });
      set({ messages });
      get()._endRun(selectedConversationId);
    } catch (e) {
      set((state) => ({
        error: String(e),
        messages: state.messages.filter((m) => !m.id.startsWith("temp-thinking-")),
      }));
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
        { id: `temp-thinking-${now}`, conversationId: selectedConversationId, role: "assistant", content: "", progressContent: "", timestamp: now, status: "streaming", engine: "opencode", model },
      ],
    }));
    try {
      const input: SendWithClaudeInput = { projectKey: selectedProjectKey, conversationId: selectedConversationId, prompt, model };
      await invoke<Message>("send_with_opencode", { input });
      const messages = await invoke<Message[]>("list_messages", { conversationId: selectedConversationId });
      set({ messages });
      get()._endRun(selectedConversationId);
    } catch (e) {
      set((state) => ({
        error: String(e),
        messages: state.messages.filter((m) => !m.id.startsWith("temp-thinking-")),
      }));
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
    const unlisten = await listen<Message>("roundtable:progress", (event) => {
      const msg = event.payload;
      if (msg.role === "user") return;
      set((state) => {
        if (!placeholderCleared) {
          placeholderCleared = true;
          const filtered = state.messages.filter((m) => !m.id.startsWith("temp-thinking-"));
          return { messages: [...filtered, msg] };
        }
        return { messages: [...state.messages, msg] };
      });
    });

    try {
      const input: RoundtableRunInput = {
        conversationId: selectedConversationId,
        prompt,
        participants,
        mode,
      };
      await invoke<Message[]>("roundtable_run", { input });
      const messages = await invoke<Message[]>("list_messages", {
        conversationId: selectedConversationId,
      });
      set({ messages });
      get()._endRun(selectedConversationId);
    } catch (e) {
      set({ error: String(e) });
      get()._endRun(selectedConversationId);
    } finally {
      unlisten();
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
    const unlisten = await listen<Message>("roundtable:progress", (event) => {
      const msg = event.payload;
      if (msg.role === "user") return;
      set((state) => {
        if (!placeholderCleared2) {
          placeholderCleared2 = true;
          const filtered = state.messages.filter((m) => !m.id.startsWith("temp-thinking-"));
          return { messages: [...filtered, msg] };
        }
        return { messages: [...state.messages, msg] };
      });
    });

    try {
      const input: RoundtableRunInput = {
        conversationId: selectedConversationId,
        prompt,
        participants,
        mode,
      };
      await invoke<Message[]>("roundtable_followup", { input });
      const messages = await invoke<Message[]>("list_messages", {
        conversationId: selectedConversationId,
      });
      set({ messages });
      get()._endRun(selectedConversationId);
    } catch (e) {
      set({ error: String(e) });
      get()._endRun(selectedConversationId);
    } finally {
      unlisten();
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
