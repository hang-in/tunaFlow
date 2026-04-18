/**
 * Review workflow — RT review creation, verdict processing, marker scanning.
 * Phases: D→E (start review RT), E (process verdict).
 */
import { invoke } from "@tauri-apps/api/core";
import type { Message, Plan, RoundtableParticipant } from "@/types";
import * as planApi from "../api/plans";
import * as failureLessonsApi from "../api/failureLessons";
import * as insightApi from "../api/insight";
import {
  buildPlanContext,
  createAndLinkBranch,
  saveFailureLessons,
  createVerdictArtifact,
  createTestReportArtifact,
  getPlanSlug,
} from "./helpers";
import type { CreateBranchResult } from "./helpers";
import { syncResultReport, syncReviewReport } from "./reportSync";
import {
  extractImplPlan, hasImplComplete, hasReviewVerdict, extractReviewVerdict,
} from "../planProposalParser";
import type { ParsedImplPlan, ParsedReviewVerdict } from "../planProposalParser";

export type { CreateBranchResult };

// ─── Phase D→E: impl-complete → Start Review RT ───────────────────────────

export interface StartReviewRTResult extends CreateBranchResult {
  participants: RoundtableParticipant[];
  prompt: string;
  mode: "sequential";
}

export async function startReviewRT(
  plan: Plan,
  implMessages: Message[],
  testOutput?: string,
  reviewerEngines?: string[],
): Promise<StartReviewRTResult> {
  await planApi.updatePlanPhase(plan.id, "review");
  await planApi.createPlanEvent(plan.id, "impl_completed", "developer");

  syncResultReport(plan.id, implMessages, plan.developerEngine ?? undefined,
    plan.implementationBranchId ? `dev: ${plan.title}` : undefined);

  if (testOutput) {
    createTestReportArtifact(plan, testOutput);
  }

  const engines = reviewerEngines ?? ["claude", "gemini"];

  const { branch, shadowConvId } = await createAndLinkBranch(
    plan, "review", `Review RT: ${plan.title}`, "roundtable",
  );

  const planContext = await buildPlanContext(plan);
  const implSummary = implMessages
    .filter((m) => m.role === "assistant")
    .map((m) => m.content.slice(0, 2000))
    .join("\n---\n");

  const prompt = [
    `당신은 코드 리뷰어입니다. **코드를 읽어서** 검증하세요. 빌드/테스트 명령을 직접 실행하지 마세요.`,
    "",
    `## Plan (원래 요구사항)`,
    planContext,
    "",
    `## Implementation (Developer 구현 결과)`,
    implSummary.slice(0, 6000),
    "",
    testOutput ? `## 테스트 결과\n${testOutput.slice(0, 3000)}\n` : "",
    `## 리뷰 절차`,
    ``,
    `각 subtask의 task 파일(컨텍스트에 포함됨, 또는 \`docs/plans/${getPlanSlug(plan)}-task-*.md\`에서 Read 도구로 열 수 있음)을 참고하여 아래 3가지를 확인하세요:`,
    ``,
    `1. **Changed files 확인**: task 파일에 명시된 파일이 실제로 수정/생성되었는가? 변경 내용이 Change description과 일치하는가?`,
    `2. **Verification 결과 확인**: Developer가 보고한 검증 결과를 확인하세요. 모든 Verification 명령이 통과했는가?`,
    `3. **결함 검사**: 변경된 코드에 런타임 에러, 논리 버그, 보안 취약점이 있는가? (코드를 읽어서 판단)`,
    ``,
    `### Pass 조건`,
    `위 3가지가 모두 충족되면 **pass**입니다.`,
    ``,
    `### Fail 사유가 되지 않는 것`,
    `- 코드 스타일/구조가 task 파일과 다르지만 결과가 올바른 경우`,
    `- task 파일에 명시되지 않은 테스트 커버리지 부족`,
    `- Changed files 밖 파일의 기존 품질 문제`,
    `- "더 나은 방법이 있다"는 의견 → recommendations에 작성`,
    ``,
    `### 판정 형식`,
    `- verdict: pass / fail / conditional`,
    `- 다음 5개 차원을 각각 1~5점으로 평가하여 명시:`,
    `  - **plan_coverage** — 모든 subtask가 task 파일 내용대로 구현되었는가`,
    `  - **code_quality** — 런타임/논리/보안 결함 정도 (결함 없을수록 5)`,
    `  - **test_coverage** — 테스트 결과 + Verification 통과 범위 (task 파일에 명시된 부분 기준)`,
    `  - **doc_quality** — 주석·README·CLAUDE.md 등 문서 업데이트 적절성`,
    `  - **convention** — 프로젝트 기존 코딩 컨벤션과의 일관성`,
    `  점수 기준 총점: 22+ → pass / 10 미만 → fail / 그 외 → conditional (단, findings가 있으면 verdict 우선)`,
    `- fail 시 반드시 **파일:줄번호 + 구체적 결함 설명**을 findings에 포함`,
    `- fail 시 \`failed_subtask_ids: [N, M]\` 형식으로 해당 subtask 번호 명시`,
    `- 개선 제안은 findings가 아닌 **recommendations**에 분리`,
    ``,
    `### 출력 예시`,
    "```",
    `<!-- tunaflow:review-verdict -->`,
    `verdict: conditional`,
    `plan_coverage: 4`,
    `code_quality: 3`,
    `test_coverage: 2`,
    `doc_quality: 4`,
    `convention: 5`,
    `findings:`,
    `- [code_quality] src/api/users.ts:42 사용자 입력을 파라미터화 없이 SQL 에 삽입`,
    `- [test_coverage] 400/404 에러 케이스 테스트 누락`,
    `recommendations:`,
    `- prepared statement 사용으로 전환`,
    `- status-code별 응답 테스트 추가`,
    `<!-- /tunaflow:review-verdict -->`,
    "```",
  ].filter(Boolean).join("\n");

  const participants: RoundtableParticipant[] = engines.map((eng, i) => ({
    name: `Reviewer-${String.fromCharCode(65 + i)}`,
    engine: eng,
    role: "reviewer" as const,
  }));

  const mode = "sequential" as const;
  const rtConfig = JSON.stringify({ participants, mode });
  // Tauri camelCase: Rust 의 `config_json: String` 은 FE 에서 `configJson` 으로 호출.
  // 이전엔 `config` 로 잘못 보내 invoke 가 실패하며 save_rt_config 가 no-op 되고
  // Review RT 진입 자체가 throw 되던 버그. s37 재현 로그로 특정.
  await invoke("save_rt_config", { conversationId: shadowConvId, configJson: rtConfig });

  // NOTE: RT execution is the caller's responsibility.
  // After this returns, the caller should call openThread(branch.id) then
  // sendThreadRoundtable(prompt, participants, mode) to actually run the review.
  // We intentionally do NOT create a user_message here — sendThreadRoundtable
  // persists the prompt as the user message itself, and pre-seeding it would
  // cause a duplicate prompt in the branch.
  return { branch, shadowConvId, participants, prompt, mode };
}

// ─── Phase E: Process review verdict ──────────────────────────────────────

export async function processReviewVerdict(
  plan: Plan,
  verdict: ParsedReviewVerdict,
): Promise<void> {
  const verdictEventType = verdict.verdict === "pass" ? "review_passed"
    : verdict.verdict === "fail" ? "review_failed" : "review_conditional";
  const events = await planApi.listPlanEvents(plan.id);
  const lastReviewStart = [...events].reverse().find((e) => e.eventType === "review_started");
  if (lastReviewStart) {
    const alreadyProcessed = events.some(
      (e) => e.eventType === verdictEventType && e.createdAt > lastReviewStart.createdAt,
    );
    if (alreadyProcessed) {
      console.debug("[verdict] already processed for this review round, skipping");
      return;
    }
  }

  const detail = JSON.stringify({
    verdict: verdict.verdict,
    findings: verdict.findings,
    recommendations: verdict.recommendations,
  });

  if (verdict.verdict === "pass") {
    await planApi.updatePlanPhase(plan.id, "done");
    await planApi.updatePlanStatus(plan.id, "done");
    await planApi.createPlanEvent(plan.id, "review_passed", "reviewer", detail);
    // Notify Meta — plan cycle finished, user may want next-priority suggestion.
    window.dispatchEvent(new CustomEvent("tunaflow:meta-task"));
    try {
      await failureLessonsApi.resolveFailureLessonsByPlan(
        plan.id,
        `Review passed — ${verdict.recommendations?.[0] ?? "resolved"}`,
      );
    } catch (e) { console.warn("[failure-learning] resolve failed:", e); }
    insightApi.resolveInsightFindingsByPlan(plan.id)
      .catch((e) => console.warn("[insight] resolve findings failed:", e));
    await createVerdictArtifact(plan, verdict);
    if (plan.implementationBranchId) {
      await invoke("archive_branch", { id: plan.implementationBranchId }).catch((e) => console.debug("[archive]", e));
    }
    if (plan.reviewBranchId) {
      await invoke("archive_branch", { id: plan.reviewBranchId }).catch((e) => console.debug("[archive]", e));
    }
    // Notify: auto-send plan completion summary to Architect
    window.dispatchEvent(new CustomEvent("tunaflow:plan-completed", {
      detail: { planId: plan.id, title: plan.title, conversationId: plan.conversationId },
    }));
  } else if (verdict.verdict === "fail") {
    await planApi.updatePlanPhase(plan.id, "rework");
    await planApi.createPlanEvent(plan.id, "review_failed", "reviewer", detail);
    await saveFailureLessons(plan, verdict.findings);
    await createVerdictArtifact(plan, verdict);

    // Doom loop detection: count review_failed events SINCE last escalation
    const freshEvents = await planApi.listPlanEvents(plan.id);
    let lastEscalationIdx = -1;
    for (let i = freshEvents.length - 1; i >= 0; i--) {
      if (freshEvents[i].eventType === "doom_loop_escalated" || freshEvents[i].eventType === "architect_redesign_requested") { lastEscalationIdx = i; break; }
    }
    const eventsSinceReset = lastEscalationIdx >= 0 ? freshEvents.slice(lastEscalationIdx + 1) : freshEvents;
    const failEvents = eventsSinceReset.filter((e) => e.eventType === "review_failed");
    const failCount = failEvents.length;

    if (failCount >= 2) {
      try {
        const prevFailEvent = failEvents[failEvents.length - 2];
        const prevDetail = JSON.parse(prevFailEvent?.detail ?? "{}");
        const prevFiles = new Set((prevDetail.findings as string[] ?? [])
          .map((f: string) => f.match(/([a-zA-Z0-9_./-]+\.[a-zA-Z]+)/)?.[1])
          .filter(Boolean));
        const currFiles = new Set(verdict.findings
          .map((f: string) => f.match(/([a-zA-Z0-9_./-]+\.[a-zA-Z]+)/)?.[1])
          .filter(Boolean));
        const overlap = [...currFiles].filter((f) => prevFiles.has(f)).length;
        const overlapRatio = currFiles.size > 0 ? overlap / currFiles.size : 0;
        if (overlapRatio > 0.4) {
          await planApi.createPlanEvent(
            plan.id, "design_review_suggested", "system",
            `동일 파일에서 연속 실패 (겹침 ${Math.round(overlapRatio * 100)}%) — 설계 재검토 권장`,
          );
        }
        const prevFindingTexts = (prevDetail.findings as string[] ?? []).map((f: string) => f.slice(0, 60).toLowerCase());
        const currFindingTexts = verdict.findings.map((f: string) => f.slice(0, 60).toLowerCase());
        const textOverlap = currFindingTexts.filter((cf) => prevFindingTexts.some((pf) => cf.includes(pf.slice(0, 30)) || pf.includes(cf.slice(0, 30)))).length;
        const textOverlapRatio = currFindingTexts.length > 0 ? textOverlap / currFindingTexts.length : 0;
        if (textOverlapRatio > 0.5 && failCount >= 2) {
          await planApi.createPlanEvent(
            plan.id, "design_review_suggested", "system",
            `동일 유형 findings 반복 (${Math.round(textOverlapRatio * 100)}% 일치) — 구현이 아닌 설계 문제 가능성`,
          );
        }
      } catch { /* ignore parse errors */ }
    }

    if (failCount >= 5) {
      await planApi.updatePlanPhase(plan.id, "subtask_review");
      await planApi.createPlanEvent(
        plan.id,
        "doom_loop_escalated",
        "system",
        `Review 실패 ${failCount}회 — Architect 재설계로 강제 에스컬레이션`,
      );
      // Baton has moved to Architect — review branch is no longer active.
      const { archiveReviewBranchForHandoff } = await import("./implementWorkflow");
      await archiveReviewBranchForHandoff(plan);
      // Meta notification — escalation requires user attention.
      window.dispatchEvent(new CustomEvent("tunaflow:meta-task"));
    } else if (failCount >= 3) {
      await planApi.createPlanEvent(
        plan.id,
        "doom_loop_warning",
        "system",
        `Review 실패 ${failCount}회 — 설계 재검토를 권장합니다. Architect 재설계 또는 Developer 계속 rework 중 선택하세요.`,
      );
      // Meta notification — user should review whether to keep iterating or redesign.
      window.dispatchEvent(new CustomEvent("tunaflow:meta-task"));
    }
  } else {
    await planApi.createPlanEvent(plan.id, "review_conditional", "reviewer", detail);
  }

  syncReviewReport(plan.id, verdict);
}

// ─── Message scanning ──────────────────────────────────────────────────────

/**
 * Scan branch messages for workflow markers and return detected signals.
 */
export function scanMessagesForMarkers(messages: Message[]): {
  implPlan: ParsedImplPlan | null;
  implComplete: boolean;
  reviewVerdict: ParsedReviewVerdict | null;
} {
  let implPlan: ParsedImplPlan | null = null;
  let implComplete = false;
  let reviewVerdict: ParsedReviewVerdict | null = null;

  for (const msg of messages) {
    if (msg.role !== "assistant") continue;
    if (!implPlan) {
      const plan = extractImplPlan(msg.content);
      if (plan) implPlan = plan;
    }
    if (!implComplete && hasImplComplete(msg.content)) {
      implComplete = true;
    }
    // Use LAST verdict — followup reviews supersede earlier ones
    if (hasReviewVerdict(msg.content)) {
      const v = extractReviewVerdict(msg.content);
      if (v) reviewVerdict = v;
    }
  }

  return { implPlan, implComplete, reviewVerdict };
}
