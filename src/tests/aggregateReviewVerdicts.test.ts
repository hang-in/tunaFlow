import { describe, it, expect } from "vitest";
import {
  aggregateReviewVerdicts,
  formatTallyForSynthesizer,
  type NamedVerdict,
} from "@/lib/aggregateReviewVerdicts";
import type { ParsedReviewVerdict, ReviewRubric, ReviewVerdict } from "@/lib/planProposalParser";

function mkVerdict(
  verdict: ReviewVerdict,
  opts?: {
    rubric?: Partial<ReviewRubric>;
    findings?: string[];
    recommendations?: string[];
    failedSubtaskIds?: number[];
    reviewerName?: string;
  },
): NamedVerdict {
  const fullRubric: ReviewRubric | undefined = opts?.rubric
    ? {
        planCoverage: opts.rubric.planCoverage ?? 3,
        codeQuality: opts.rubric.codeQuality ?? 3,
        testCoverage: opts.rubric.testCoverage ?? 3,
        docQuality: opts.rubric.docQuality ?? 3,
        convention: opts.rubric.convention ?? 3,
      }
    : undefined;
  const base: ParsedReviewVerdict = {
    verdict,
    rubric: fullRubric,
    findings: opts?.findings ?? [],
    recommendations: opts?.recommendations ?? [],
    failedSubtaskIds: opts?.failedSubtaskIds ?? [],
    raw: "",
  };
  return opts?.reviewerName ? { ...base, reviewerName: opts.reviewerName } : base;
}

describe("aggregateReviewVerdicts", () => {
  it("returns null for empty input", () => {
    expect(aggregateReviewVerdicts([])).toBeNull();
  });

  it("unanimous pass → pass, unanimous=true, minorityFail=false", () => {
    const agg = aggregateReviewVerdicts([mkVerdict("pass"), mkVerdict("pass"), mkVerdict("pass")])!;
    expect(agg.verdict).toBe("pass");
    expect(agg.unanimous).toBe(true);
    expect(agg.minorityFail).toBe(false);
    expect(agg.votes.pass).toBe(3);
  });

  it("single fail among passes → fail (minorityFail=true)", () => {
    const agg = aggregateReviewVerdicts([mkVerdict("pass"), mkVerdict("fail"), mkVerdict("pass")])!;
    expect(agg.verdict).toBe("fail");
    expect(agg.unanimous).toBe(false);
    expect(agg.minorityFail).toBe(true);
    expect(agg.votes.fail).toBe(1);
  });

  it("mix of pass/conditional (no fail) → conditional", () => {
    const agg = aggregateReviewVerdicts([mkVerdict("pass"), mkVerdict("conditional")])!;
    expect(agg.verdict).toBe("conditional");
    expect(agg.unanimous).toBe(false);
    expect(agg.minorityFail).toBe(false);
  });

  it("unanimous fail → fail, unanimous=true, minorityFail=false", () => {
    const agg = aggregateReviewVerdicts([mkVerdict("fail"), mkVerdict("fail")])!;
    expect(agg.verdict).toBe("fail");
    expect(agg.unanimous).toBe(true);
    expect(agg.minorityFail).toBe(false);
  });

  describe("rubric stats", () => {
    it("is null when no reviewer provided a rubric", () => {
      const agg = aggregateReviewVerdicts([mkVerdict("pass"), mkVerdict("conditional")])!;
      expect(agg.rubric).toBeNull();
    });

    it("computes mean/min/max correctly", () => {
      const agg = aggregateReviewVerdicts([
        mkVerdict("pass", { rubric: { codeQuality: 5 } }),
        mkVerdict("pass", { rubric: { codeQuality: 3 } }),
        mkVerdict("pass", { rubric: { codeQuality: 4 } }),
      ])!;
      expect(agg.rubric).not.toBeNull();
      expect(agg.rubric!.codeQuality.mean).toBeCloseTo(4, 5);
      expect(agg.rubric!.codeQuality.min).toBe(3);
      expect(agg.rubric!.codeQuality.max).toBe(5);
      expect(agg.rubric!.codeQuality.count).toBe(3);
      expect(agg.rubric!.codeQuality.stddev).toBeGreaterThan(0);
    });

    it("stddev is 0 when only one reviewer has rubric", () => {
      const agg = aggregateReviewVerdicts([
        mkVerdict("pass", { rubric: { codeQuality: 5 } }),
        mkVerdict("pass"), // no rubric
      ])!;
      expect(agg.rubric!.codeQuality.count).toBe(1);
      expect(agg.rubric!.codeQuality.stddev).toBe(0);
    });

    it("maxStddev picks the largest dimension spread", () => {
      const agg = aggregateReviewVerdicts([
        mkVerdict("pass", { rubric: { codeQuality: 1, testCoverage: 5 } }),
        mkVerdict("pass", { rubric: { codeQuality: 5, testCoverage: 5 } }),
      ])!;
      // codeQuality spread 1↔5 >> testCoverage spread 5↔5
      expect(agg.rubric!.codeQuality.stddev).toBeGreaterThan(agg.rubric!.testCoverage.stddev);
      expect(agg.rubric!.maxStddev).toBe(agg.rubric!.codeQuality.stddev);
    });
  });

  describe("findings / recommendations / failedSubtaskIds", () => {
    it("dedupes findings by lowercased prefix", () => {
      // Shared ≥80-char prefix (lowercased) → should dedupe.
      const longFinding = "src/api.ts:10 missing parameter validation on user_id — enables SQL injection attack";
      const agg = aggregateReviewVerdicts([
        mkVerdict("fail", { findings: [longFinding, "new issue"] }),
        mkVerdict("fail", { findings: [longFinding.toUpperCase() + " — confirmed by reviewer B"] }),
      ])!;
      // longFinding + upper variant share 80-char prefix → collapse to 1
      expect(agg.findings).toHaveLength(2);
    });

    it("skips empty strings", () => {
      const agg = aggregateReviewVerdicts([
        mkVerdict("pass", { findings: ["", "   ", "real finding"] }),
      ])!;
      expect(agg.findings).toEqual(["real finding"]);
    });

    it("unions + sorts failedSubtaskIds", () => {
      const agg = aggregateReviewVerdicts([
        mkVerdict("fail", { failedSubtaskIds: [3, 1] }),
        mkVerdict("fail", { failedSubtaskIds: [2, 1] }),
      ])!;
      expect(agg.failedSubtaskIds).toEqual([1, 2, 3]);
    });
  });
});

describe("formatTallyForSynthesizer", () => {
  it("renders vote counts + consensus", () => {
    const agg = aggregateReviewVerdicts([mkVerdict("pass"), mkVerdict("fail")])!;
    const formatted = formatTallyForSynthesizer(agg);
    expect(formatted).toContain("pass: 1");
    expect(formatted).toContain("fail: 1");
    expect(formatted).toContain("aggregate verdict: **fail**");
    expect(formatted).toContain("minority fail");
  });

  it("includes rubric mean + max stddev when rubrics present", () => {
    const agg = aggregateReviewVerdicts([
      mkVerdict("pass", { rubric: { codeQuality: 5 } }),
      mkVerdict("pass", { rubric: { codeQuality: 3 } }),
    ])!;
    const formatted = formatTallyForSynthesizer(agg);
    expect(formatted).toContain("rubric mean");
    expect(formatted).toContain("max disagreement");
  });

  it("omits findings section when empty", () => {
    const agg = aggregateReviewVerdicts([mkVerdict("pass")])!;
    const formatted = formatTallyForSynthesizer(agg);
    expect(formatted).not.toContain("Merged Findings");
  });

  it("lists failed subtask ids when present", () => {
    const agg = aggregateReviewVerdicts([mkVerdict("fail", { failedSubtaskIds: [4, 7] })])!;
    const formatted = formatTallyForSynthesizer(agg);
    expect(formatted).toContain("Failed subtask ids: 4, 7");
  });
});
