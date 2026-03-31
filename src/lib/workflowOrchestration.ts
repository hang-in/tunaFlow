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

/** Generate/update plan document in project directory. Fire-and-forget. */
export async function syncPlanDocument(planId: string): Promise<void> {
  try {
    const { useChatStore } = await import("@/stores/chatStore");
    const projectKey = useChatStore.getState().selectedProjectKey;
    if (!projectKey) return;
    const project = await invoke("get_project", { key: projectKey }) as { path?: string };
    if (!project?.path) return;
    await planApi.generatePlanDocument(planId, project.path);
  } catch { /* fire-and-forget */ }
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
  const input = {
    conversationId: plan.conversationId,
    label,
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

  // Build developer prompt — caller will send via sendThreadMessage
  const planContext = await buildPlanContext(plan);
  const prompt = [
    `당신은 Developer입니다. 아래 Plan의 모든 subtask를 **순서대로** 구현하세요.`,
    "",
    planContext,
    "",
    `## 작업 규칙`,
    `1. subtask 순서대로 진행하세요.`,
    `2. 각 subtask의 상세 설계(details)를 따르세요.`,
    `3. 각 subtask 완료 시 \`<!-- tunaflow:subtask-done:N -->\`을 포함하세요 (N = subtask 번호).`,
    `4. 전체 구현 완료 후 \`<!-- tunaflow:impl-complete -->\`를 포함하세요.`,
  ].join("\n");

  return { branch, shadowConvId, prompt };
}

// ─── Phase D: Approve impl-plan → Start implementation ──────────────────────

/** Returns the prompt string — caller sends via sendThreadMessage */
export async function approveImplPlan(
  plan: Plan,
): Promise<string> {
  await planApi.createPlanEvent(plan.id, "impl_approved", "user");
  return "실행 계획이 승인되었습니다. 구현을 시작하세요. 완료되면 `<!-- tunaflow:impl-complete -->`를 포함하세요.";
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
    `\`<!-- tunaflow:review-verdict -->\` 형식으로 판정하세요.`,
    `verdict: pass | fail | conditional`,
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
  } else if (verdict.verdict === "fail") {
    await planApi.updatePlanPhase(plan.id, "rework");
    await planApi.createPlanEvent(plan.id, "review_failed", "reviewer", detail);
  } else {
    // conditional — log event, user decides
    await planApi.createPlanEvent(plan.id, "review_failed", "reviewer", detail);
  }
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
