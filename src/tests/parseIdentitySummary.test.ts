import { describe, it, expect } from "vitest";
import {
  extractFrontmatter,
  splitBySections,
  parseInflectionPoints,
  parseDoAvoid,
  parseIdentitySummary,
} from "@/lib/parseIdentitySummary";

describe("extractFrontmatter", () => {
  it("parses full frontmatter block", () => {
    const content =
      "---\nproject_key: proj-x\nperiod_start: 100\nperiod_end: 200\nartifact_refs: [a1,a2]\nsupersedes: prev\n---\n\n### Project identity\nbody";
    const [fm, body] = extractFrontmatter(content);
    expect(fm).toEqual({
      projectKey: "proj-x",
      periodStart: 100,
      periodEnd: 200,
      artifactRefs: ["a1", "a2"],
      supersedes: "prev",
    });
    expect(body.startsWith("### Project identity")).toBe(true);
  });

  it("returns null when no frontmatter", () => {
    const plain = "### Project identity\nbody";
    const [fm, body] = extractFrontmatter(plain);
    expect(fm).toBeNull();
    expect(body).toBe(plain);
  });

  it("empty artifact_refs returns empty array", () => {
    const content = "---\nproject_key: p\nperiod_start: 0\nperiod_end: 0\nartifact_refs: []\nsupersedes: \n---\n\nbody";
    const [fm] = extractFrontmatter(content);
    expect(fm?.artifactRefs).toEqual([]);
    expect(fm?.supersedes).toBeUndefined();
  });
});

describe("splitBySections", () => {
  it("splits body by each header in order", () => {
    const body =
      "### Project identity\nid-body\n\n### User working preference\nuser-body\n\n### Do / Avoid\nda-body";
    const sections = splitBySections(body, [
      "### Project identity",
      "### User working preference",
      "### Agent operating preference",
      "### Recent inflection points",
      "### Do / Avoid",
    ]);
    expect(sections[0]).toBe("id-body");
    expect(sections[1]).toBe("user-body");
    expect(sections[2]).toBe("");
    expect(sections[3]).toBe("");
    expect(sections[4]).toBe("da-body");
  });
});

describe("parseInflectionPoints", () => {
  it("parses three groups separated by blank lines", () => {
    const section = `
- What changed: engine switch
- Why: art-1
- When: 2026-04-20

- What changed: added worldview
- Why: art-2
- When: 2026-04-21

- What changed: metaAgent landed
- Why: art-3
- When: 2026-04-23
`;
    const pts = parseInflectionPoints(section);
    expect(pts).toHaveLength(3);
    expect(pts[0].what).toBe("engine switch");
    expect(pts[0].why).toBe("art-1");
    expect(pts[0].artifactId).toBe("art-1");
    expect(pts[2].when).toBe("2026-04-23");
  });

  it("returns empty when section blank", () => {
    expect(parseInflectionPoints("")).toEqual([]);
    expect(parseInflectionPoints("   ")).toEqual([]);
  });
});

describe("parseDoAvoid", () => {
  it("parses Do and Avoid lists from sub-headers", () => {
    const section = `
Do:
- prefer CLI
- plan before coding

Avoid:
- force-push to main
- skip review
`;
    const da = parseDoAvoid(section);
    expect(da.do).toEqual(["prefer CLI", "plan before coding"]);
    expect(da.avoid).toEqual(["force-push to main", "skip review"]);
  });

  it("empty section returns empty arrays", () => {
    expect(parseDoAvoid("")).toEqual({ do: [], avoid: [] });
  });
});

describe("parseIdentitySummary (integration)", () => {
  it("full document → frontmatter + 5 sections parsed", () => {
    const content = `---
project_key: proj-x
period_start: 100
period_end: 200
artifact_refs: [a1,a2,a3]
supersedes: prev-id
---

### Project identity
Primary summary line.
- Nature: CLI orchestration app
- Stage: beta-ready

### User working preference
- CLI-first preferred
- Architect→Developer pattern

### Agent operating preference
- Claude for implementation
- Codex for review

### Recent inflection points
- What changed: added worldview
- Why: art-a1
- When: 2026-04-23

### Do / Avoid
Do:
- keep PRs small

Avoid:
- force push
`;
    const parsed = parseIdentitySummary(content);
    expect(parsed.frontmatter?.projectKey).toBe("proj-x");
    expect(parsed.frontmatter?.artifactRefs).toHaveLength(3);
    expect(parsed.sections.projectIdentity).toContain("CLI orchestration");
    expect(parsed.sections.userWorkingPreference).toContain("CLI-first");
    expect(parsed.sections.agentOperatingPreference).toContain("Claude for implementation");
    expect(parsed.sections.inflectionPoints).toHaveLength(1);
    expect(parsed.sections.doAvoid.do).toEqual(["keep PRs small"]);
    expect(parsed.sections.doAvoid.avoid).toEqual(["force push"]);
  });

  it("missing sections return empty strings / arrays (best-effort)", () => {
    const content = `---
project_key: p
period_start: 0
period_end: 0
artifact_refs: []
supersedes:
---

### Project identity
Only this section exists.
`;
    const parsed = parseIdentitySummary(content);
    expect(parsed.sections.projectIdentity).toContain("Only this section");
    expect(parsed.sections.userWorkingPreference).toBe("");
    expect(parsed.sections.agentOperatingPreference).toBe("");
    expect(parsed.sections.inflectionPoints).toEqual([]);
    expect(parsed.sections.doAvoid).toEqual({ do: [], avoid: [] });
  });

  it("no frontmatter still parses sections", () => {
    const content =
      "### Project identity\nbody\n\n### Do / Avoid\nDo:\n- x\n\nAvoid:\n- y";
    const parsed = parseIdentitySummary(content);
    expect(parsed.frontmatter).toBeNull();
    expect(parsed.sections.projectIdentity).toBe("body");
    expect(parsed.sections.doAvoid.do).toEqual(["x"]);
    expect(parsed.sections.doAvoid.avoid).toEqual(["y"]);
  });
});
