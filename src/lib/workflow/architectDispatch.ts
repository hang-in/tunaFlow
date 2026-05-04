/**
 * Architect dispatch helpers — review verdict (pass/fail/escalate) 처리 시
 * Architect 에게 main conv 로 직접 prompt 를 dispatch 하는 단일 진입점.
 *
 * **배경**: 기존엔 review-cycle 알림 (review_passed / doom_loop_escalated 등) 이
 * Meta inbox 로만 가고, 사용자가 *"메타에게 물어보기"* 또는 *"플랜 재설계"* 버튼을
 * 눌러야 Architect 가 실제 작업했다. 본 helper 가 도입되면서 dispatch 책임이
 * Meta inbox → Architect main conv 로 이동.
 *
 * **호출처**:
 *  - `processReviewVerdict()` 의 pass 분기 → `dispatchArchitectNextPriority(plan)`
 *  - `processReviewVerdict()` 의 doom-escalate 분기 → `dispatchArchitectRedesign(plan, verdict, { reason: "doom-escalate" })`
 *  - `ReviewVerdictCard.handleRedesign()` 사용자 클릭 → `dispatchArchitectRedesign(plan, verdict, { reason: "user-redesign" })`
 *
 * **유의**:
 *  - phase=done / status=abandoned 갱신 + subtask reset 로직은 caller 책임
 *    (helper 는 *dispatch 만* 담당). 같은 helper 가 자동/수동 양쪽에서 호출되므로
 *    plan 상태 머신 변경은 caller 쪽에 둔다.
 *  - i18n key 변경 시 ko/en 동시 갱신.
 */
import type { Plan } from "@/types";
import type { ParsedReviewVerdict } from "../planProposalParser";
import { useChatStore } from "@/stores/chatStore";
import i18n from "i18next";

export type ArchitectRedesignReason = "user-redesign" | "doom-escalate";

/** pass 직후 main conv 로 다음 우선순위 제안 prompt 를 dispatch.
 *  실패해도 throw 하지 않는다 (review pass 처리 자체를 막지 않기 위함). */
export async function dispatchArchitectNextPriority(plan: Plan): Promise<void> {
  try {
    const { sendWithEngine, getConversationEngine } = useChatStore.getState();
    const saved = getConversationEngine(plan.conversationId);
    const engine = saved?.engine ?? "claude";
    const prompt = i18n.t("review.verdict.next_priority_prompt", {
      ns: "workflow",
      title: plan.title,
    });
    await sendWithEngine(engine, prompt);
  } catch (e) {
    console.warn("[architectDispatch] next-priority failed:", e);
  }
}

/** fail/escalate 시 main conv 로 plan 재설계 prompt 를 dispatch.
 *  - reason="user-redesign": ReviewVerdictCard 의 사용자 클릭 경로
 *  - reason="doom-escalate": review 5회 누적 실패 시 자동 호출 경로
 *  실패해도 throw 하지 않는다. */
export async function dispatchArchitectRedesign(
  plan: Plan,
  verdict: ParsedReviewVerdict,
  opts: { reason: ArchitectRedesignReason; failCount?: number },
): Promise<void> {
  try {
    const { sendWithEngine, getConversationEngine } = useChatStore.getState();
    const saved = getConversationEngine(plan.conversationId);
    const engine = saved?.engine ?? "claude";
    const findings = verdict.findings.length > 0
      ? verdict.findings.map((f) => `- ${f}`).join("\n")
      : i18n.t("review.verdict.findings_empty_redesign", { ns: "workflow" });
    const recs = verdict.recommendations.length > 0
      ? verdict.recommendations.map((r) => `- ${r}`).join("\n")
      : "";
    const recsBlock = recs
      ? i18n.t("review.verdict.redesign_recs_block", { ns: "workflow", recs })
      : "";
    const reasonNote = opts.reason === "doom-escalate" && opts.failCount
      ? i18n.t("review.verdict.redesign_reason_doom_escalate", {
          ns: "workflow",
          failCount: opts.failCount,
        })
      : "";
    const prompt = i18n.t("review.verdict.redesign_prompt", {
      ns: "workflow",
      title: plan.title,
      revision: plan.revision,
      verdict: verdict.verdict.toUpperCase(),
      findings,
      recsBlock,
      nextRevision: (plan.revision ?? 1) + 1,
    });
    const finalPrompt = reasonNote ? `${reasonNote}\n\n${prompt}` : prompt;
    await sendWithEngine(engine, finalPrompt);
  } catch (e) {
    console.warn("[architectDispatch] redesign failed:", e);
  }
}
