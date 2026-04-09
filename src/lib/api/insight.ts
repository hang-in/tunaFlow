import { invoke } from "@tauri-apps/api/core";
import type {
  InsightSession,
  InsightFinding,
  InsightReport,
  InsightCategory,
  InsightFindingStatus,
} from "@/types";

// ── Session ──────────────────────────────────────────────────

export async function createInsightSession(
  projectKey: string,
  categories?: InsightCategory[],
): Promise<InsightSession> {
  return invoke<InsightSession>("create_insight_session", {
    input: { projectKey, categories },
  });
}

export async function getInsightSession(
  sessionId: string,
): Promise<InsightSession> {
  return invoke<InsightSession>("get_insight_session", { sessionId });
}

export async function listInsightSessions(
  projectKey: string,
): Promise<InsightSession[]> {
  return invoke<InsightSession[]>("list_insight_sessions", { projectKey });
}

export async function updateInsightSessionStatus(
  sessionId: string,
  status: string,
  summary?: string,
  testOutput?: string,
): Promise<InsightSession> {
  return invoke<InsightSession>("update_insight_session_status", {
    sessionId,
    status,
    summary,
    testOutput,
  });
}

// ── Findings ──────��──────────────────────────────────────────

export interface CreateInsightFindingInput {
  sessionId: string;
  projectKey: string;
  category: InsightCategory;
  severity: string;
  fixDifficulty: string;
  title: string;
  description: string;
  filePath?: string;
  lineNumber?: number;
  snippet?: string;
  estimatedFiles?: number;
}

export async function createInsightFindingsBatch(
  findings: CreateInsightFindingInput[],
): Promise<InsightFinding[]> {
  return invoke<InsightFinding[]>("create_insight_findings_batch", { findings });
}

export async function listInsightFindings(
  sessionId: string,
  category?: InsightCategory,
): Promise<InsightFinding[]> {
  return invoke<InsightFinding[]>("list_insight_findings", {
    sessionId,
    category,
  });
}

export async function updateInsightFindingStatus(
  findingId: string,
  status: InsightFindingStatus,
  resolution?: string,
  planId?: string,
): Promise<InsightFinding> {
  return invoke<InsightFinding>("update_insight_finding_status", {
    findingId,
    status,
    resolution,
    planId,
  });
}

export async function updateInsightFindingsBatchStatus(
  findingIds: string[],
  status: InsightFindingStatus,
): Promise<number> {
  return invoke<number>("update_insight_findings_batch_status", {
    findingIds,
    status,
  });
}

export async function resolveInsightFindingsByPlan(
  planId: string,
): Promise<number> {
  return invoke<number>("resolve_insight_findings_by_plan", { planId });
}

// ── Reports ──────────────────────────────────���───────────────

export async function createInsightReport(
  sessionId: string,
  projectKey: string,
  reportType: "category" | "meta",
  content: string,
  category?: string,
): Promise<InsightReport> {
  return invoke<InsightReport>("create_insight_report", {
    input: { sessionId, projectKey, reportType, category, content },
  });
}

export async function listInsightReports(
  sessionId: string,
): Promise<InsightReport[]> {
  return invoke<InsightReport[]>("list_insight_reports", { sessionId });
}

// ── Extraction pipeline ──────────────────────────────────────

export interface ExtractedSnippet {
  file: string;
  line: number;
  snippet: string;
  scope?: string;
  confidence: number;
  query: string;
}

export interface CategoryExtraction {
  category: string;
  snippets: ExtractedSnippet[];
  extraContext: string[];
}

export interface TestRunResult {
  testType: string;
  passed: number;
  failed: number;
  skipped: number;
  durationMs: number;
  output: string;
  success: boolean;
}

export interface ExtractionResult {
  categories: CategoryExtraction[];
  testOutput?: TestRunResult;
  crgSummary?: string;
  failureLessons: string[];
  memoryTopics: string[];
}

export async function exportInsightToFiles(
  sessionId: string,
  projectPath: string,
): Promise<number> {
  return invoke<number>("export_insight_to_files", { sessionId, projectPath });
}

export async function runInsightExtraction(
  projectKey: string,
  projectPath: string,
  categories?: InsightCategory[],
): Promise<ExtractionResult> {
  return invoke<ExtractionResult>("run_insight_extraction", {
    projectKey,
    projectPath,
    categories,
  });
}
