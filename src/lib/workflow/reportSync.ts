/**
 * Document sync utilities — plan document, review report, result report generation.
 */
import type { Message, Plan } from "@/types";
import * as planApi from "../api/plans";
import type { ParsedReviewVerdict } from "../planProposalParser";
import { getProjectPath, createTestReportArtifact } from "./helpers";
import { stripTunaflowMarkers } from "./markerScrub";

/**
 * UTF-8 boundary-safe truncation by code points.
 * 잘렸을 때 [...truncated, original N chars] 마커를 붙여 다운스트림(reviewer 등)이
 * 잘림 사실을 인지할 수 있게 한다. surrogate pair 안전 (Array.from 사용).
 */
export function truncateSafe(text: string, limit: number): string {
  if (typeof text !== "string" || text.length === 0) return text ?? "";
  // 빠른 경로: byte length 가 limit 보다 충분히 작으면 코드포인트 길이도 작음
  if (text.length <= limit) return text;
  const codePoints = Array.from(text);
  if (codePoints.length <= limit) return text;
  const head = codePoints.slice(0, limit).join("");
  return `${head}\n\n[…truncated, original ${codePoints.length} chars]`;
}

/**
 * Sentinel guard: prior result.md echo 인지 식별.
 * `# Implementation Result:` 헤더와 `> Plan Revision:` 헤더가 본문 첫 200자 안에
 * **둘 다** 출현하면 result.md echo 로 간주. 한쪽만 매칭하면 false positive 위험이
 * 있어 거부. 이 함수는 syncResultReport 안에서만 사용.
 */
export function isResultMdEcho(content: string): boolean {
  if (typeof content !== "string" || content.length === 0) return false;
  const head = content.slice(0, 200);
  const hasImplHeader = /^#\s*Implementation Result:/m.test(head);
  const hasRevisionHeader = /^>\s*Plan Revision:/m.test(head);
  return hasImplHeader && hasRevisionHeader;
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
    const rawAssistantMsgs = relevantMessages.filter((m) => m.role === "assistant");
    // self-include guard: prior result.md 인용 메시지 제외 (두 헤더 동시 매칭만)
    const assistantMsgs = rawAssistantMsgs.filter((m) => !isResultMdEcho(m.content));
    const excludedEchoCount = rawAssistantMsgs.length - assistantMsgs.length;
    if (excludedEchoCount > 0) {
      console.debug(`[syncResultReport] excluded ${excludedEchoCount} echoed result.md messages`);
    }
    const summary = assistantMsgs.length > 0
      ? stripTunaflowMarkers(truncateSafe(assistantMsgs[assistantMsgs.length - 1].content, 8000))
      : "(No implementation output)";

    const { scanCompletedSubtasks } = await import("../planProposalParser");
    const completedNums = scanCompletedSubtasks(implMessages);
    const subtaskResults = assistantMsgs
      .slice(-10)
      .map((m) => stripTunaflowMarkers(truncateSafe(m.content, 2000)))
      .filter((c) => c.trim().length > 0);

    const knownIssues: string[] = [];

    await planApi.generateResultReport(
      planId, pp, summary, subtaskResults, knownIssues,
      developerEngine, branchLabel,
    );
  } catch (e) { console.warn("[tunaflow]", e); }
}
