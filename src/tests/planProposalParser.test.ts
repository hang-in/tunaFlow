import { describe, it, expect } from "vitest";
import {
  splitPlanProposals, hasPlanProposal,
  hasImplPlan, extractImplPlan,
  hasImplComplete,
  hasReviewVerdict, extractReviewVerdict,
  extractCompletedSubtasks, scanCompletedSubtasks,
} from "@/lib/planProposalParser";

const SAMPLE = `Here is a plan:

<!-- tunaflow:plan-proposal -->
## Plan Proposal: API Refactoring

### Description
Convert REST API to GraphQL.

### Expected Outcome
All endpoints migrated to GraphQL schema.

### Subtasks
1. Define schema — Create GraphQL type definitions
2. Implement resolvers
3. Migrate endpoints — Update frontend calls

### Constraints
- No breaking changes to existing clients
- Maintain backward compatibility

### Non-goals
- Mobile app changes
<!-- /tunaflow:plan-proposal -->

Let me know if you'd like changes.`;

describe("planProposalParser", () => {
  it("detects plan proposal markers", () => {
    expect(hasPlanProposal(SAMPLE)).toBe(true);
    expect(hasPlanProposal("just some text")).toBe(false);
  });

  it("splits content into segments", () => {
    const segments = splitPlanProposals(SAMPLE);
    expect(segments).toHaveLength(3);
    expect(segments[0].type).toBe("markdown");
    expect(segments[1].type).toBe("plan-proposal");
    expect(segments[2].type).toBe("markdown");
  });

  it("parses proposal fields", () => {
    const segments = splitPlanProposals(SAMPLE);
    const seg = segments[1];
    if (seg.type !== "plan-proposal") throw new Error("expected plan-proposal");
    const p = seg.proposal;

    expect(p.title).toBe("API Refactoring");
    expect(p.description).toContain("GraphQL");
    expect(p.expectedOutcome).toContain("migrated");
    expect(p.subtasks).toHaveLength(3);
    expect(p.subtasks[0].title).toBe("Define schema");
    expect(p.subtasks[0].details).toBe("Create GraphQL type definitions");
    expect(p.constraints).toHaveLength(2);
    expect(p.nonGoals).toHaveLength(1);
  });

  it("parses bold subtask titles", () => {
    const content = `<!-- tunaflow:plan-proposal -->
## Plan Proposal: Test

### Subtasks
1. **Bold Title** — Some details
2. **Another Bold** — More details
<!-- /tunaflow:plan-proposal -->`;
    const segments = splitPlanProposals(content);
    const seg = segments.find((s) => s.type === "plan-proposal");
    if (seg?.type !== "plan-proposal") throw new Error("expected plan-proposal");
    expect(seg.proposal.subtasks).toHaveLength(2);
    expect(seg.proposal.subtasks[0].title).toBe("Bold Title");
    expect(seg.proposal.subtasks[0].details).toBe("Some details");
  });

  it("returns single segment for plain content", () => {
    const segments = splitPlanProposals("just markdown");
    expect(segments).toHaveLength(1);
    expect(segments[0].type).toBe("markdown");
  });

  it("handles unclosed marker gracefully", () => {
    const content = "before\n<!-- tunaflow:plan-proposal -->\nsome stuff";
    const segments = splitPlanProposals(content);
    expect(segments).toHaveLength(2);
    expect(segments[0].type).toBe("markdown");
    expect(segments[1].type).toBe("markdown");
  });
});

describe("implPlanParser", () => {
  const IMPL = `
<!-- tunaflow:impl-plan -->
### Files to modify
- \`src/api/rest.ts\` → remove
- \`src/api/graphql.ts\` → create

### Dependencies
- Add graphql package

### Risks
- Breaking change to clients
<!-- /tunaflow:impl-plan -->`;

  it("detects impl-plan marker", () => {
    expect(hasImplPlan(IMPL)).toBe(true);
    expect(hasImplPlan("no marker")).toBe(false);
  });

  it("extracts impl plan data", () => {
    const plan = extractImplPlan(IMPL);
    expect(plan).not.toBeNull();
    expect(plan!.files).toHaveLength(2);
    expect(plan!.files[0].path).toBe("src/api/rest.ts");
    expect(plan!.files[0].action).toBe("remove");
    expect(plan!.dependencies).toHaveLength(1);
    expect(plan!.risks).toHaveLength(1);
  });
});

describe("implCompleteParser", () => {
  it("detects impl-complete marker", () => {
    expect(hasImplComplete("done <!-- tunaflow:impl-complete --> yay")).toBe(true);
    expect(hasImplComplete("no marker")).toBe(false);
  });
});

describe("subtaskDoneParser", () => {
  it("extracts completed subtask numbers", () => {
    const content = "Done step 1 <!-- tunaflow:subtask-done:1 --> and step 3 <!-- tunaflow:subtask-done:3 -->";
    expect(extractCompletedSubtasks(content)).toEqual([1, 3]);
  });

  it("returns empty for no markers", () => {
    expect(extractCompletedSubtasks("no markers here")).toEqual([]);
  });

  it("scans multiple messages", () => {
    const msgs = [
      { role: "assistant", content: "<!-- tunaflow:subtask-done:1 -->" },
      { role: "user", content: "ok" },
      { role: "assistant", content: "<!-- tunaflow:subtask-done:2 --> <!-- tunaflow:subtask-done:3 -->" },
    ];
    const done = scanCompletedSubtasks(msgs);
    expect(done).toEqual(new Set([1, 2, 3]));
  });
});

describe("reviewVerdictParser", () => {
  const REVIEW = `
<!-- tunaflow:review-verdict -->
verdict: pass
findings:
- All subtasks implemented
- Tests pass
recommendations:
- Add integration test
<!-- /tunaflow:review-verdict -->`;

  it("detects review-verdict marker", () => {
    expect(hasReviewVerdict(REVIEW)).toBe(true);
    expect(hasReviewVerdict("no marker")).toBe(false);
  });

  it("extracts verdict data", () => {
    const v = extractReviewVerdict(REVIEW);
    expect(v).not.toBeNull();
    expect(v!.verdict).toBe("pass");
    expect(v!.findings).toHaveLength(2);
    expect(v!.recommendations).toHaveLength(1);
  });

  it("defaults to conditional for unknown verdict", () => {
    const v = extractReviewVerdict("<!-- tunaflow:review-verdict -->\nsome text\n<!-- /tunaflow:review-verdict -->");
    expect(v).not.toBeNull();
    expect(v!.verdict).toBe("conditional");
  });
});
