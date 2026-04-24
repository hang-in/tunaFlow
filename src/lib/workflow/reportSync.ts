/**
 * Document sync utilities — plan document, review report, result report generation.
 */
import type { Message, Plan } from "@/types";
import * as planApi from "../api/plans";
import type { ParsedReviewVerdict } from "../planProposalParser";
import { getProjectPath, createTestReportArtifact } from "./helpers";
import { stripTunaflowMarkers } from "./markerScrub";

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
    // testOutput 은 LLM/CI raw 문자열일 가능성이 있어 스크럽 통과.
    // verdict.findings/recommendations 는 planProposalParser 에서 marker 제거 후
    // payload 만 넘겨주므로 추가 처리 없음.
    const scrubbedTestOutput = testOutput ? stripTunaflowMarkers(testOutput) : undefined;
    await planApi.generateReviewReport(
      planId, pp, verdict.verdict,
      verdict.findings, verdict.recommendations,
      reviewerEngines, scrubbedTestOutput,
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
      ? stripTunaflowMarkers(assistantMsgs[assistantMsgs.length - 1].content.slice(0, 2000))
      : "(No implementation output)";

    const { scanCompletedSubtasks } = await import("../planProposalParser");
    const completedNums = scanCompletedSubtasks(implMessages);
    const subtaskResults = assistantMsgs
      .slice(-10)
      .map((m) => stripTunaflowMarkers(m.content.slice(0, 500)))
      .filter((c) => c.trim().length > 0);

    const knownIssues: string[] = [];

    await planApi.generateResultReport(
      planId, pp, summary, subtaskResults, knownIssues,
      developerEngine, branchLabel,
    );
  } catch (e) { console.warn("[tunaflow]", e); }
}
