import { describe, it, expect } from "vitest";
import { splitPlanProposals, hasPlanProposal } from "@/lib/planProposalParser";

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
