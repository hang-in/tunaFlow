import { describe, it, expect, vi, beforeEach } from "vitest";

vi.mock("@/lib/appStore", () => ({
  getSetting: vi.fn((_key: string, fallback: unknown) => Promise.resolve(fallback)),
  setSetting: vi.fn(),
}));

import { getSetting, setSetting } from "@/lib/appStore";
import {
  normalizeInitialSetup,
  applyInitialSetup,
  toAgentProfile,
  type InitialSetupPayload,
} from "@/lib/initialSetupApply";
import type { AgentProfile } from "@/types";

beforeEach(() => {
  vi.clearAllMocks();
  (getSetting as any).mockImplementation((_k: string, fallback: unknown) => Promise.resolve(fallback));
});

describe("normalizeInitialSetup", () => {
  it("returns null for non-object inputs", () => {
    expect(normalizeInitialSetup(null)).toBeNull();
    expect(normalizeInitialSetup("string")).toBeNull();
    expect(normalizeInitialSetup(42)).toBeNull();
  });

  it("keeps only well-formed agent_profiles", () => {
    const raw = {
      agent_profiles: [
        { role: "architect", engine: "claude", model: "claude-opus-4-6", persona_id: "persona_architect" },
        { role: "bad" }, // missing engine
        { engine: "codex" }, // missing role
      ],
    };
    const out = normalizeInitialSetup(raw);
    expect(out?.agent_profiles).toHaveLength(1);
    expect(out?.agent_profiles?.[0].engine).toBe("claude");
  });

  it("filters non-string skills and keeps string ones", () => {
    const raw = { skills: ["rust-review", 42, null, "cargo-test"] };
    const out = normalizeInitialSetup(raw);
    expect(out?.skills).toEqual(["rust-review", "cargo-test"]);
  });

  it("accepts partial workflow", () => {
    const raw = { workflow: { review_track: "deep", rt_participants: ["claude", "codex"] } };
    const out = normalizeInitialSetup(raw);
    expect(out?.workflow?.review_track).toBe("deep");
    expect(out?.workflow?.context_mode).toBeUndefined();
    expect(out?.workflow?.rt_participants).toEqual(["claude", "codex"]);
  });

  it("returns null when payload has no usable fields", () => {
    // all fields get filtered away
    expect(normalizeInitialSetup({ agent_profiles: [{ role: "x" }], skills: [123], workflow: {} })).toBeNull();
  });
});

describe("toAgentProfile", () => {
  it("drops profiles with unknown engines", () => {
    const result = toAgentProfile({ role: "developer", engine: "unknown-engine" }, 0);
    expect(result).toBeNull();
  });

  it("falls back to persona_general for unknown persona_id", () => {
    const result = toAgentProfile(
      { role: "developer", engine: "codex", persona_id: "persona_nonexistent" },
      0,
    );
    expect(result?.personaId).toBe("persona_general");
  });

  it("preserves known persona_id", () => {
    const result = toAgentProfile(
      { role: "architect", engine: "claude", persona_id: "persona_architect" },
      0,
    );
    expect(result?.personaId).toBe("persona_architect");
    expect(result?.engine).toBe("claude");
  });
});

describe("applyInitialSetup", () => {
  const payload: InitialSetupPayload = {
    agent_profiles: [
      { role: "architect", engine: "claude", model: "claude-opus-4-6", persona_id: "persona_architect" },
      { role: "developer", engine: "codex", model: "gpt-5-codex", persona_id: "persona_implementer" },
    ],
    skills: ["rust-review", "cargo-test"],
    workflow: { review_track: "deep", context_mode: "standard", rt_participants: ["claude", "codex"] },
  };

  it("appends selected profiles to existing ones", async () => {
    const existing: AgentProfile[] = [
      { id: "existing-1", label: "X", engine: "claude", defaultSkills: [] },
    ];
    const saveProfiles = vi.fn();
    const setActiveSkills = vi.fn();

    const result = await applyInitialSetup(
      payload,
      { profileIndices: new Set([0]), skills: new Set(), applyWorkflow: false },
      {
        currentProfiles: existing,
        saveProfiles,
        currentActiveSkills: [],
        setActiveSkills,
        availableSkillNames: new Set(),
      },
    );

    expect(saveProfiles).toHaveBeenCalledOnce();
    const saved = saveProfiles.mock.calls[0][0] as AgentProfile[];
    expect(saved).toHaveLength(2); // existing + 1 new
    expect(saved[0].id).toBe("existing-1");
    expect(saved[1].engine).toBe("claude");
    expect(result.profilesAdded).toBe(1);
    expect(result.skillsActivated).toBe(0);
    expect(result.workflowApplied).toBe(false);
  });

  it("only activates skills present in registry", async () => {
    const setActiveSkills = vi.fn();
    const result = await applyInitialSetup(
      payload,
      { profileIndices: new Set(), skills: new Set(["rust-review", "cargo-test"]), applyWorkflow: false },
      {
        currentProfiles: [],
        saveProfiles: vi.fn(),
        currentActiveSkills: ["existing-skill"],
        setActiveSkills,
        availableSkillNames: new Set(["rust-review"]), // cargo-test missing
      },
    );
    expect(setActiveSkills).toHaveBeenCalledOnce();
    const saved = setActiveSkills.mock.calls[0][0] as string[];
    expect(saved).toContain("existing-skill");
    expect(saved).toContain("rust-review");
    expect(saved).not.toContain("cargo-test");
    expect(result.skillsActivated).toBe(1);
  });

  it("persists workflow via appStore when selected", async () => {
    await applyInitialSetup(
      payload,
      { profileIndices: new Set(), skills: new Set(), applyWorkflow: true },
      {
        currentProfiles: [],
        saveProfiles: vi.fn(),
        currentActiveSkills: [],
        setActiveSkills: vi.fn(),
        availableSkillNames: new Set(),
      },
    );
    // contextBudgetConfig should have been set with mode: "standard"
    const ctxCall = (setSetting as any).mock.calls.find((c: unknown[]) => c[0] === "contextBudgetConfig");
    expect(ctxCall).toBeDefined();
    expect((ctxCall[1] as { mode: string }).mode).toBe("standard");
    // hints saved
    const hintsCall = (setSetting as any).mock.calls.find((c: unknown[]) => c[0] === "onboardingWorkflowHints");
    expect(hintsCall).toBeDefined();
    expect((hintsCall[1] as { reviewTrack: string }).reviewTrack).toBe("deep");
  });

  it("does nothing when selection is empty", async () => {
    const saveProfiles = vi.fn();
    const setActiveSkills = vi.fn();
    const result = await applyInitialSetup(
      payload,
      { profileIndices: new Set(), skills: new Set(), applyWorkflow: false },
      {
        currentProfiles: [],
        saveProfiles,
        currentActiveSkills: [],
        setActiveSkills,
        availableSkillNames: new Set(["rust-review"]),
      },
    );
    expect(saveProfiles).not.toHaveBeenCalled();
    expect(setActiveSkills).not.toHaveBeenCalled();
    expect(result.profilesAdded).toBe(0);
    expect(result.skillsActivated).toBe(0);
    expect(result.workflowApplied).toBe(false);
  });
});
