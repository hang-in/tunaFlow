import { invoke } from "@tauri-apps/api/core";
import type { Plan, PlanEvent, PlanPhase, PlanSubtask, SubtaskInput, CreatePlanInput } from "@/types";

export async function listPlansByConversation(conversationId: string): Promise<Plan[]> {
  return invoke<Plan[]>("list_plans_by_conversation", { conversationId });
}

export async function createPlan(input: CreatePlanInput): Promise<Plan> {
  return invoke<Plan>("create_plan", { input });
}

export async function updatePlanStatus(id: string, status: string): Promise<void> {
  return invoke("update_plan_status", { input: { id, status } });
}

export async function listSubtasks(planId: string): Promise<PlanSubtask[]> {
  return invoke<PlanSubtask[]>("list_subtasks", { planId });
}

export async function updateSubtaskStatus(
  id: string,
  status: string,
  outcome: string | null = null,
): Promise<void> {
  return invoke("update_subtask_status", { input: { id, status, outcome } });
}

export async function setSubtaskOwner(
  id: string,
  ownerAgent: string | null,
): Promise<void> {
  return invoke("set_subtask_owner", { id, ownerAgent });
}

// ─── Orchestration (Phase A) ────────────────────────────────────────────────

export async function updatePlanPhase(id: string, phase: PlanPhase): Promise<void> {
  return invoke("update_plan_phase", { id, phase });
}

export async function createPlanEvent(
  planId: string,
  eventType: string,
  actor?: string,
  detail?: string,
): Promise<PlanEvent> {
  return invoke<PlanEvent>("create_plan_event", { planId, eventType, actor, detail });
}

export async function listPlanEvents(planId: string): Promise<PlanEvent[]> {
  return invoke<PlanEvent[]>("list_plan_events", { planId });
}

export async function assignPlanEngines(
  id: string,
  engines: { architect?: string; developer?: string; reviewers?: string[] },
): Promise<void> {
  return invoke("assign_plan_engines", {
    id,
    architectEngine: engines.architect ?? null,
    developerEngine: engines.developer ?? null,
    reviewerEngines: engines.reviewers ? JSON.stringify(engines.reviewers) : null,
  });
}

export async function linkPlanBranch(
  id: string,
  branchType: "implementation" | "review",
  branchId: string | null,
): Promise<void> {
  return invoke("link_plan_branch", { id, branchType, branchId });
}

export async function replacePlanSubtasks(
  planId: string,
  subtasks: SubtaskInput[],
): Promise<PlanSubtask[]> {
  return invoke<PlanSubtask[]>("replace_plan_subtasks", { planId, subtasks });
}

export async function getPlan(id: string): Promise<Plan> {
  return invoke<Plan>("get_plan", { id });
}

export async function bumpPlanMajorVersion(id: string): Promise<void> {
  return invoke("bump_plan_major_version", { id });
}

/** Generate/update plan document markdown in project directory. Returns file path. */
export async function generatePlanDocument(planId: string, projectPath: string): Promise<string> {
  return invoke<string>("generate_plan_document", { planId, projectPath });
}

/** Generate review report. Returns file path. */
export async function generateReviewReport(
  planId: string, projectPath: string,
  verdict: string, findings: string[], recommendations: string[],
  reviewerEngines: string[], testOutput?: string,
): Promise<string> {
  return invoke<string>("generate_review_report", {
    planId, projectPath, verdict, findings, recommendations,
    reviewerEngines, testOutput: testOutput ?? null,
  });
}

/** Generate implementation result report. Returns file path. */
export async function generateResultReport(
  planId: string, projectPath: string,
  summary: string, subtaskResults: string[],
  knownIssues: string[], developerEngine?: string, branchLabel?: string,
): Promise<string> {
  return invoke<string>("generate_result_report", {
    planId, projectPath, summary, subtaskResults, knownIssues,
    developerEngine: developerEngine ?? null, branchLabel: branchLabel ?? null,
  });
}

export async function findPlanByBranch(branchId: string): Promise<Plan | null> {
  return invoke<Plan | null>("find_plan_by_branch", { branchId });
}
