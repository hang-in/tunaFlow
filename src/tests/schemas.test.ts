import { describe, it, expect } from "vitest";
import {
  PlanProposalSchema,
  ImplPlanSchema,
  ReviewVerdictSchema,
  SubtaskDoneSchema,
  ImplCompleteSchema,
  toParsedPlanProposal,
  toParsedReviewVerdict,
} from "@/lib/schemas";

describe("PlanProposalSchema", () => {
  it("validates a complete proposal", () => {
    const input = {
      title: "Auth 리팩토링",
      description: "JWT 기반 인증 시스템 교체",
      expected_outcome: "세션 토큰 제거",
      subtasks: [
        { title: "기존 세션 분석", details: "DB 스키마 확인" },
        { title: "JWT 발급 구현" },
      ],
      constraints: ["하위 호환성 유지"],
      non_goals: ["OAuth 연동"],
    };
    const result = PlanProposalSchema.safeParse(input);
    expect(result.success).toBe(true);
  });

  it("rejects empty title", () => {
    const input = { title: "", description: "desc", subtasks: [{ title: "t1" }] };
    const result = PlanProposalSchema.safeParse(input);
    expect(result.success).toBe(false);
  });

  it("rejects empty subtasks array", () => {
    const input = { title: "Plan", description: "desc", subtasks: [] };
    const result = PlanProposalSchema.safeParse(input);
    expect(result.success).toBe(false);
  });

  it("defaults optional fields", () => {
    const input = { title: "Plan", description: "desc", subtasks: [{ title: "t1" }] };
    const result = PlanProposalSchema.parse(input);
    expect(result.constraints).toEqual([]);
    expect(result.non_goals).toEqual([]);
    expect(result.expected_outcome).toBe("");
  });

  it("toParsedPlanProposal converts snake_case to camelCase", () => {
    const input = PlanProposalSchema.parse({
      title: "Test",
      description: "Desc",
      subtasks: [{ title: "Sub1" }],
    });
    const parsed = toParsedPlanProposal(input, "raw markdown");
    expect(parsed.expectedOutcome).toBe("");
    expect(parsed.nonGoals).toEqual([]);
    expect(parsed.raw).toBe("raw markdown");
  });
});

describe("ImplPlanSchema", () => {
  it("validates with files, dependencies, risks", () => {
    const input = {
      files: [
        { path: "src/auth.ts", action: "create" },
        { path: "src/db.ts", action: "modify" },
      ],
      dependencies: ["jsonwebtoken"],
      risks: ["마이그레이션 실패 시 롤백 필요"],
    };
    const result = ImplPlanSchema.safeParse(input);
    expect(result.success).toBe(true);
  });

  it("defaults action to modify", () => {
    const input = { files: [{ path: "src/foo.ts" }] };
    const result = ImplPlanSchema.parse(input);
    expect(result.files[0].action).toBe("modify");
  });

  it("accepts empty object", () => {
    const result = ImplPlanSchema.parse({});
    expect(result.files).toEqual([]);
    expect(result.dependencies).toEqual([]);
  });
});

describe("ReviewVerdictSchema", () => {
  it("validates a pass verdict with rubric", () => {
    const input = {
      verdict: "pass",
      rubric: {
        plan_coverage: 5,
        code_quality: 4,
        test_coverage: 3,
        doc_quality: 4,
        convention: 5,
      },
      findings: [{ description: "잘 구현됨" }],
      recommendations: ["테스트 추가 권장"],
    };
    const result = ReviewVerdictSchema.safeParse(input);
    expect(result.success).toBe(true);
  });

  it("rejects invalid verdict value", () => {
    const input = { verdict: "maybe", findings: [] };
    const result = ReviewVerdictSchema.safeParse(input);
    expect(result.success).toBe(false);
  });

  it("rejects rubric score out of range", () => {
    const input = {
      verdict: "pass",
      rubric: {
        plan_coverage: 6,
        code_quality: 4,
        test_coverage: 3,
        doc_quality: 4,
        convention: 5,
      },
      findings: [],
    };
    const result = ReviewVerdictSchema.safeParse(input);
    expect(result.success).toBe(false);
  });

  it("accepts verdict without rubric", () => {
    const input = { verdict: "fail", findings: [{ description: "빌드 실패" }] };
    const result = ReviewVerdictSchema.safeParse(input);
    expect(result.success).toBe(true);
  });

  it("toParsedReviewVerdict formats findings with file/severity", () => {
    const input = ReviewVerdictSchema.parse({
      verdict: "conditional",
      findings: [
        { description: "버그 발견", file: "src/auth.ts", line: 42, severity: "critical" },
        { description: "경고" },
      ],
    });
    const parsed = toParsedReviewVerdict(input, "raw");
    expect(parsed.findings[0]).toContain("src/auth.ts:42");
    expect(parsed.findings[0]).toContain("(critical)");
    expect(parsed.findings[1]).toBe("경고");
  });
});

describe("SubtaskDoneSchema", () => {
  it("validates subtask number", () => {
    const result = SubtaskDoneSchema.safeParse({ subtask_number: 3, summary: "완료" });
    expect(result.success).toBe(true);
  });

  it("rejects zero or negative", () => {
    expect(SubtaskDoneSchema.safeParse({ subtask_number: 0 }).success).toBe(false);
    expect(SubtaskDoneSchema.safeParse({ subtask_number: -1 }).success).toBe(false);
  });
});

describe("ImplCompleteSchema", () => {
  it("validates with summary", () => {
    const result = ImplCompleteSchema.safeParse({ summary: "전체 구현 완료" });
    expect(result.success).toBe(true);
  });

  it("rejects empty summary", () => {
    const result = ImplCompleteSchema.safeParse({ summary: "" });
    expect(result.success).toBe(false);
  });
});
