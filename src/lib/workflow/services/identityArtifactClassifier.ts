/**
 * Identity artifact classifier (projectIdentityAnalysisPlan subtask-01 Phase B).
 *
 * Review verdict + plan subtasks 를 입력으로 받아 subtask 별 identity-input
 * artifact 종류를 결정. emit 경로에서 순수 로직 분리 — classifier 는 DB 접근
 * 이나 invoke 없이 `{successes, failures}` 만 반환. 호출자가 각 subtask 에 대해
 * 적절한 invoke 를 수행.
 *
 * 판정 규칙 (subtask §3.4/§3.5 + Architect 보수적 정의):
 *   - `failed_subtask_ids` 포함 (idx+1 기준 1-based) → `finding_failure`
 *   - 그 외이면서 `status === "done"` → `finding_success`
 *   - 그 외 (in_progress / todo / abandoned) → 분류 skip
 */
import type { PlanSubtask } from "@/types";
import type { ParsedReviewVerdict } from "@/lib/planProposalParser";

export interface IdentityArtifactClassification {
  /** finding_success 로 emit 될 subtask 들. */
  successes: PlanSubtask[];
  /** finding_failure 로 emit 될 subtask 들. */
  failures: PlanSubtask[];
}

/**
 * verdict 의 failed_subtask_ids (1-based) 와 subtasks.idx (0-based) 를 맞춰
 * success / failure 버킷으로 나눈다. 중복 id 는 set 으로 normalize.
 *
 * verdict.verdict 가 "pass" 이면 `failed_subtask_ids` 는 비어있어야 하므로
 * 모든 done subtask 가 success 로 떨어짐. fail 이면 ids 에 있는 건 failure,
 * 나머지 done 은 success (부분 실패 허용).
 */
export function classifyIdentityArtifacts(
  subtasks: readonly PlanSubtask[],
  verdict: ParsedReviewVerdict,
): IdentityArtifactClassification {
  const failedSet = new Set<number>(verdict.failedSubtaskIds ?? []);
  const successes: PlanSubtask[] = [];
  const failures: PlanSubtask[] = [];
  for (const st of subtasks) {
    const oneBased = st.idx + 1;
    if (failedSet.has(oneBased)) {
      failures.push(st);
    } else if (st.status === "done") {
      successes.push(st);
    }
    // todo / in_progress / abandoned — 분류 대상 아님 (review 결과 직접 반영 없음)
  }
  return { successes, failures };
}

/**
 * `review_failed` plan-event 개수로 현재 rework round 를 계산한다. verdict
 * fail 이 막 처리되어 event 를 추가한 직후 호출한다고 가정 — "방금 포함된"
 * fail event 까지 카운트한다. computeDoomLoopState 와 달리 escalation window
 * 와 무관하게 누적 count (plan 생성 이후 총 fail round). Architect 주석에 따라
 * plans.rework_cycle 컬럼이 없어 plan_events derive 로 대체.
 */
export function computeReworkRound(
  planEvents: readonly { eventType: string }[],
): number {
  return planEvents.filter((e) => e.eventType === "review_failed").length;
}
