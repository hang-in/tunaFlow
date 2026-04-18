/**
 * aggregateReviewVerdicts — roll up multiple reviewers' verdicts into a single
 * vote tally for the Plan → Review RT flow.
 *
 * Design notes (Harness roadmap item 12 / rtAlgorithmEnhancementIdeas.md P1):
 *  - The individual verdicts are NOT overwritten — the synthesizer uses the
 *    tally as input and produces a separate MoA-style summary message.
 *  - Tally rule is **intentionally conservative**:
 *     - unanimous pass → pass
 *     - any fail → fail  (one reviewer saying "bug found" overrides agreement)
 *     - otherwise        → conditional
 *    Rationale: in a code review, a single credible failure claim should block
 *    merge. The synthesizer can still flag dissent if the fail reason is weak.
 *  - Per-dimension rubric stats (mean / min / max / stddev-ish) help the
 *    synthesizer flag areas of disagreement explicitly.
 */
import type { ParsedReviewVerdict, ReviewRubric, ReviewVerdict } from "@/lib/planProposalParser";

export interface RubricDimensionStats {
  /** Arithmetic mean across reviewers that reported this rubric. */
  mean: number;
  min: number;
  max: number;
  /** Sample standard deviation (0 if only one reviewer). Used as disagreement signal. */
  stddev: number;
  /** How many reviewers actually provided a rubric score for this dimension. */
  count: number;
}

export interface ReviewVerdictAggregate {
  /** Total reviewer count (including those without rubric). */
  reviewerCount: number;
  /** Final consensus verdict — per the tally rule above. */
  verdict: ReviewVerdict;
  votes: {
    pass: number;
    fail: number;
    conditional: number;
  };
  /** True when all reviewers agreed on verdict. */
  unanimous: boolean;
  /** True when the aggregate verdict was fail due to a single minority fail vote. */
  minorityFail: boolean;
  /** Per-dimension stats. `null` means no reviewer provided a rubric at all. */
  rubric: null | {
    planCoverage: RubricDimensionStats;
    codeQuality: RubricDimensionStats;
    testCoverage: RubricDimensionStats;
    docQuality: RubricDimensionStats;
    convention: RubricDimensionStats;
    /** Max stddev across dimensions — useful as "overall disagreement" signal. */
    maxStddev: number;
  };
  /** Union of findings across reviewers, deduplicated by lowercased prefix. */
  findings: string[];
  /** Union of recommendations, deduplicated. */
  recommendations: string[];
  /** Union of failed subtask ids. */
  failedSubtaskIds: number[];
}

/** A verdict + the reviewer name who emitted it (for UI display). */
export interface NamedVerdict extends ParsedReviewVerdict {
  reviewerName?: string;
}

function rubricDimensionStats(values: number[]): RubricDimensionStats {
  if (values.length === 0) {
    return { mean: 0, min: 0, max: 0, stddev: 0, count: 0 };
  }
  const sum = values.reduce((a, b) => a + b, 0);
  const mean = sum / values.length;
  const min = Math.min(...values);
  const max = Math.max(...values);
  // Sample stddev — n=1 yields 0 (no disagreement signal possible with 1 sample).
  const variance = values.length > 1
    ? values.reduce((a, v) => a + (v - mean) ** 2, 0) / (values.length - 1)
    : 0;
  return { mean, min, max, stddev: Math.sqrt(variance), count: values.length };
}

/** Dedupe findings by lowercased 80-char prefix so "src/x.ts:1 ..." matches. */
function dedupeByPrefix(items: string[], prefixLen = 80): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const item of items) {
    const trimmed = item.trim();
    if (!trimmed) continue;
    const key = trimmed.slice(0, prefixLen).toLowerCase();
    if (seen.has(key)) continue;
    seen.add(key);
    out.push(trimmed);
  }
  return out;
}

export function aggregateReviewVerdicts(
  verdicts: NamedVerdict[],
): ReviewVerdictAggregate | null {
  if (verdicts.length === 0) return null;

  const votes = { pass: 0, fail: 0, conditional: 0 };
  for (const v of verdicts) votes[v.verdict]++;

  // Tally rule — see module header.
  let consensus: ReviewVerdict;
  if (votes.fail > 0) consensus = "fail";
  else if (votes.pass === verdicts.length) consensus = "pass";
  else consensus = "conditional";

  const unanimous =
    votes.pass === verdicts.length ||
    votes.fail === verdicts.length ||
    votes.conditional === verdicts.length;

  const minorityFail = consensus === "fail" && votes.fail < verdicts.length;

  // Rubric stats — only across reviewers who actually reported rubric.
  const rubrics: ReviewRubric[] = verdicts
    .map((v) => v.rubric)
    .filter((r): r is ReviewRubric => !!r);

  let rubric: ReviewVerdictAggregate["rubric"] = null;
  if (rubrics.length > 0) {
    const dim = (pick: (r: ReviewRubric) => number) => rubricDimensionStats(rubrics.map(pick));
    const stats = {
      planCoverage: dim((r) => r.planCoverage),
      codeQuality: dim((r) => r.codeQuality),
      testCoverage: dim((r) => r.testCoverage),
      docQuality: dim((r) => r.docQuality),
      convention: dim((r) => r.convention),
    };
    const maxStddev = Math.max(
      stats.planCoverage.stddev,
      stats.codeQuality.stddev,
      stats.testCoverage.stddev,
      stats.docQuality.stddev,
      stats.convention.stddev,
    );
    rubric = { ...stats, maxStddev };
  }

  const findings = dedupeByPrefix(verdicts.flatMap((v) => v.findings));
  const recommendations = dedupeByPrefix(verdicts.flatMap((v) => v.recommendations));
  const failedSubtaskIds = Array.from(
    new Set(verdicts.flatMap((v) => v.failedSubtaskIds)),
  ).sort((a, b) => a - b);

  return {
    reviewerCount: verdicts.length,
    verdict: consensus,
    votes,
    unanimous,
    minorityFail,
    rubric,
    findings,
    recommendations,
    failedSubtaskIds,
  };
}

/**
 * Format a vote-tally summary suitable for injection into the synthesizer's
 * prompt. Kept short — the synthesizer re-reads each reviewer's full verdict
 * from the transcript, so this is only a structured snapshot.
 */
export function formatTallyForSynthesizer(agg: ReviewVerdictAggregate): string {
  const lines: string[] = [
    `## Vote Tally (from ${agg.reviewerCount} reviewer${agg.reviewerCount === 1 ? "" : "s"})`,
    `- pass: ${agg.votes.pass} / fail: ${agg.votes.fail} / conditional: ${agg.votes.conditional}`,
    `- aggregate verdict: **${agg.verdict}**${agg.unanimous ? " (unanimous)" : ""}${agg.minorityFail ? " (minority fail — investigate before overriding)" : ""}`,
  ];
  if (agg.rubric) {
    const r = agg.rubric;
    lines.push(
      `- rubric mean: plan=${r.planCoverage.mean.toFixed(1)}, code=${r.codeQuality.mean.toFixed(1)}, test=${r.testCoverage.mean.toFixed(1)}, doc=${r.docQuality.mean.toFixed(1)}, conv=${r.convention.mean.toFixed(1)}`,
      `- max disagreement (stddev): ${r.maxStddev.toFixed(2)} — flag dimensions >1.0 as contested`,
    );
  }
  if (agg.findings.length > 0) {
    lines.push("", "### Merged Findings (dedup)");
    for (const f of agg.findings.slice(0, 20)) lines.push(`- ${f}`);
  }
  if (agg.failedSubtaskIds.length > 0) {
    lines.push("", `### Failed subtask ids: ${agg.failedSubtaskIds.join(", ")}`);
  }
  return lines.join("\n");
}
