/**
 * Workflow orchestration utilities.
 *
 * Connects plan phase transitions to branch creation, agent invocation,
 * and marker-based auto-transitions.
 */

import { invoke } from "@tauri-apps/api/core";
import type { Branch, Plan, Message, RoundtableParticipant } from "@/types";
import * as planApi from "./api/plans";
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
    const assistantMsgs = implMessages.filter((m) => m.role === "assistant");
    const summary = assistantMsgs.length > 0
      ? stripMarkers(assistantMsgs[assistantMsgs.length - 1].content.slice(0, 2000))
      : "(No implementation output)";

    // Extract subtask-done markers to build per-subtask results
    const { scanCompletedSubtasks } = await import("./planProposalParser");
    const completedNums = scanCompletedSubtasks(implMessages);
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
  const slug = plan.title.replace(/[^\w가-힣-]/g, "-").replace(/-+/g, "-").replace(/^-|-$/g, "").toLowerCase().slice(0, 80);
  const subtasks = await planApi.listSubtasks(plan.id);
  const taskFileList = subtasks.map((_, i) =>
    `- \`docs/plans/${slug}-task-${String(i + 1).padStart(2, "0")}.md\``
  ).join("\n");

  const taskItems = subtasks.map((_, i) =>
    `- \`docs/plans/${slug}-task-${String(i + 1).padStart(2, "0")}.md\``
  );
  const prompt = [
    `### 🔧 구현 시작`,
    ``,
    `**Plan**: "${plan.title}"`,
    ``,
    `**작업 지시서**:`,
    ...taskItems,
    ``,
    `각 task 파일을 읽고 순서대로 구현하세요.`,
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
    `당신은 코드 리뷰어입니다.`,
    "",
    `## Plan (원래 요구사항)`,
    planContext,
    "",
    `## Implementation (Developer 구현 결과)`,
    implSummary.slice(0, 6000),
    "",
    testOutput ? `## 테스트 결과\n${testOutput.slice(0, 3000)}\n` : "",
    `## 리뷰 기준`,
    `1. Plan의 모든 subtask가 구현되었는가?`,
    `2. 코드 품질 (버그, 보안, 성능)`,
    `3. 테스트 커버리지`,
    "",
    `리뷰 결과를 verdict (pass / fail / conditional)로 판정하세요.`,
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
    const failCount = events.filter((e) => e.eventType === "review_failed").length;
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

  // System prompt: full context for the Architect (not visible in chat)
  const systemPrompt = [
    `당신은 Architect입니다. Implementation Branch에서 계획 수정 요청이 왔습니다.`,
    `아래 정보를 기반으로 수정된 Plan을 \`<!-- tunaflow:plan-proposal -->\` 형식으로 제안하세요.`,
    `변경 이유를 간단히 설명하고, 기존 subtask 중 유지/수정/삭제할 항목을 명확히 구분하세요.`,
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
