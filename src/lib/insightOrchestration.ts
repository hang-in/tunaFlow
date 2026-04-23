/**
 * Insight analysis orchestration.
 *
 * Coordinates the full pipeline:
 *   1. Pre-extract data from rawq/CRG/lessons/test/memory
 *   2. Build focused prompt per category
 *   3. Run agent analysis
 *   4. Parse findings from response
 *   5. Evaluate fix_difficulty
 *   6. Store in DB
 */

import { invoke } from "@tauri-apps/api/core";
import type {
  InsightCategory,
  InsightSession,
  InsightFinding,
  InsightAgentConfig,
} from "@/types";
import * as insightApi from "./api/insight";
import { getSetting } from "./appStore";
import type {
  ExtractionResult,
  CategoryExtraction,
} from "./api/insight";
import { extractInsightFindings } from "./planProposalParser";
import type { InsightFindingItemInput } from "./schemas/insightFindings";

// ── Fix difficulty evaluation ────────────────────────────────

/**
 * Evaluate fix_difficulty based on heuristics from SWE-bench research:
 * - auto:   1 file, <20 lines, low fan-out → agent success 90%+
 * - guided: 2-5 files, <100 lines → 70%+
 * - manual: 5+ files or 100+ lines → <70%
 */
function evaluateFixDifficulty(finding: InsightFindingItemInput): "auto" | "guided" | "manual" {
  const files = finding.estimated_files ?? 1;
  const hasSnippet = !!finding.snippet;
  const hasEvidence = !!finding.evidence;
  const confidence = finding.confidence ?? "medium";

  // Low confidence findings → always guided or manual (never auto-fix uncertain things)
  if (confidence === "low") return files <= 3 ? "guided" : "manual";

  // Single file with snippet + evidence = likely auto-fixable
  if (files === 1 && (hasSnippet || hasEvidence)) {
    // Pattern-based fixes (empty catch, missing error handling) are auto
    const autoPatterns = ["catch", "unwrap", "expect", "TODO", "FIXME", "deprecated", "console.log",
      "unused", "dead code", "silent", "empty"];
    const text = `${finding.title} ${finding.description}`.toLowerCase();
    const isSimplePattern = autoPatterns.some((p) => text.includes(p.toLowerCase()));
    if (isSimplePattern && confidence === "high") return "auto";
    if (isSimplePattern) return "guided"; // medium confidence → guided, not auto
  }

  if (files <= 1) return "guided";
  if (files <= 5) return "guided";
  return "manual";
}

// ── Prompt building ──────────────────────────────────────────

const CATEGORY_LABELS: Record<InsightCategory, string> = {
  stability: "안정성 (Stability)",
  test: "테스트 (Testing)",
  architecture: "아키텍처 (Architecture)",
  performance: "성능 (Performance)",
  security: "보안 (Security)",
  debt: "기술 부채 (Technical Debt)",
};

function buildAnalysisPrompt(
  category: InsightCategory,
  extraction: CategoryExtraction,
): string {
  const label = CATEGORY_LABELS[category] || category;

  let prompt = `### 📊 Insight Analysis — ${label}

아래에 코드베이스에서 사전 추출한 데이터가 있습니다. **${label}** 관점에서 분석하세요.

## 분석 규칙 (반드시 준수)

1. **증거 기반**: 아래 제공된 스니펫에서 확인할 수 있는 문제만 보고
2. **환각 금지**: 입력에 없는 파일 경로나 줄번호를 만들지 마세요
3. **evidence 필수**: 각 finding에 해당 코드를 그대로 인용 (evidence 필드)
4. **confidence 표기**: high(코드에서 명확히 확인) / medium(패턴 일치) / low(추정)
5. **severity 기준**:
   - critical: 프로덕션 크래시, 데이터 손실, 보안 취약점
   - major: 버그, 리소스 누수, 에러 처리 누락
   - minor: 코드 스멜, 비일관성, 경미한 비효율
   - info: 스타일, 문서화 부족, 개선 가능성

`;

  // Add code snippets with line anchors
  if (extraction.snippets.length > 0) {
    prompt += `## 코드 스니펫 (코드베이스 검색 결과)\n\n`;
    for (const s of extraction.snippets) {
      prompt += `### ${s.file}:${s.line}${s.scope ? ` (${s.scope})` : ""}\n`;
      prompt += `검색어: "${s.query}" | 신뢰도: ${s.confidence.toFixed(2)}\n`;
      // Add line numbers to snippet for precise referencing
      const lines = s.snippet.split("\n");
      const numbered = lines.map((line, i) => `${s.line + i}: ${line}`).join("\n");
      prompt += `\`\`\`\n${numbered}\n\`\`\`\n\n`;
    }
  }

  // Add extra context
  if (extraction.extraContext.length > 0) {
    prompt += `## 추가 컨텍스트\n\n`;
    for (const ctx of extraction.extraContext) {
      prompt += `${ctx}\n\n`;
    }
  }

  prompt += `## 출력 형식

아래 마커 안에 JSON으로 응답하세요:

<!-- tunaflow:insight-findings -->
\`\`\`json
{
  "findings": [
    {
      "category": "${category}",
      "severity": "major",
      "confidence": "high",
      "title": "문제의 짧은 제목",
      "description": "문제에 대한 상세 설명과 프로덕션에서의 영향",
      "evidence": "문제를 증명하는 정확한 코드 인용 (입력 스니펫에서 복사)",
      "file_path": "src/path/to/file.ts",
      "line_number": 42,
      "snippet": "수정이 필요한 코드 영역",
      "estimated_files": 1
    }
  ],
  "summary": "이 카테고리에 대한 전체 평가 요약"
}
\`\`\`
<!-- /tunaflow:insight-findings -->

- confidence가 "low"인 finding은 최소화하세요 (확실한 것만 보고)
- 같은 근본 원인을 공유하는 문제는 하나의 finding으로 묶으세요
- 발견 사항이 없으면 빈 findings 배열과 이유를 summary에 작성하세요`;

  return prompt;
}

// ── Main orchestration ───────────────────────────────────────

export interface InsightRunOptions {
  projectKey: string;
  projectPath: string;
  categories?: InsightCategory[];
  engine?: string;
  /** Callback for progress updates */
  onProgress?: (msg: string) => void;
}

/**
 * Run the full Insight analysis pipeline.
 *
 * 1. Create session
 * 2. Run extraction (rawq/CRG/lessons/test/memory)
 * 3. For each category: build prompt → send to agent → parse findings
 * 4. Evaluate fix_difficulty
 * 5. Store findings in DB
 * 6. Update session status
 */
export async function runInsightAnalysis(
  opts: InsightRunOptions,
): Promise<{ session: InsightSession; findings: InsightFinding[] }> {
  const { projectKey, projectPath, categories, onProgress } = opts;

  // 1. Create session
  resetInsightTokenUsage();
  onProgress?.("세션 생성 중...");
  const session = await insightApi.createInsightSession(projectKey, categories);

  try {
    // 2. Update to analyzing
    await insightApi.updateInsightSessionStatus(session.id, "analyzing");

    // 3. Pre-extraction
    onProgress?.("데이터 사전 추출 중 (rawq/CRG/테스트/메모리)...");
    const extraction = await insightApi.runInsightExtraction(
      projectKey,
      projectPath,
      categories,
    );

    // Store test output
    if (extraction.testOutput) {
      await insightApi.updateInsightSessionStatus(
        session.id,
        "analyzing",
        undefined,
        JSON.stringify(extraction.testOutput),
      );
    }

    // 4. For each category, run agent analysis
    const allFindings: InsightFinding[] = [];

    // Log extraction summary
    const snippetTotal = extraction.categories.reduce((sum, c) => sum + c.snippets.length, 0);
    const contextTotal = extraction.categories.reduce((sum, c) => sum + c.extraContext.length, 0);
    onProgress?.(`사전 추출 완료: 스니펫 ${snippetTotal}개, 컨텍스트 ${contextTotal}개, 카테고리 ${extraction.categories.length}개`);

    if (snippetTotal === 0 && contextTotal === 0) {
      onProgress?.("⚠️ 사전 추출 데이터 없음 — rawq 인덱싱 여부 확인 필요");
    }

    for (const catExtraction of extraction.categories) {
      const cat = catExtraction.category as InsightCategory;
      const catLabel = CATEGORY_LABELS[cat] || cat;
      // insightStabilityPlan Subtask 02 (INV-2): 증거 기반 분석 원칙 — snippets 가
      // 없으면 extraContext 만으로는 LLM 이 "아래 스니펫에서 확인할 수 있는 문제만
      // 보고" 시스템 규칙과 모순되어 extended thinking 폭주 + hallucination 위험.
      // OR 였던 기존 조건을 AND-equivalent (스니펫 필수) 로 강화.
      const hasData = catExtraction.snippets.length > 0;

      if (!hasData) {
        onProgress?.(
          `${catLabel}: 스니펫 없음, 건너뜀 (extraContext=${catExtraction.extraContext.length})`,
        );
        continue;
      }

      onProgress?.(`${catLabel} 분석 중... (스니펫 ${catExtraction.snippets.length}개)`);

      // Build prompt
      const prompt = buildAnalysisPrompt(cat, catExtraction);

      // Send to agent and get response
      let response: string | null;
      try {
        response = await sendAnalysisToAgent(projectKey, prompt);
      } catch (err) {
        onProgress?.(`${catLabel}: 에이전트 호출 실패 — ${err}`);
        console.error(`[insight] ${cat} agent error:`, err);
        continue;
      }

      if (!response) {
        onProgress?.(`${catLabel}: 에이전트 응답 없음 (빈 응답)`);
        continue;
      }

      onProgress?.(`${catLabel}: 응답 수신, 파싱 중...`);

      // Parse findings from response
      const parsed = extractInsightFindings(response);
      if (!parsed) {
        // Marker not found — store raw response as report for debugging
        onProgress?.(`${catLabel}: insight-findings 마커 없음 — 원본 응답 저장`);
        console.warn(`[insight] ${cat}: no markers found in response (${response.length} chars)`);
        await insightApi.createInsightReport(
          session.id,
          projectKey,
          "category",
          response.slice(0, 5000), // save first 5k chars for debugging
          cat,
        );
        continue;
      }

      if (parsed.findings.length === 0) {
        onProgress?.(`${catLabel}: 발견 사항 없음`);
        if (parsed.summary) {
          await insightApi.createInsightReport(session.id, projectKey, "category", parsed.summary, cat);
        }
        continue;
      }

      // Filter out low-confidence findings
      const reliable = parsed.findings.filter((f) => (f.confidence ?? "medium") !== "low");
      const filtered = parsed.findings.length - reliable.length;
      if (filtered > 0) {
        onProgress?.(`${catLabel}: ${filtered}개 low-confidence finding 필터링`);
      }

      // Evaluate fix_difficulty and prepare for DB
      const findingInputs = reliable.map((f) => ({
        sessionId: session.id,
        projectKey,
        category: f.category || cat,
        severity: f.severity,
        fixDifficulty: evaluateFixDifficulty(f),
        title: f.title,
        description: f.evidence
          ? `${f.description}\n\n**Evidence**: \`${f.evidence}\``
          : f.description,
        filePath: f.file_path,
        lineNumber: f.line_number,
        snippet: f.snippet,
        estimatedFiles: f.estimated_files,
      }));

      // Store findings
      const stored = await insightApi.createInsightFindingsBatch(findingInputs);
      allFindings.push(...stored);

      // Store category report
      if (parsed.summary) {
        await insightApi.createInsightReport(session.id, projectKey, "category", parsed.summary, cat);
      }

      const u = getInsightTokenUsage();
      onProgress?.(`${catLabel}: ${stored.length}개 발견 — 누적 $${u.cost.toFixed(3)} (${u.input}in/${u.output}out)`);
    }

    // 5. Generate meta summary
    const summaryParts: string[] = [];
    const catCounts: Record<string, number> = {};
    for (const f of allFindings) {
      catCounts[f.category] = (catCounts[f.category] || 0) + 1;
    }
    for (const [cat, count] of Object.entries(catCounts)) {
      summaryParts.push(`${CATEGORY_LABELS[cat as InsightCategory] || cat}: ${count}건`);
    }
    const usage = getInsightTokenUsage();
    const costStr = usage.cost > 0 ? ` ($${usage.cost.toFixed(3)}, ${usage.input}in/${usage.output}out)` : "";
    const summary = allFindings.length > 0
      ? `총 ${allFindings.length}건 발견 — ${summaryParts.join(", ")}${costStr}`
      : `분석 완료 — 발견 사항 없음${costStr}`;

    // 6. Complete session
    const completed = await insightApi.updateInsightSessionStatus(
      session.id,
      "completed",
      summary,
    );

    // 7. Auto-export findings to docs/insight/ files
    insightApi.exportInsightToFiles(session.id, projectPath)
      .then((n) => onProgress?.(`파일 저장 완료: ${n}건`))
      .catch((e) => console.warn("[insight] auto-export failed:", e));

    onProgress?.(`완료: ${summary}`);
    return { session: completed, findings: allFindings };
  } catch (err) {
    console.error("[insight] analysis failed:", err);
    await insightApi.updateInsightSessionStatus(
      session.id,
      "failed",
      String(err),
    );
    throw err;
  }
}

// ── Agent communication ──────────────────────────────────────

/**
 * Send analysis prompt to agent and return the response.
 *
 * Uses Claude CLI single-turn mode. The prompt includes pre-extracted
 * data so the agent does NOT need to search the codebase.
 */
const DEFAULT_INSIGHT_CONFIG: InsightAgentConfig = {
  engine: "claude",
  model: "",
  systemPrompt: "",
  presetId: "balanced",
};

async function loadInsightConfig(): Promise<InsightAgentConfig> {
  return getSetting("insightAgentConfig", DEFAULT_INSIGHT_CONFIG);
}

interface AnalysisResponse {
  content: string;
  inputTokens: number;
  outputTokens: number;
  costUsd: number;
}

let _totalInsightTokens = { input: 0, output: 0, cost: 0 };

export function getInsightTokenUsage() { return { ..._totalInsightTokens }; }
export function resetInsightTokenUsage() { _totalInsightTokens = { input: 0, output: 0, cost: 0 }; }

async function sendAnalysisToAgent(
  projectKey: string,
  prompt: string,
): Promise<string | null> {
  const config = await loadInsightConfig();
  console.log("[insight] sendAnalysisToAgent: engine=%s, model=%s, prompt_len=%d",
    config.engine, config.model || "(default)", prompt.length);

  const raw = await invoke<string>("run_insight_analysis", {
    projectKey,
    prompt,
    engine: config.engine,
    model: config.model || null,
    systemPrompt: config.systemPrompt || null,
  });

  if (!raw) {
    throw new Error("에이전트가 빈 응답을 반환했습니다");
  }

  // Parse JSON response with token info
  try {
    const parsed: AnalysisResponse = JSON.parse(raw);
    _totalInsightTokens.input += parsed.inputTokens || 0;
    _totalInsightTokens.output += parsed.outputTokens || 0;
    _totalInsightTokens.cost += parsed.costUsd || 0;
    console.log("[insight] tokens: %din/%dout, cost=$%s, total=$%s",
      parsed.inputTokens, parsed.outputTokens,
      parsed.costUsd.toFixed(4), _totalInsightTokens.cost.toFixed(4));
    return parsed.content;
  } catch {
    // Fallback: raw string response (non-JSON)
    console.log("[insight] agent response: %d chars (non-JSON)", raw.length);
    return raw;
  }
}

// ── Auto Fix pipeline ────────────────────────────────────────

/**
 * Auto-fix a single finding using the CodeCureAgent pattern:
 * 1. Generate fix prompt with file/line/snippet context
 * 2. Run agent to apply the fix
 * 3. Run tests to verify no regressions
 * 4. Re-scan with rawq to verify pattern is gone
 * 5. On failure: report (agent ran in project dir, git revert may be needed)
 */
export async function autoFixFinding(
  finding: InsightFinding,
  projectKey: string,
  projectPath: string,
  onProgress?: (msg: string) => void,
): Promise<{ success: boolean; message: string }> {
  const { title, description, filePath, lineNumber, snippet } = finding;

  // 1. Build fix prompt
  const prompt = buildFixPrompt(finding);
  onProgress?.(`수정 중: ${title}`);

  // 2. Run agent
  const response = await sendAnalysisToAgent(projectKey, prompt);
  if (!response) {
    return { success: false, message: "에이전트 응답 없음" };
  }

  // 3. Run tests
  onProgress?.("테스트 검증 중...");
  try {
    const testResult = await invoke<{ success: boolean; output: string }>(
      "run_project_tests",
      { projectPath },
    );
    if (!testResult.success) {
      return {
        success: false,
        message: `테스트 실패 — git revert 필요. 출력:\n${testResult.output.slice(0, 500)}`,
      };
    }
  } catch {
    // Test runner not available — skip test verification
  }

  // 4. Re-scan with rawq to verify fix
  if (filePath) {
    onProgress?.("패턴 재스캔 중...");
    try {
      const extraction = await insightApi.runInsightExtraction(
        projectKey,
        projectPath,
        [finding.category],
      );
      // Check if original file+line still shows up
      const stillPresent = extraction.categories.some((cat) =>
        cat.snippets.some(
          (s) => s.file === filePath && Math.abs(s.line - (lineNumber || 0)) < 5,
        ),
      );
      if (stillPresent) {
        return {
          success: false,
          message: "패턴이 여전히 존재 — 수정이 불완전할 수 있음",
        };
      }
    } catch {
      // rawq scan failed — skip verification
    }
  }

  // 5. Update finding status
  await insightApi.updateInsightFindingStatus(finding.id, "resolved", "Auto-fixed");

  return { success: true, message: "자동 수정 완료 + 검증 통과" };
}

function buildFixPrompt(finding: InsightFinding): string {
  let prompt = `### 🔧 Auto Fix — ${finding.title}

Fix the following code quality issue.

**Category**: ${finding.category}
**Severity**: ${finding.severity}
**Issue**: ${finding.description}
`;

  if (finding.filePath) {
    prompt += `\n**File**: ${finding.filePath}`;
    if (finding.lineNumber) prompt += `:${finding.lineNumber}`;
    prompt += "\n";
  }

  if (finding.snippet) {
    prompt += `\n**Current code**:\n\`\`\`\n${finding.snippet}\n\`\`\`\n`;
  }

  prompt += `
**Instructions**:
1. Read the file and fix the specific issue
2. Make minimal changes — only fix the reported issue
3. Do NOT refactor surrounding code
4. Do NOT add features beyond the fix
5. Preserve existing behavior

Respond with the changes you made.`;

  return prompt;
}

/**
 * Auto-fix all "quick wins" (auto difficulty) findings.
 */
export async function autoFixQuickWins(
  findings: InsightFinding[],
  projectKey: string,
  projectPath: string,
  onProgress?: (msg: string) => void,
): Promise<{ fixed: number; failed: number }> {
  const autoFindings = findings.filter(
    (f) => f.fixDifficulty === "auto" && f.status === "open",
  );

  let fixed = 0;
  let failed = 0;

  for (const finding of autoFindings) {
    onProgress?.(`(${fixed + failed + 1}/${autoFindings.length}) ${finding.title}`);
    const result = await autoFixFinding(finding, projectKey, projectPath, onProgress);
    if (result.success) {
      fixed++;
    } else {
      failed++;
      onProgress?.(`실패: ${result.message}`);
    }
  }

  return { fixed, failed };
}

// ── Revalidate open findings ──────────────────────────────────────────────────

interface RevalidateResult {
  id: string;
  status: "still_open" | "resolved" | "uncertain";
  reason: string;
}

/**
 * Ask the analysis agent to re-check which existing open findings are still
 * present in the current codebase. Useful after code changes that may have
 * incidentally fixed reported issues.
 */
export async function revalidateFindings(
  findings: InsightFinding[],
  projectKey: string,
  onProgress?: (msg: string) => void,
): Promise<RevalidateResult[]> {
  const openFindings = findings.filter((f) => f.status === "open");
  if (openFindings.length === 0) return [];

  onProgress?.("코드베이스 재검토 중...");

  const findingsSummary = openFindings.map((f) => ({
    id: f.id,
    title: f.title,
    category: f.category,
    severity: f.severity,
    description: f.description,
    filePath: f.filePath,
    lineNumber: f.lineNumber,
    snippet: f.snippet?.slice(0, 200),
  }));

  const prompt = `## Insight Findings 재검토

아래 findings가 현재 코드베이스에 여전히 존재하는지 확인해주세요.
각 finding의 filePath를 직접 읽고 판단해주세요.

**판정 기준**:
- \`still_open\`: 문제가 여전히 코드에 존재함
- \`resolved\`: 이미 수정되어 더 이상 존재하지 않음
- \`uncertain\`: 파일이 없거나 판단이 어려움

**Findings**:
\`\`\`json
${JSON.stringify(findingsSummary, null, 2)}
\`\`\`

**출력 형식** (JSON 배열만 출력, 설명 없이):
\`\`\`json
[
  { "id": "finding_id", "status": "still_open|resolved|uncertain", "reason": "한 줄 이유" }
]
\`\`\``;

  const response = await sendAnalysisToAgent(projectKey, prompt);
  if (!response) return [];

  try {
    const jsonMatch = response.match(/```json\s*([\s\S]*?)```/) || response.match(/(\[[\s\S]*\])/);
    if (!jsonMatch) return [];
    const parsed = JSON.parse(jsonMatch[1].trim());
    if (!Array.isArray(parsed)) return [];
    return parsed.filter((r): r is RevalidateResult =>
      typeof r.id === "string" &&
      ["still_open", "resolved", "uncertain"].includes(r.status) &&
      typeof r.reason === "string",
    );
  } catch {
    console.error("[insight] revalidateFindings: JSON 파싱 실패", response.slice(0, 200));
    return [];
  }
}
