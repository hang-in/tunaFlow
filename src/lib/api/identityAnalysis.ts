/**
 * FE wrapper for identity analysis (projectIdentityAnalysisPlan subtask-04).
 *
 * Rust 측 Tauri commands:
 * - `list_identity_summaries(project_key)` → project 별 identity_summary artifact list
 * - `get_artifact(id)` → 단건 artifact
 * - `trigger_identity_analysis_now(input: { project_key, force })` → 수동 trigger
 * - `get_identity_trigger_status(project_key)` → UI 상태 배지
 */
import { invoke } from "@tauri-apps/api/core";
import type { Artifact } from "@/types";

export type IdentityTriggerDecision = {
  shouldRun: boolean;
  donePlanCount: number;
  eligibleArtifactCount: number;
  threshold: number;
  reason: string;
};

export function listIdentitySummaries(projectKey: string): Promise<Artifact[]> {
  return invoke("list_identity_summaries", { projectKey });
}

export function getArtifact(id: string): Promise<Artifact> {
  return invoke("get_artifact", { id });
}

export function triggerIdentityAnalysisNow(
  projectKey: string,
  force = false,
): Promise<IdentityTriggerDecision> {
  return invoke("trigger_identity_analysis_now", {
    input: { projectKey, force },
  });
}

export function getIdentityTriggerStatus(
  projectKey: string,
): Promise<IdentityTriggerDecision> {
  return invoke("get_identity_trigger_status", { projectKey });
}
