import { invoke } from "@tauri-apps/api/core";
import type { Plan, PlanEvent, PlanPhase, PlanSubtask, CreatePlanInput } from "@/types";

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
