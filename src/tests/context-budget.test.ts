import { describe, it, expect } from "vitest";

// Test context budget configuration validation (pure logic, no Tauri dependency)

describe("Context budget — mode validation", () => {
  const VALID_MODES = ["auto", "lite", "standard", "full"];

  it("all valid modes are recognized", () => {
    for (const mode of VALID_MODES) {
      expect(VALID_MODES).toContain(mode);
    }
  });

  it("auto mode returns undefined override", () => {
    const mode = "auto";
    const override = mode === "auto" ? undefined : mode;
    expect(override).toBeUndefined();
  });

  it("explicit modes return the mode string", () => {
    for (const mode of ["lite", "standard", "full"]) {
      const override = mode === "auto" ? undefined : mode;
      expect(override).toBe(mode);
    }
  });
});

describe("Context budget — cap validation", () => {
  const BUDGET_MIN = 20_000;
  const BUDGET_MAX = 120_000;
  const BUDGET_DEFAULT = 60_000;
  const BUDGET_STEP = 10_000;

  it("default cap returns undefined override", () => {
    const cap = BUDGET_DEFAULT;
    const override = cap === BUDGET_DEFAULT ? undefined : cap;
    expect(override).toBeUndefined();
  });

  it("custom cap returns the value", () => {
    const cap = 80_000;
    const override = cap === BUDGET_DEFAULT ? undefined : cap;
    expect(override).toBe(80_000);
  });

  it("valid caps are multiples of step within range", () => {
    for (let cap = BUDGET_MIN; cap <= BUDGET_MAX; cap += BUDGET_STEP) {
      expect(cap).toBeGreaterThanOrEqual(BUDGET_MIN);
      expect(cap).toBeLessThanOrEqual(BUDGET_MAX);
      expect(cap % BUDGET_STEP).toBe(0);
    }
  });

  it("step count is correct", () => {
    const steps = (BUDGET_MAX - BUDGET_MIN) / BUDGET_STEP;
    expect(steps).toBe(10);
  });
});

describe("Context budget — section policy", () => {
  const SECTION_POLICY: Record<string, string[]> = {
    lite: ["Project", "Context"],
    standard: ["Project", "Context", "Plan", "Findings", "Artifacts"],
    full: ["Project", "Context", "Plan", "Findings", "Artifacts", "Skills", "rawq", "Cross-session"],
  };

  it("lite includes only project and context", () => {
    expect(SECTION_POLICY.lite).toHaveLength(2);
    expect(SECTION_POLICY.lite).toContain("Project");
    expect(SECTION_POLICY.lite).toContain("Context");
  });

  it("standard is superset of lite", () => {
    for (const sec of SECTION_POLICY.lite) {
      expect(SECTION_POLICY.standard).toContain(sec);
    }
    expect(SECTION_POLICY.standard.length).toBeGreaterThan(SECTION_POLICY.lite.length);
  });

  it("full is superset of standard", () => {
    for (const sec of SECTION_POLICY.standard) {
      expect(SECTION_POLICY.full).toContain(sec);
    }
    expect(SECTION_POLICY.full.length).toBeGreaterThan(SECTION_POLICY.standard.length);
  });

  it("full includes rawq and skills", () => {
    expect(SECTION_POLICY.full).toContain("Skills");
    expect(SECTION_POLICY.full).toContain("rawq");
  });
});
