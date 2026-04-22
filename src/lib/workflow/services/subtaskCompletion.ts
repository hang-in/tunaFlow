/**
 * Subtask completion service.
 *
 * Two concerns that used to be re-implemented separately in
 * `branchSync.ts:autoSyncImplCompletion` and `useSubtaskProgress.ts`:
 *
 *   1. Turn a mix of marker scans + DB status into a single "which
 *      subtasks are done?" snapshot (`detectCompletedSubtasks`, pure).
 *   2. Persist marker-discovered completions back to the DB
 *      (`syncSubtaskCompletion`, side-effecting).
 *
 * Markers use 1-based numbering (`subtask-done:1`), DB rows use 0-based
 * `idx`. This module normalises everything to 1-based so callers don't
 * have to keep mapping.
 */
import type { Message, PlanSubtask } from "@/types";
import { scanCompletedSubtasks, hasImplComplete } from "@/lib/planProposalParser";

export interface SubtaskCompletionState {
  /** 1-based subtask numbers marked done via `<!-- tunaflow:subtask-done:N -->`. */
  markerNums: Set<number>;
  /** Any assistant message carries `<!-- tunaflow:impl-complete -->`. */
  hasImplCompleteMarker: boolean;
  /** 1-based subtask numbers whose DB row status == 'done'. */
  dbDoneNums: Set<number>;
  /** Markers ∪ DB; plus every subtask when `allComplete`. */
  completedNums: Set<number>;
  /**
   * `true` when the impl-complete marker fired OR the DB already shows
   * every subtask as done. Callers treat this as "ready for review".
   */
  allComplete: boolean;
}

/**
 * Merge marker scan + DB status into a single completion snapshot.
 * Pure — no DB writes and no async work. Callers get a stable value to
 * drive UI (`useSubtaskProgress`) or follow-up side effects
 * (`branchSync.autoSyncImplCompletion`).
 */
export function detectCompletedSubtasks(
  messages: Message[],
  subtasks: readonly PlanSubtask[],
): SubtaskCompletionState {
  // Streaming-중인 assistant 메시지의 중간 content 에 마커가 섞여 있어도 감지하지 않는다.
  // `m.status === "done"` 이 된 뒤에야 완료 신호로 취급 (워크플로 버튼 조기 활성화 방지).
  const doneAssistant = messages.filter(
    (m) => m.role === "assistant" && m.status === "done",
  );
  const markerNums = scanCompletedSubtasks(doneAssistant);
  const hasImplCompleteMarker = doneAssistant.some((m) => hasImplComplete(m.content));
  const dbDoneNums = new Set(
    subtasks.filter((s) => s.status === "done").map((s) => s.idx + 1),
  );
  const completedNums = new Set<number>([...markerNums, ...dbDoneNums]);
  const allDoneInDb = subtasks.length > 0 && subtasks.every((s) => s.status === "done");
  const allComplete = hasImplCompleteMarker || allDoneInDb;
  if (allComplete) {
    for (const s of subtasks) completedNums.add(s.idx + 1);
  }
  return { markerNums, hasImplCompleteMarker, dbDoneNums, completedNums, allComplete };
}

/**
 * Side-effecting: push marker-discovered completions (and everything on
 * an impl-complete signal) to the DB via `updateSubtaskStatus`. Returns
 * the 1-based numbers that were actually written — callers use this to
 * decide whether any state changed.
 *
 * Errors on individual subtasks are swallowed with a debug log so a
 * single transient write failure doesn't abort the batch.
 */
export async function syncSubtaskCompletion(
  _planId: string,
  subtasks: readonly PlanSubtask[],
  messages: Message[],
): Promise<{ updated: number[] }> {
  const state = detectCompletedSubtasks(messages, subtasks);
  const numsToSync: Set<number> = state.hasImplCompleteMarker
    ? new Set(subtasks.map((s) => s.idx + 1))
    : state.markerNums;
  if (numsToSync.size === 0) return { updated: [] };

  const { updateSubtaskStatus } = await import("@/lib/api/plans");
  const updated: number[] = [];
  for (const num of numsToSync) {
    const st = subtasks.find((s) => s.idx === num - 1);
    if (!st || st.status === "done") continue;
    try {
      await updateSubtaskStatus(st.id, "done");
      updated.push(num);
    } catch (e) {
      console.debug("[subtask-sync]", e);
    }
  }
  return { updated };
}
