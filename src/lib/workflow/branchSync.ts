/**
 * Branch sync helpers — called after agent:completed on implementation/review branches.
 * Extracted from threadSlice to keep store slice lean.
 */
import type { Message } from "@/types";

// ─── Auto-sync implementation completion from completed branch ─────────────
// Called after agent:completed on implementation branches.
// Syncs subtask-done markers to DB AND detects impl-complete structurally
// (all subtasks done) even when the agent doesn't emit the marker.

export async function autoSyncImplCompletion(shadowConvId: string, messages: Message[]): Promise<void> {
  if (!shadowConvId.startsWith("branch:")) return;
  const branchId = shadowConvId.slice("branch:".length);

  try {
    const { findPlanByBranch, listSubtasks, updateSubtaskStatus } = await import("@/lib/api/plans");
    const plan = await findPlanByBranch(branchId);
    if (!plan || plan.implementationBranchId !== branchId) return;
    if (plan.phase !== "implementation" && plan.phase !== "rework") return;

    const { scanCompletedSubtasks, hasImplComplete } = await import("@/lib/planProposalParser");
    const subtasks = await listSubtasks(plan.id);
    if (subtasks.length === 0) return;

    // 1. Sync marker-detected subtask completions to DB
    const markerNums = scanCompletedSubtasks(messages);
    const hasMarker = messages.some((m) => m.role === "assistant" && hasImplComplete(m.content));

    for (const num of markerNums) {
      const st = subtasks.find((s) => s.idx === num - 1); // markers are 1-based, idx is 0-based
      if (st && st.status !== "done") {
        await updateSubtaskStatus(st.id, "done").catch((e) => console.debug("[subtask-sync]", e));
      }
    }

    // 2. If impl-complete marker exists, mark all subtasks done
    if (hasMarker) {
      for (const st of subtasks) {
        if (st.status !== "done") {
          await updateSubtaskStatus(st.id, "done").catch((e) => console.debug("[subtask-sync]", e));
        }
      }
      return; // marker present, no need for structural detection
    }

    // 3. Structural detection: check if agent's final message indicates completion
    //    Look for completion signals in the last assistant message
    const lastAssistant = [...messages].reverse().find((m) => m.role === "assistant");
    if (!lastAssistant) return;

    // Check if all subtasks are now done (after marker sync above)
    const refreshed = await listSubtasks(plan.id);
    const allDone = refreshed.every((st) => st.status === "done");

    if (!allDone) {
      // Heuristic: if the message mentions all tasks being complete, mark remaining subtasks done
      const content = lastAssistant.content.toLowerCase();
      const completionSignals = [
        "모든 task", "모든 태스크", "전체 완료", "구현이 완료", "구현 완료",
        "all tasks", "all subtasks", "implementation complete", "completed all",
      ];
      const looksComplete = completionSignals.some((s) => content.includes(s));
      if (looksComplete) {
        for (const st of refreshed) {
          if (st.status !== "done") {
            await updateSubtaskStatus(st.id, "done").catch((e) => console.debug("[subtask-sync]", e));
          }
        }
      }
    }
  } catch (e) {
    console.warn("[impl-sync]", e);
  }
}

// ─── Auto-detect review verdict from completed branch ──────────────────────
// Called after agent:completed in both single-agent and RT review flows.
// Extracts branchId from shadow conversation ID, finds linked plan,
// and auto-processes verdict if found.

export async function autoDetectReviewVerdict(shadowConvId: string, messages: Message[]): Promise<void> {
  if (!shadowConvId.startsWith("branch:")) return;
  const branchId = shadowConvId.slice("branch:".length);

  try {
    const { findPlanByBranch } = await import("@/lib/api/plans");
    const plan = await findPlanByBranch(branchId);
    if (!plan || plan.reviewBranchId !== branchId) return;
    if (plan.phase !== "review") return;

    const { scanAllReviewerVerdicts } = await import("@/lib/planProposalParser");
    const { scanMessagesForMarkers, processReviewVerdict } = await import("@/lib/workflowOrchestration");
    const { aggregateReviewVerdicts } = await import("@/lib/aggregateReviewVerdicts");
    const planApi = await import("@/lib/api/plans");

    // 리뷰 브랜치 재사용(A안) 부작용 방지: 마지막 review_started 이벤트 timestamp
    // 이후의 verdict 만 집계. 이전 라운드 fail verdict 가 현재 라운드 판정을
    // 오염시키던 버그를 차단. (s37 재현: 단일 reviewer 가 같은 브랜치에서 3차
    // rework 끝에 pass 해도 1차/2차 fail 때문에 "3명 중 fail" 로 Rework 결정됨)
    const events = await planApi.listPlanEvents(plan.id).catch(() => []);
    const lastReviewStart = [...events].reverse().find((e) => e.eventType === "review_started");
    // plan_events.created_at 은 초 단위, messages.timestamp 는 ms. 자동 정규화.
    const sinceTs = lastReviewStart
      ? (lastReviewStart.createdAt < 10_000_000_000 ? lastReviewStart.createdAt * 1000 : lastReviewStart.createdAt)
      : undefined;

    // Multi-reviewer path (RT): collect every reviewer verdict, aggregate, and
    // feed the consensus to processReviewVerdict. Single-reviewer path falls
    // back to the legacy last-verdict-wins behavior.
    const all = scanAllReviewerVerdicts(messages, sinceTs);
    let effectiveVerdict;
    if (all.length >= 2) {
      const agg = aggregateReviewVerdicts(all);
      if (!agg) return;
      // Flatten aggregate into ParsedReviewVerdict shape for processReviewVerdict.
      effectiveVerdict = {
        verdict: agg.verdict,
        rubric: agg.rubric
          ? {
              planCoverage: agg.rubric.planCoverage.mean,
              codeQuality: agg.rubric.codeQuality.mean,
              testCoverage: agg.rubric.testCoverage.mean,
              docQuality: agg.rubric.docQuality.mean,
              convention: agg.rubric.convention.mean,
            }
          : undefined,
        findings: agg.findings,
        recommendations: agg.recommendations,
        failedSubtaskIds: agg.failedSubtaskIds,
        raw: `aggregated from ${agg.reviewerCount} reviewers (pass=${agg.votes.pass}, fail=${agg.votes.fail}, conditional=${agg.votes.conditional})`,
      };
    } else {
      const markers = scanMessagesForMarkers(messages);
      if (!markers.reviewVerdict) return;
      effectiveVerdict = markers.reviewVerdict;
    }

    const { toast } = await import("sonner");
    const verdict = effectiveVerdict.verdict;
    await processReviewVerdict(plan, effectiveVerdict);

    if (verdict === "pass") {
      toast.success(
        all.length >= 2
          ? `Review 통과 — 만장일치 (${all.length} reviewers)`
          : "Review 통과 — Plan 완료 처리됨",
      );
    } else if (verdict === "fail") {
      toast.warning(
        all.length >= 2
          ? `Review 실패 — ${all.length}명 중 fail 투표 있음 (Rework)`
          : "Review 실패 — Rework 단계로 전환됨",
      );
    } else {
      toast.info("Review 조건부 통과 — 사용자 판단 필요");
    }
  } catch (e) {
    console.warn("[verdict-autodetect]", e);
  }
}
