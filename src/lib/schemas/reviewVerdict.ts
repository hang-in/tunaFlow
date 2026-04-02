import { z } from "zod";

/**
 * Review Verdict schema — matches tool_handler.rs submit_review_verdict
 * and planProposalParser.ts ParsedReviewVerdict.
 */

const score = z.number().int().min(1).max(5);

export const ReviewRubricSchema = z.object({
  plan_coverage: score,
  code_quality: score,
  test_coverage: score,
  doc_quality: score,
  convention: score,
});

export const ReviewFindingSchema = z.object({
  description: z.string().min(1),
  file: z.string().optional(),
  line: z.number().int().optional(),
  severity: z.enum(["critical", "major", "minor"]).optional(),
});

export const ReviewVerdictSchema = z.object({
  verdict: z.enum(["pass", "fail", "conditional"]),
  rubric: ReviewRubricSchema.optional(),
  findings: z.array(ReviewFindingSchema).default([]),
  recommendations: z.array(z.string()).default([]),
});

export type ReviewVerdictInput = z.infer<typeof ReviewVerdictSchema>;

/** Convert zod-validated input to ParsedReviewVerdict shape */
export function toParsedReviewVerdict(
  input: ReviewVerdictInput,
  raw: string,
): {
  verdict: "pass" | "fail" | "conditional";
  rubric?: {
    planCoverage: number;
    codeQuality: number;
    testCoverage: number;
    docQuality: number;
    convention: number;
  };
  findings: string[];
  recommendations: string[];
  raw: string;
} {
  return {
    verdict: input.verdict,
    rubric: input.rubric
      ? {
          planCoverage: input.rubric.plan_coverage,
          codeQuality: input.rubric.code_quality,
          testCoverage: input.rubric.test_coverage,
          docQuality: input.rubric.doc_quality,
          convention: input.rubric.convention,
        }
      : undefined,
    findings: input.findings.map((f) => {
      const parts = [f.description];
      if (f.file) parts.push(`[${f.file}${f.line ? `:${f.line}` : ""}]`);
      if (f.severity) parts.push(`(${f.severity})`);
      return parts.join(" ");
    }),
    recommendations: input.recommendations,
    raw,
  };
}
