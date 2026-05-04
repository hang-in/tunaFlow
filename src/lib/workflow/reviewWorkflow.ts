/**
 * Review workflow — RT review creation, verdict processing, marker scanning.
 * Phases: D→E (start review RT), E (process verdict).
 */
import { invoke } from "@tauri-apps/api/core";
import type { Message, Plan, RoundtableParticipant } from "@/types";
import * as planApi from "../api/plans";
import { getSetting } from "../appStore";
import {
  extractManualItems,
  type ManualVerificationItem,
  type ManualVerificationResult,
} from "../manualVerification";
import * as failureLessonsApi from "../api/failureLessons";
import * as insightApi from "../api/insight";
import { dispatchMetaNotification } from "../metaNotifications";
import { maybeTriggerMetaAnalysis } from "../metaAnalysisTrigger";
import { dispatchArchitectNextPriority, dispatchArchitectRedesign } from "./architectDispatch";
import {
  buildPlanContext,
  getOrCreateReviewBranch,
  saveFailureLessons,
  createVerdictArtifact,
  createReviewOutcomeArtifact,
  createReworkReasonArtifact,
  createFindingSuccessArtifact,
  createFindingFailureArtifact,
  createTestReportArtifact,
  getPlanSlug,
} from "./helpers";
import { classifyIdentityArtifacts, computeReworkRound } from "./services/identityArtifactClassifier";
import type { CreateBranchResult } from "./helpers";
import { syncResultReport, syncReviewReport } from "./reportSync";
import {
  extractImplPlan, hasImplComplete, hasReviewVerdict, extractReviewVerdict,
} from "../planProposalParser";
import type { ParsedImplPlan, ParsedReviewVerdict } from "../planProposalParser";

export type { CreateBranchResult };

/**
 * Manual verification gate error (B-19 / Issue #176).
 *
 * Gate 결과에 fail 이 있을 때 startReviewRT 가 던진다. 호출부는 instanceof 로
 * 구분해 DevProgressView 의 기존 rework 상태 전환 UI 를 트리거하고 toast 는
 * 띄우지 않는다 (normal "review failed" 경로와 UX 맞춤).
 */
export class ManualVerificationFailed extends Error {
  constructor(public readonly failedItems: Array<{ label: string; reason?: string }>) {
    super(`Manual verification failed: ${failedItems.length} item(s)`);
    this.name = "ManualVerificationFailed";
  }
}

/**
 * Review RT entry failure (Plan reviewRTEntryFailureRollbackPlan_2026-04-25).
 *
 * startReviewRT 의 phase 전환 이후 어느 stage 에서든 throw 되면 그 직후
 * Layer A 가 phase 를 implementation 으로 rollback 하고 review_entry_failed
 * plan_event 를 기록한 뒤 본 에러로 wrap 해서 다시 throw 한다.
 *
 * 호출자(DevProgressView)는 instanceof 로 구분해 toast 대신 재시도 UI(Layer B)를
 * 노출. ManualVerificationFailed / cancel 경로와는 분리된다.
 */
export class ReviewRTEntryFailed extends Error {
  constructor(
    public readonly stage: string,
    public readonly cause: unknown,
  ) {
    const reason = cause instanceof Error ? cause.message : String(cause);
    super(`startReviewRT failed at stage "${stage}": ${reason}`);
    this.name = "ReviewRTEntryFailed";
  }
}

/**
 * Layer A 의 stage 식별자. plan_event detail 과 console.debug 에 동일 값 사용.
 * 매트릭스: docs/reference/reviewRTEntryFailureAudit_2026-04-25.md.
 */
export type StartReviewRTStage =
  | "update_phase_review"
  | "event_impl_completed"
  | "test_artifact"
  | "get_or_create_review_branch"
  | "build_plan_context"
  | "save_rt_config";

// ─── Phase D→E: impl-complete → Start Review RT ───────────────────────────

export interface StartReviewRTResult extends CreateBranchResult {
  participants: RoundtableParticipant[];
  prompt: string;
  mode: "sequential";
}

/** Reviewer 선택 정보 — engine 뿐 아니라 model 까지 명시해야 Codex app-server
 *  fallback(gpt-5-codex) 같은 엉뚱한 기본값이 끼어들지 않는다.
 *  `reviewerEngines?: string[]` 경로는 deprecated — 최종적으로 `reviewers` 로 통일.
 */
export interface ReviewerChoice {
  engine: string;
  model?: string;
  name?: string;
}

export async function startReviewRT(
  plan: Plan,
  implMessages: Message[],
  testOutput?: string,
  reviewers?: ReviewerChoice[] | string[],
  /**
   * Manual verification gate callback (B-19). 호출부가 UI dialog 를 띄우고
   * results (items 와 동일 순서/길이) 를 resolve, 사용자 취소 시 null 을 resolve.
   * 제공 안 하면 게이트 skip (기존 플로우 유지 — 테스트/CLI 호환).
   */
  runManualGate?: (items: ManualVerificationItem[]) => Promise<ManualVerificationResult[] | null>,
): Promise<StartReviewRTResult> {
  // ─── Manual Verification Gate (B-19) ───
  // Phase 전환 전에 수행 — fail 시 review phase 로 잘못 진입 방지 (INV-1).
  const skipGate = await getSetting<boolean>("skipManualVerificationGate", false).catch(() => false);
  if (!skipGate && runManualGate) {
    const manualItems = extractManualItems(implMessages);
    if (manualItems.length === 0) {
      // INV-3: 0 items 면 다이얼로그 안 띄움. 이 경우에만 skipped 기록 (plan 주의).
      await planApi.createPlanEvent(plan.id, "manual_verification_skipped", "system",
        JSON.stringify({ reason: "no manual items found in impl response" })).catch(() => {});
    } else {
      const results = await runManualGate(manualItems);
      if (results === null) {
        // 사용자가 dialog 취소 → phase 유지. INV-5.
        throw new Error("Manual verification cancelled by user");
      }
      const hasFail = results.some((r) => r.status === "fail");
      const eventType = hasFail ? "manual_verification_failed" : "manual_verification_passed";
      await planApi.createPlanEvent(plan.id, eventType, "user", JSON.stringify({
        items: manualItems.map((it, i) => ({
          label: it.label,
          status: results[i].status,
          reason: results[i].reason,
        })),
      })).catch((e) => console.warn("[manual-gate] plan_event failed:", e));

      if (hasFail) {
        // Rework 경로 진입. INV-1: phase 를 review 로 넘기지 않는다.
        const failItems = manualItems
          .map((it, i) => ({ label: it.label, reason: results[i].reason }))
          .filter((_, i) => results[i].status === "fail");
        await planApi.updatePlanPhase(plan.id, "rework");
        // INV-6: rework_reason identity-input artifact 에 manual 실패 항목 포함.
        // 기존 createReworkReasonArtifact 는 ParsedReviewVerdict 시그니처라 inline
        // invoke 로 manual 전용 artifact 작성.
        invoke("create_identity_artifact", {
          input: {
            kind: "rework_reason",
            conversationId: plan.conversationId,
            planId: plan.id,
            subtaskId: null,
            title: `Manual verification failed: ${plan.title}`,
            content: {
              source: "manual_verification",
              failedItems: failItems.map((f) => ({
                label: f.label,
                reason: f.reason || "manual verification failed",
              })),
            },
          },
        }).catch((e) => console.warn("[manual-gate] artifact failed:", e));
        throw new ManualVerificationFailed(failItems);
      }
    }
  }

  // ─── Layer A: stage 추적 + 실패 시 phase rollback (Plan ...RollbackPlan_2026-04-25) ───
  // 각 critical await 직전에 currentStage 를 갱신하고, 외곽 try/catch 가 실패한
  // stage 를 잡아 phase=implementation 로 rollback + review_entry_failed plan_event 기록.
  // INV-1/INV-2/INV-3 (audit doc 참고) 준수.
  let currentStage: StartReviewRTStage = "update_phase_review";
  // Layer C: 단계 식별 로그 — 디버깅 시 마지막 도달 stage 추적용.
  console.debug("[startReviewRT.stage]", { stage: currentStage, planId: plan.id });
  try {
    await planApi.updatePlanPhase(plan.id, "review");

    currentStage = "event_impl_completed";
    console.debug("[startReviewRT.stage]", { stage: currentStage, planId: plan.id });
    await planApi.createPlanEvent(plan.id, "impl_completed", "developer");

    syncResultReport(plan.id, implMessages, plan.developerEngine ?? undefined,
      plan.implementationBranchId ? `dev: ${plan.title}` : undefined);

    if (testOutput) {
      currentStage = "test_artifact";
      console.debug("[startReviewRT.stage]", { stage: currentStage, planId: plan.id });
      createTestReportArtifact(plan, testOutput);
    }

    // 하위호환: string[] 로 들어오면 model 없음으로 normalize.
    // default 는 claude/gemini — 이 둘은 무료/구독 양쪽에서 model fallback 문제가 없음.
    const normalized: ReviewerChoice[] = (() => {
      if (!reviewers || reviewers.length === 0) {
        return [{ engine: "claude" }, { engine: "gemini" }];
      }
      return (reviewers as (ReviewerChoice | string)[]).map((r) =>
        typeof r === "string" ? { engine: r } : r,
      );
    })();

    currentStage = "get_or_create_review_branch";
    console.debug("[startReviewRT.stage]", { stage: currentStage, planId: plan.id });
    // A안: 같은 roundtable 모드면 기존 리뷰 브랜치 재사용. 모드 달라지거나 없으면 신규.
    const { branch, shadowConvId, reused } = await getOrCreateReviewBranch(
      plan, `Review RT: ${plan.title}`, "roundtable",
    );
    if (reused) {
      console.debug("[startReviewRT] reusing existing review branch:", branch.id);
    }

    currentStage = "build_plan_context";
    console.debug("[startReviewRT.stage]", { stage: currentStage, planId: plan.id });
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

    const participants: RoundtableParticipant[] = normalized.map((r, i) => ({
      name: r.name ?? `Reviewer-${String.fromCharCode(65 + i)}`,
      engine: r.engine,
      model: r.model,
      role: "reviewer" as const,
    }));

    const mode = "sequential" as const;
    const rtConfig = JSON.stringify({ participants, mode });

    currentStage = "save_rt_config";
    console.debug("[startReviewRT.stage]", { stage: currentStage, planId: plan.id });
    // Tauri camelCase: Rust 의 `config_json: String` 은 FE 에서 `configJson` 으로 호출.
    // 이전엔 `config` 로 잘못 보내 invoke 가 실패하며 save_rt_config 가 no-op 되고
    // Review RT 진입 자체가 throw 되던 버그. s37 재현 로그로 특정.
    await invoke("save_rt_config", { conversationId: shadowConvId, configJson: rtConfig });

    // 리뷰 브랜치 shadow conv 에도 engine/model 을 기록한다. 사용자가 RT 진행 중
    // 리뷰 브랜치에 "직접 메시지" 를 보낼 때(handoff, follow-up 등), `resolveModel()`
    // 이 이 엔트리를 읽어 올바른 model 을 전달할 수 있게 한다. 이전엔 RT 참여자
    // 기반으로만 실행되고 shadow conv 엔트리가 없어서, 일반 채팅 경로로 빠질 때
    // engine=codex/model=undefined 가 되어 app-server fallback(gpt-5 등) 이 발동하며
    // ChatGPT 계정에서 400 에러가 나던 문제를 수정.
    const first = participants[0];
    if (first?.engine) {
      try {
        const { useChatStore } = await import("@/stores/chatStore");
        useChatStore.getState().saveConversationEngine(shadowConvId, {
          profileId: null,
          engine: first.engine,
          model: first.model,
          // profileId is null (RT participant pick), so saveProfiles sync
          // never touches this entry regardless of source — tag for clarity.
          source: "user-explicit",
        });
      } catch (e) { console.warn("[startReviewRT] saveConversationEngine failed:", e); }
    }

    // NOTE: RT execution is the caller's responsibility.
    // After this returns, the caller should call openThread(branch.id) then
    // sendThreadRoundtable(prompt, participants, mode) to actually run the review.
    // We intentionally do NOT create a user_message here — sendThreadRoundtable
    // persists the prompt as the user message itself, and pre-seeding it would
    // cause a duplicate prompt in the branch.
    console.debug("[startReviewRT.stage]", {
      stage: "complete",
      planId: plan.id,
      branchId: branch.id,
      reused,
    });
    return { branch, shadowConvId, participants, prompt, mode };
  } catch (err) {
    // Layer A: 어느 stage 에서든 throw → phase rollback + review_entry_failed 기록.
    // INV-2: phase 를 implementation 으로 복귀 (PlanPhase enum 에 "ready" 가 없어
    // review 직전 자연스러운 상태인 implementation 사용). plan 본문의 "ready"
    // 표현은 informal naming.
    // INV-1: review_entry_failed event 가 plan 에 남아 있어 UI 가 "phase=review 인데
    // RT 없음" 상태와 "정상 review 진행" 을 구분 가능.
    // INV-3: ReviewRTEntryFailed 로 wrap 해서 호출자가 instanceof 분기로 재시도 UI 노출.
    const reason = err instanceof Error ? err.message : String(err);
    console.warn("[startReviewRT] failed at stage", currentStage, ":", reason);
    await planApi.updatePlanPhase(plan.id, "implementation").catch((rollbackErr) => {
      console.warn("[startReviewRT] phase rollback failed:", rollbackErr);
    });
    await planApi.createPlanEvent(
      plan.id,
      "review_entry_failed",
      "system",
      JSON.stringify({ stage: currentStage, reason }),
    ).catch((evErr) => {
      console.warn("[startReviewRT] review_entry_failed event creation failed:", evErr);
    });
    throw new ReviewRTEntryFailed(currentStage, err);
  }
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

  // Pre-emit identity-input artifact regardless of verdict — identity 분석 input 은
  // pass/fail/conditional 모든 경로에서 "Plan 가치 · 품질 곡선" 을 복원하는 원천.
  // createVerdictArtifact (markdown) 와 별도. subtask-01 INV-1 "이벤트 시점" 기반.
  const reviewRound = (plan.versionMinor || 0) + 1;
  await createReviewOutcomeArtifact(plan, verdict, undefined, reviewRound);

  // Phase B: subtask 별 finding_success / finding_failure. pass/fail 구분 없이
  // classifier 가 판정 (failed_subtask_ids 있으면 failure, 없고 done 이면 success).
  // pass 시 failed_subtask_ids 는 빈 배열 → 모든 done 이 success 로 떨어짐.
  try {
    const subtasks = await planApi.listSubtasks(plan.id);
    const { successes, failures } = classifyIdentityArtifacts(subtasks, verdict);
    const agentEngine = plan.developerEngine ?? undefined;
    await Promise.all([
      ...successes.map((st) => createFindingSuccessArtifact(plan, st, agentEngine)),
      ...failures.map((st) => createFindingFailureArtifact(plan, st, verdict, agentEngine, null)),
    ]);
  } catch (e) {
    console.warn("[identity-artifact] subtask classification failed:", e);
  }

  if (verdict.verdict === "pass") {
    await planApi.updatePlanPhase(plan.id, "done");
    await planApi.updatePlanStatus(plan.id, "done");
    await planApi.createPlanEvent(plan.id, "review_passed", "reviewer", detail);
    // projectKey 는 plan 의 메인 conv 경유해서 찾기 — Tier 2 분석 트리거용.
    let projectKey: string | undefined;
    try {
      const conv = await invoke<{ projectKey?: string }>("get_conversation", { id: plan.conversationId });
      projectKey = conv?.projectKey;
    } catch { /* ignore */ }
    // Tier 2 분석 (Haiku/Flash brief) 은 보존 — 결과 dispatch kind 는 PR-3 에서 tier2_brief 로 분리.
    if (projectKey) {
      maybeTriggerMetaAnalysis(projectKey, "review_passed", { planTitle: plan.title })
        .catch((e) => console.debug("[meta-trigger]", e));
    }
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
    // archive_branch 가 DB 에 반영된 뒤 Store 의 branches state 도 갱신. 이게 없으면
    // 사이드바 메인 트리에 archived branch 가 active 로 남아있는 stale 상태가 됨.
    try {
      const { useChatStore } = await import("@/stores/chatStore");
      await useChatStore.getState().loadBranches(plan.conversationId);
    } catch (e) { console.debug("[loadBranches after archive]", e); }
    // Plan 완료 → Architect 가 main conv 에서 다음 우선순위 제안 prompt 를 자동 수신.
    // 기존엔 Meta inbox 의 review_passed 알림 + 사용자 클릭 (askMeta) 흐름이었지만,
    // *plan-cycle 결정* 은 Meta 의 read-only 역할보다 Architect 의 design 역할에 속함.
    // 사용자 의사결정 burden 제거. Tier 2 brief (Haiku/Flash) 는 별 axis 로 inbox 유지.
    await dispatchArchitectNextPriority(plan);
  } else if (verdict.verdict === "fail") {
    await planApi.updatePlanPhase(plan.id, "rework");
    await planApi.createPlanEvent(plan.id, "review_failed", "reviewer", detail);
    await saveFailureLessons(plan, verdict.findings);
    await createVerdictArtifact(plan, verdict);

    // Phase B: Rework 진입 시 rework_reason identity-input artifact.
    // cycle 은 review_failed plan-event 개수 (방금 추가한 건 포함). plans.rework_cycle
    // 컬럼이 없어 derive. computeDoomLoopState 와 달리 escalation window 무관한
    // 누적 count — identity 분석은 plan 전체 생애주기를 보기 위함.
    try {
      const eventsAfterFail = await planApi.listPlanEvents(plan.id);
      const cycle = computeReworkRound(eventsAfterFail);
      await createReworkReasonArtifact(plan, verdict, cycle);
    } catch (e) {
      console.warn("[identity-artifact] rework_reason failed:", e);
    }

    // Tier 2 트리거 — projectKey 조회 후 fail 누적 카운트 체크.
    try {
      const conv = await invoke<{ projectKey?: string }>("get_conversation", { id: plan.conversationId });
      if (conv?.projectKey) {
        maybeTriggerMetaAnalysis(conv.projectKey, "review_failed", {
          planTitle: plan.title,
          findings: verdict.findings,
        }).catch((e) => console.debug("[meta-trigger]", e));
      }
    } catch { /* ignore */ }

    // Doom-loop window state (failCount scoped to "since last escalation")
    // + optional overlap analysis against the previous fail event's
    // findings — both behind `services/doomLoopDetector`.
    const freshEvents = await planApi.listPlanEvents(plan.id);
    const { computeDoomLoopState, computeFindingOverlap } = await import("./services/doomLoopDetector");
    const doom = computeDoomLoopState(freshEvents);
    const failCount = doom.failCount;
    const failEvents = doom.windowEvents.filter((e) => e.eventType === "review_failed");

    if (failCount >= 2) {
      try {
        const prevFailEvent = failEvents[failEvents.length - 2];
        const prevDetail = JSON.parse(prevFailEvent?.detail ?? "{}");
        const prevFindings = (prevDetail.findings as string[]) ?? [];
        const overlap = computeFindingOverlap(prevFindings, verdict.findings);
        if (overlap.fileOverlapRatio > 0.4) {
          await planApi.createPlanEvent(
            plan.id, "design_review_suggested", "system",
            `동일 파일에서 연속 실패 (겹침 ${Math.round(overlap.fileOverlapRatio * 100)}%) — 설계 재검토 권장`,
          );
        }
        if (overlap.textOverlapRatio > 0.5) {
          await planApi.createPlanEvent(
            plan.id, "design_review_suggested", "system",
            `동일 유형 findings 반복 (${Math.round(overlap.textOverlapRatio * 100)}% 일치) — 구현이 아닌 설계 문제 가능성`,
          );
        }
      } catch { /* ignore parse errors */ }
    }

    if (doom.recommendation === "escalate") {
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
      // projectKey 조회 (meta conv mirror 용).
      const escProjectKey = await invoke<{ projectKey?: string }>("get_conversation", { id: plan.conversationId })
        .then((c) => c?.projectKey).catch(() => undefined);
      dispatchMetaNotification({
        kind: "doom_loop_escalated",
        title: `⚠️ Plan "${plan.title}" 재설계 필요`,
        summary: `Review ${failCount}회 실패로 Architect 재설계가 강제되었습니다. Subtask 범위를 재검토하세요.`,
        projectKey: escProjectKey,
        route: { tab: "workflow", stage: "plan-check", planId: plan.id },
      });
    } else if (doom.recommendation === "warn") {
      await planApi.createPlanEvent(
        plan.id,
        "doom_loop_warning",
        "system",
        `Review 실패 ${failCount}회 — 설계 재검토를 권장합니다. Architect 재설계 또는 Developer 계속 rework 중 선택하세요.`,
      );
      const warnProjectKey = await invoke<{ projectKey?: string }>("get_conversation", { id: plan.conversationId })
        .then((c) => c?.projectKey).catch(() => undefined);
      dispatchMetaNotification({
        kind: "doom_loop_warning",
        title: `⚠️ Plan "${plan.title}" ${failCount}회 실패`,
        summary: "설계 재검토 권장. 계속 rework 할지 Architect 재설계로 갈지 선택하세요.",
        projectKey: warnProjectKey,
        route: { tab: "workflow", stage: "review", planId: plan.id },
      });
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
