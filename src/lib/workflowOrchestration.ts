/**
 * Workflow orchestration utilities.
 *
 * Connects plan phase transitions to branch creation, agent invocation,
 * and marker-based auto-transitions.
 */

import { invoke } from "@tauri-apps/api/core";
import type { Branch, Plan, Message, RoundtableParticipant } from "@/types";
import * as planApi from "./api/plans";

/** Generate ASCII-only slug from plan title for file paths.
 *  Korean/CJK titles produce very short slugs (e.g. "분석 UX 개선" → "ux"),
 *  so we append a hash suffix to prevent collisions between different plans.
 */
export function slugifyPlanTitle(title: string): string {
  const base = title
    .replace(/[^a-zA-Z0-9-]/g, "-")
    .replace(/-+/g, "-")
    .replace(/^-|-$/g, "")
    .toLowerCase()
    .slice(0, 60);
  // If slug is very short (< 4 chars, common with Korean-only titles),
  // append a 4-char hash from the full title to prevent collisions
  if (base.length < 4) {
    let hash = 0;
    for (let i = 0; i < title.length; i++) {
      hash = ((hash << 5) - hash + title.charCodeAt(i)) | 0;
    }
    const suffix = Math.abs(hash).toString(36).slice(0, 4);
    return (base ? `${base}-${suffix}` : `plan-${suffix}`) || "plan";
  }
  return base || "plan";
}
import { extractImplPlan, hasImplComplete, hasReviewVerdict, extractReviewVerdict } from "./planProposalParser";
import type { ParsedImplPlan, ParsedReviewVerdict } from "./planProposalParser";

// ─── Plan document helper ───────────────────────────────────────────────────

/** Resolve current project path. Returns null if unavailable. */
async function getProjectPath(): Promise<string | null> {
  try {
    const { useChatStore } = await import("@/stores/chatStore");
    const projectKey = useChatStore.getState().selectedProjectKey;
    if (!projectKey) return null;
    const project = await invoke("get_project", { key: projectKey }) as { path?: string };
    return project?.path ?? null;
  } catch { return null; }
}

/** Generate/update plan document in project directory. Fire-and-forget. */
export async function syncPlanDocument(planId: string): Promise<void> {
  try {
    const pp = await getProjectPath();
    if (!pp) return;
    await planApi.generatePlanDocument(planId, pp);
  } catch (e) { console.warn("[tunaflow]", e); }
}

/** Generate review report document. Fire-and-forget. */
export async function syncReviewReport(
  planId: string,
  verdict: ParsedReviewVerdict,
  reviewerEngines: string[] = [],
  testOutput?: string,
): Promise<void> {
  try {
    const pp = await getProjectPath();
    if (!pp) return;
    await planApi.generateReviewReport(
      planId, pp, verdict.verdict,
      verdict.findings, verdict.recommendations,
      reviewerEngines, testOutput,
    );
  } catch (e) { console.warn("[tunaflow]", e); }
}

/** Generate implementation result report. Fire-and-forget. */
export async function syncResultReport(
  planId: string,
  implMessages: Message[],
  developerEngine?: string,
  branchLabel?: string,
): Promise<void> {
  try {
    const pp = await getProjectPath();
    if (!pp) return;

    // Strip tunaflow workflow markers from content before including in documents
    const stripMarkers = (text: string) =>
      text.replace(/<!--\s*tunaflow:[a-z_-]+(?::\d+)?\s*-->/g, "")
          .replace(/<!--\s*subtask-done:\d+\s*-->/g, "")
          .replace(/<!--\s*impl-complete\s*-->/g, "")
          .replace(/\n{3,}/g, "\n\n")
          .trim();

    // Compress impl messages into summary + per-subtask results
    // After rework, only use messages since the last rework prompt (avoid stale content)
    let lastReworkIdx = -1;
    for (let i = implMessages.length - 1; i >= 0; i--) {
      if (implMessages[i].role === "user" && implMessages[i].content.includes("### 🔄 Rework")) {
        lastReworkIdx = i;
        break;
      }
    }
    const relevantMessages = lastReworkIdx >= 0
      ? implMessages.slice(lastReworkIdx + 1)
      : implMessages;
    const assistantMsgs = relevantMessages.filter((m) => m.role === "assistant");
    const summary = assistantMsgs.length > 0
      ? stripMarkers(assistantMsgs[assistantMsgs.length - 1].content.slice(0, 2000))
      : "(No implementation output)";

    // Extract subtask-done markers to build per-subtask results
    const { scanCompletedSubtasks } = await import("./planProposalParser");
    const completedNums = scanCompletedSubtasks(implMessages); // scan ALL messages for done markers
    const subtaskResults = assistantMsgs
      .slice(-10)
      .map((m) => stripMarkers(m.content.slice(0, 500)))
      .filter((c) => c.trim().length > 0);

    const knownIssues: string[] = [];

    await planApi.generateResultReport(
      planId, pp, summary, subtaskResults, knownIssues,
      developerEngine, branchLabel,
    );
  } catch (e) { console.warn("[tunaflow]", e); }
}

// ─── Branch helpers ─────────────────────────────────────────────────────────

interface CreateBranchResult {
  branch: Branch;
  shadowConvId: string;
}

async function createAndLinkBranch(
  plan: Plan,
  branchType: "implementation" | "review",
  label: string,
  mode: "chat" | "roundtable" = "chat",
): Promise<CreateBranchResult> {
  // Add round number for review branches (2nd review → "Review RT: ... (2차)")
  let finalLabel = label;
  if (branchType === "review") {
    const events = await planApi.listPlanEvents(plan.id);
    const reviewCount = events.filter(
      (e) => e.eventType === "review_requested" || e.eventType === "impl_completed"
    ).length;
    if (reviewCount > 1) {
      finalLabel = `${label} (${reviewCount}차)`;
    }
  }

  const input = {
    conversationId: plan.conversationId,
    label: finalLabel,
    mode,
    parentBranchId: plan.branchId ?? undefined,
  };
  const branch = await invoke<Branch>("create_branch", { input });
  const shadowConvId = await invoke<string>("open_branch_stream", { branchId: branch.id });

  // Link branch to plan
  await planApi.linkPlanBranch(plan.id, branchType, branch.id);

  return { branch, shadowConvId };
}

// ─── Plan content builder ───────────────────────────────────────────────────

async function buildPlanContext(plan: Plan): Promise<string> {
  const subtasks = await planApi.listSubtasks(plan.id);
  const subtaskList = subtasks
    .map((st, i) => `${i + 1}. ${st.title}${st.details ? ` — ${st.details}` : ""}`)
    .join("\n");

  return [
    `## Plan: ${plan.title}`,
    plan.description ? `\n### Description\n${plan.description}` : "",
    plan.expectedOutcome ? `\n### Expected Outcome\n${plan.expectedOutcome}` : "",
    `\n### Subtasks\n${subtaskList || "(none)"}`,
  ].filter(Boolean).join("\n");
}

// ─── Phase C: Review Branch ─────────────────────────────────────────────────

export async function startReviewBranch(
  plan: Plan,
  feedback: string,
): Promise<CreateBranchResult> {
  const { branch, shadowConvId } = await createAndLinkBranch(
    plan, "review", `Review: ${plan.title}`, "chat",
  );
  await planApi.createPlanEvent(plan.id, "review_requested", "user", feedback);

  // Build review prompt with plan context + user feedback
  const planContext = await buildPlanContext(plan);
  const prompt = [
    `이 Plan에 대한 검토가 요청되었습니다.`,
    "",
    planContext,
    "",
    `### 사용자 의견`,
    feedback,
    "",
    `Plan을 분석하고, 수정이 필요하면 \`<!-- tunaflow:plan-proposal -->\` 형식으로 수정된 Plan을 제안하세요.`,
  ].join("\n");

  // Create user message in the branch shadow conversation
  await invoke("create_user_message", { input: { conversationId: shadowConvId, content: prompt } });

  return { branch, shadowConvId };
}

// ─── Phase C→D: Approve → Implementation Branch ─────────────────────────────

export async function approveAndStartImplementation(
  plan: Plan,
  developerEngine: string = "claude",
): Promise<CreateBranchResult & { prompt: string }> {
  // Phase transition
  await planApi.updatePlanPhase(plan.id, "implementation");
  await planApi.updatePlanStatus(plan.id, "active");
  await planApi.createPlanEvent(plan.id, "approved", "user");
  await planApi.assignPlanEngines(plan.id, { developer: developerEngine });

  // Create implementation branch
  const { branch, shadowConvId } = await createAndLinkBranch(
    plan, "implementation", `Impl: ${plan.title}`, "chat",
  );

  // Build developer prompt — lightweight, agent reads files directly
  // Only include pending subtasks (skip already completed ones)
  const slug = slugifyPlanTitle(plan.title);
  const subtasks = await planApi.listSubtasks(plan.id);
  const pendingSubtasks = subtasks.filter((s) => s.status !== "done");
  const targetSubtasks = pendingSubtasks.length > 0 ? pendingSubtasks : subtasks;

  const taskItems = targetSubtasks.map((s) =>
    `- \`docs/plans/${slug}-task-${String(s.idx).padStart(2, "0")}.md\` — ${s.title}`
  );
  const doneCount = subtasks.length - targetSubtasks.length;
  const doneNote = doneCount > 0
    ? `\n> ${doneCount}개 태스크는 이미 완료됨 — 해당 코드를 변경하지 마세요.`
    : "";
  const prompt = [
    `### 🔧 구현 시작`,
    ``,
    `**Plan**: "${plan.title}"`,
    ``,
    `**작업 지시서**:`,
    ...taskItems,
    ``,
    `각 task 파일을 읽고 순서대로 구현하세요.${doneNote}`,
    ``,
    `**필수 절차**:`,
    `1. task 파일을 읽고 **Changed files** 섹션의 파일만 수정하세요.`,
    `2. 구현 후 task 파일의 **Verification** 섹션의 명령을 **모두 실행**하고 결과를 보고하세요.`,
    `3. 모든 검증이 통과하면 \`<!-- tunaflow:subtask-done:N -->\` 마커를 포함하세요.`,
    `4. 전체 완료 시 \`<!-- tunaflow:impl-complete -->\` 마커를 포함하세요.`,
  ].join("\n");

  return { branch, shadowConvId, prompt };
}

// ─── Phase D: Approve impl-plan → Start implementation ──────────────────────

/** Returns the prompt string — caller sends via sendThreadMessage */
export async function approveImplPlan(
  plan: Plan,
): Promise<string> {
  await planApi.createPlanEvent(plan.id, "impl_approved", "user");
  return "실행 계획이 승인되었습니다. 구현을 시작하세요.";
}

// ─── Phase D→E: impl-complete → Start Review RT ─────────────────────────────

export async function startReviewRT(
  plan: Plan,
  implMessages: Message[],
  testOutput?: string,
  reviewerEngines?: string[],
): Promise<CreateBranchResult> {
  // Phase transition
  await planApi.updatePlanPhase(plan.id, "review");
  await planApi.createPlanEvent(plan.id, "impl_completed", "developer");

  // Generate implementation result report before review
  syncResultReport(plan.id, implMessages, plan.developerEngine ?? undefined,
    plan.implementationBranchId ? `Impl: ${plan.title}` : undefined);

  const engines = reviewerEngines ?? ["claude", "gemini"];

  // Create review branch with RT mode
  const { branch, shadowConvId } = await createAndLinkBranch(
    plan, "review", `Review RT: ${plan.title}`, "roundtable",
  );

  // Build review prompt
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
    `각 subtask의 task 파일(\`docs/plans/${slugifyPlanTitle(plan.title)}-task-*.md\`)을 읽고 아래 3가지를 확인하세요:`,
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
    `- fail 시 반드시 **파일:줄번호 + 구체적 결함 설명**을 findings에 포함`,
    `- fail 시 \`failed_subtask_ids: [N, M]\` 형식으로 해당 subtask 번호 명시`,
    `- 개선 제안은 findings가 아닌 **recommendations**에 분리`,
  ].filter(Boolean).join("\n");

  // Save RT config for the review branch
  const participants: RoundtableParticipant[] = engines.map((eng, i) => ({
    name: `Reviewer-${String.fromCharCode(65 + i)}`,
    engine: eng,
    role: "reviewer" as const,
  }));

  const rtConfig = JSON.stringify({ participants, mode: "sequential" });
  await invoke("save_rt_config", { conversationId: shadowConvId, config: rtConfig });

  // Create user message with the review prompt
  await invoke("create_user_message", { input: { conversationId: shadowConvId, content: prompt } });

  return { branch, shadowConvId };
}

// ─── Phase E: Process review verdict ────────────────────────────────────────

export async function processReviewVerdict(
  plan: Plan,
  verdict: ParsedReviewVerdict,
): Promise<void> {
  const detail = JSON.stringify({
    verdict: verdict.verdict,
    findings: verdict.findings,
    recommendations: verdict.recommendations,
  });

  if (verdict.verdict === "pass") {
    await planApi.updatePlanPhase(plan.id, "done");
    await planApi.updatePlanStatus(plan.id, "done");
    await planApi.createPlanEvent(plan.id, "review_passed", "reviewer", detail);
    // Archive all related branches when plan is done
    if (plan.implementationBranchId) {
      await invoke("archive_branch", { id: plan.implementationBranchId }).catch(() => {});
    }
    if (plan.reviewBranchId) {
      await invoke("archive_branch", { id: plan.reviewBranchId }).catch(() => {});
    }
  } else if (verdict.verdict === "fail") {
    await planApi.updatePlanPhase(plan.id, "rework");
    await planApi.createPlanEvent(plan.id, "review_failed", "reviewer", detail);

    // Doom loop detection: count review_failed events from plan_events
    const events = await planApi.listPlanEvents(plan.id);
    const failEvents = events.filter((e) => e.eventType === "review_failed");
    const failCount = failEvents.length;

    // At 2+ failures: compare findings to detect design vs implementation issue
    if (failCount >= 2) {
      try {
        // Compare latest two failures for file overlap
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
        // Also detect same finding type: if >50% of findings match substring
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

    // Escalate at 2 failures if overlap detected, always at 3
    if (failCount >= 3) {
      await planApi.updatePlanPhase(plan.id, "subtask_review");
      await planApi.createPlanEvent(
        plan.id,
        "doom_loop_escalated",
        "system",
        `Review 실패 ${failCount}회 — 설계 재검토로 자동 에스컬레이션`,
      );
    }
  } else {
    // conditional — log event, user decides
    await planApi.createPlanEvent(plan.id, "review_failed", "reviewer", detail);
  }

  // Generate review report document
  syncReviewReport(plan.id, verdict);
}

// ─── Plan Revision (from Implementation Branch) ────────────────────────────

/**
 * Request plan revision from the Architect.
 *
 * Compresses the Implementation Branch conversation and sends it to the
 * main conversation's Architect agent, asking for a revised plan-proposal.
 *
 * Flow: Developer Branch → compress conversation → Architect reviews →
 *       produces revised plan-proposal → user merges via MergeBranchButton.
 */
export async function requestPlanRevision(
  plan: Plan,
  branchMessages: Message[],
  architectEngine: string = "claude",
  sendToArchitect: (engine: string, prompt: string, systemPrompt?: string) => Promise<void> = async () => {},
): Promise<void> {
  // Compress branch conversation into a context-only summary (not shown to user)
  const branchSummary = branchMessages
    .slice(-20)
    .map((m) => {
      const role = m.role === "assistant"
        ? `assistant${m.persona ? `:${m.persona}` : ""}${m.engine ? ` (${m.engine})` : ""}`
        : m.role;
      const content = m.content.length > 800
        ? m.content.slice(0, 800) + "…"
        : m.content;
      return `[${role}] ${content}`;
    })
    .join("\n\n");

  const planContext = await buildPlanContext(plan);

  // Build lightweight project analysis for Architect context
  let projectAnalysis = "";
  try {
    const pp = await getProjectPath();
    if (pp) {
      const stack = await invoke<{ keywords: string[]; detectedFiles: string[] }>("detect_project_stack", { projectPath: pp }).catch(() => null);
      if (stack && stack.keywords.length > 0) {
        const topKeywords = stack.keywords.slice(0, 15).join(", ");
        projectAnalysis = [
          `### 프로젝트 분석 (자동)`,
          `- 감지된 매니페스트: ${stack.detectedFiles.join(", ")}`,
          `- 주요 기술: ${topKeywords}`,
          "",
        ].join("\n");
      }
    }
  } catch { /* best-effort */ }

  // System prompt: full context for the Architect (not visible in chat)
  const systemPrompt = [
    `당신은 Architect입니다. Implementation Branch에서 계획 수정 요청이 왔습니다.`,
    `아래 정보를 기반으로 수정된 Plan을 \`<!-- tunaflow:plan-proposal -->\` 형식으로 제안하세요.`,
    `변경 이유를 간단히 설명하고, 기존 subtask 중 유지/수정/삭제할 항목을 명확히 구분하세요.`,
    "",
    projectAnalysis,
    `### 태스크 작성 규칙`,
    `각 subtask의 작업 지시서에 반드시 포함:`,
    `1. **변경 대상 파일** — 정확한 경로 (ContextPack의 graph/rawq 섹션 참고)`,
    `2. **변경 내용** — 추가/수정/삭제할 코드의 의도`,
    `3. **의존성** — 선행 태스크`,
    `4. **검증 조건** — Developer가 자가 검증할 수 있는 구체적 기준`,
    `5. **위험 요소** — 사이드 이펙트 (graph의 impacted files 참고)`,
    "",
    `### 기존 Plan`,
    planContext,
    "",
    `### Implementation Branch 논의 내용`,
    branchSummary.slice(0, 6000),
  ].join("\n");

  // User-visible message: short summary only
  const prompt = `[계획 수정 요청] "${plan.title}" (rev.${plan.revision}) — Implementation Branch 논의를 반영하여 Plan 수정을 요청합니다.`;

  // Send via injected callback (caller handles persona management)
  await sendToArchitect(architectEngine, prompt, systemPrompt);

  await planApi.createPlanEvent(plan.id, "revision_requested", "user", `from implementation branch, architect=${architectEngine}`);
}

// ─── Message scanning ──────────────────────────────────────────────��────────

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
    if (!reviewVerdict && hasReviewVerdict(msg.content)) {
      const v = extractReviewVerdict(msg.content);
      if (v) reviewVerdict = v;
    }
  }

  return { implPlan, implComplete, reviewVerdict };
}
