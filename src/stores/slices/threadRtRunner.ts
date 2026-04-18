/**
 * threadRtRunner.ts — Roundtable execution logic for branch threads.
 *
 * Extracted from threadSlice.ts to keep RT event handling separate from
 * thread navigation and message-sending state management.
 */

import { invoke } from "@tauri-apps/api/core";
import { errorMessage } from "@/lib/utils";
import { createRtChunkBatcher } from "./streamingUtils";
import { autoDetectReviewVerdict } from "@/lib/workflow/branchSync";
import type { SetState, GetState, Message, RoundtableParticipant, RtMode } from "./types";
import type { RtParticipantStatus } from "./threadSlice";

/**
 * Shared RT execution helper for sendThreadRoundtable and sendThreadRoundtableFollowup.
 * Manages event listeners, streaming, cleanup, and run lifecycle.
 */
export async function runThreadRoundtable(
  set: SetState, get: GetState, command: string,
  prompt: string, participants: RoundtableParticipant[], mode?: RtMode,
): Promise<void> {
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
  // Guard: only update UI if this branch is still the active thread
  const isActiveThread = () => get().threadBranchConvId === threadBranchConvId;

  const ulPS = await listen<{ conversationId: string; name: string; engine: string; model?: string; round: number; status: string }>(
    "roundtable:participant_status", (e) => {
      if (e.payload.conversationId !== threadBranchConvId) return;
      if (!isActiveThread()) return;
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
    if (!isActiveThread()) return;
    set((state) => {
      const idx = state.threadMessages.findIndex((m) => m.id === msg.id);
      if (idx >= 0) {
        const msgs = [...state.threadMessages];
        msgs[idx] = msg;
        return { threadMessages: msgs };
      }
      if (!placeholderCleared) {
        placeholderCleared = true;
        return { threadMessages: [...state.threadMessages.filter((m) => !m.id.startsWith("temp-thinking-")), msg] };
      }
      return { threadMessages: [...state.threadMessages, msg] };
    });
  });

  // Throttled roundtable:chunk listener for real-time streaming
  const rtBatcher = createRtChunkBatcher(
    threadBranchConvId,
    isActiveThread,
    (batch) => set((state) => ({
      threadMessages: state.threadMessages.map((m) => {
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
    ulPS(); ulRT(); ulChunk(); ulD(); ulE();
  };
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
    // Notify RT completion
    import("@/stores/notificationStore").then(({ notify }) => {
      const state = get();
      const branch = state.branches.find((b) => threadBranchConvId != null && `branch:${b.id}` === threadBranchConvId);
      notify("completed", "Roundtable", "토론 완료", threadBranchConvId, {
        conversationTitle: branch?.customLabel ?? branch?.label,
      });
    }).catch((e) => console.debug("[notify:rt-completed]", e));
    setTimeout(() => set({ rtParticipantStatuses: new Map(), rtStatusConversationId: null }), 2000);
    get()._endRun(threadBranchConvId, { silent: true });
  });
  const ulE = await listen<{ conversationId: string; error: string }>("agent:error", async (e) => {
    if (e.payload.conversationId !== threadBranchConvId) return;
    cleanup();
    import("@/stores/notificationStore").then(({ notify }) => {
      const state = get();
      const branch = state.branches.find((b) => threadBranchConvId != null && `branch:${b.id}` === threadBranchConvId);
      notify("error", "Roundtable", `오류: ${e.payload.error.slice(0, 80)}`, threadBranchConvId, {
        conversationTitle: branch?.customLabel ?? branch?.label,
      });
    }).catch((e) => console.debug("[notify:rt-error]", e));
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
    cleanup();
    set({ error: errorMessage(e), rtParticipantStatuses: new Map(), rtStatusConversationId: null });
    get()._endRun(threadBranchConvId);
  }
}
