import { invoke } from "@tauri-apps/api/core";

export interface TestRunResult {
  testType: string;
  passed: number;
  failed: number;
  skipped: number;
  durationMs: number;
  output: string;
  success: boolean;
}

export async function runProjectTests(
  projectPath: string,
  testType?: string,
): Promise<TestRunResult> {
  return invoke<TestRunResult>("run_project_tests", {
    projectPath,
    testType: testType ?? null,
  });
}
