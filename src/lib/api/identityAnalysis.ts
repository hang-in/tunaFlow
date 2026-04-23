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

export function getIdentityAnalysisThreshold(): Promise<number> {
  return invoke("get_identity_analysis_threshold");
}

export function setIdentityAnalysisThreshold(threshold: number): Promise<void> {
  return invoke("set_identity_analysis_threshold", { input: { threshold } });
}

export function getBackgroundInsightEnabled(): Promise<boolean> {
  return invoke("get_background_insight_enabled");
}

export function setBackgroundInsightEnabled(enabled: boolean): Promise<void> {
  return invoke("set_background_insight_enabled", { input: { enabled } });
}

export type BackgroundJobCounts = {
  pending: number;
  running: number;
};

export function countBackgroundJobs(): Promise<BackgroundJobCounts> {
  return invoke("count_background_jobs");
}

export function cancelBackgroundJob(id: string): Promise<void> {
  return invoke("cancel_background_job", { id });
}
