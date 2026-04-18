/**
 * initialSetupApply — applies meta-agent's recommended initial setup to the
 * store/appSettings based on user selections.
 *
 * The meta-agent emits a JSON blob in `[INITIAL_SETUP_START/END]` during
 * onboarding. The FE shows its contents as checkboxes; this module translates
 * the checked items into actual side effects (agent profiles, activeSkills,
 * workflow defaults).
 *
 * Scope discipline (plan §7 안전 장치):
 *  - Unknown persona_ids fall back to persona_general.
 *  - Profiles with engines not in the known set are dropped.
 *  - Skills not present in the registry are filtered out by the caller.
 *  - Any individual failure is logged but does not abort the whole apply —
 *    remaining selections still land.
 */
import { getSetting, setSetting } from "@/lib/appStore";
import { DEFAULT_PERSONAS } from "@/lib/defaultPersonas";
import type { AgentProfile } from "@/types";

/** Known persona IDs. Keep in sync with `DEFAULT_PERSONAS` in defaultPersonas.ts. */
const KNOWN_PERSONAS = new Set(DEFAULT_PERSONAS.map((p) => p.id));

/** Engines we recognize. Matches ENGINE_CONFIGS keys. */
const KNOWN_ENGINES = new Set(["claude", "codex", "gemini", "ollama", "lmstudio", "openai"]);

export interface RecommendedProfile {
  role: string;        // "architect" | "developer" | "reviewer" | …
  engine: string;
  model?: string;
  persona_id?: string;
}

export interface RecommendedWorkflow {
  review_track?: "quick" | "deep" | string;
  context_mode?: "auto" | "lite" | "standard" | "full" | string;
  rt_participants?: string[];
}

export interface InitialSetupPayload {
  agent_profiles?: RecommendedProfile[];
  skills?: string[];
  workflow?: RecommendedWorkflow;
  rationale?: string;
}

export interface InitialSetupSelection {
  /** Indices into payload.agent_profiles that the user checked. */
  profileIndices: Set<number>;
  /** Skill names that the user checked. */
  skills: Set<string>;
  /** Whether to apply the workflow defaults. */
  applyWorkflow: boolean;
}

/**
 * Normalize an arbitrary value into a validated InitialSetupPayload or null
 * if the shape is unusable. Tolerant — drops unknown fields rather than
 * rejecting the whole payload.
 */
export function normalizeInitialSetup(raw: unknown): InitialSetupPayload | null {
  if (!raw || typeof raw !== "object") return null;
  const obj = raw as Record<string, unknown>;
  const out: InitialSetupPayload = {};

  if (Array.isArray(obj.agent_profiles)) {
    const profiles: RecommendedProfile[] = [];
    for (const item of obj.agent_profiles) {
      if (!item || typeof item !== "object") continue;
      const r = item as Record<string, unknown>;
      const engine = typeof r.engine === "string" ? r.engine : null;
      const role = typeof r.role === "string" ? r.role : null;
      if (!engine || !role) continue;
      profiles.push({
        role,
        engine,
        model: typeof r.model === "string" ? r.model : undefined,
        persona_id: typeof r.persona_id === "string" ? r.persona_id : undefined,
      });
    }
    if (profiles.length > 0) out.agent_profiles = profiles;
  }

  if (Array.isArray(obj.skills)) {
    const skills = obj.skills.filter((s): s is string => typeof s === "string" && s.length > 0);
    if (skills.length > 0) out.skills = skills;
  }

  if (obj.workflow && typeof obj.workflow === "object") {
    const w = obj.workflow as Record<string, unknown>;
    const workflow: RecommendedWorkflow = {};
    if (typeof w.review_track === "string") workflow.review_track = w.review_track;
    if (typeof w.context_mode === "string") workflow.context_mode = w.context_mode;
    if (Array.isArray(w.rt_participants)) {
      workflow.rt_participants = w.rt_participants.filter((x): x is string => typeof x === "string");
    }
    if (Object.keys(workflow).length > 0) out.workflow = workflow;
  }

  if (typeof obj.rationale === "string") out.rationale = obj.rationale;

  return Object.keys(out).length > 0 ? out : null;
}

/** Convert a recommended profile into an AgentProfile entry for the store. */
export function toAgentProfile(rec: RecommendedProfile, index: number): AgentProfile | null {
  if (!KNOWN_ENGINES.has(rec.engine)) return null;
  const personaId = rec.persona_id && KNOWN_PERSONAS.has(rec.persona_id)
    ? rec.persona_id
    : "persona_general";
  const roleLabel = rec.role.charAt(0).toUpperCase() + rec.role.slice(1);
  const engineLabel = rec.engine.charAt(0).toUpperCase() + rec.engine.slice(1);
  return {
    id: `onboarding-${rec.role}-${rec.engine}-${index}-${Date.now()}`,
    label: `${roleLabel} ${engineLabel}`,
    engine: rec.engine,
    model: rec.model,
    defaultSkills: [],
    personaId,
  };
}

/**
 * Apply the selected initial-setup items. Operates only on what the caller
 * selected. Returns a summary of what actually landed so the UI can show it.
 */
export async function applyInitialSetup(
  payload: InitialSetupPayload,
  selection: InitialSetupSelection,
  deps: {
    currentProfiles: AgentProfile[];
    saveProfiles: (profiles: AgentProfile[]) => void;
    currentActiveSkills: string[];
    setActiveSkills: (names: string[]) => void;
    availableSkillNames: Set<string>;
  },
): Promise<{ profilesAdded: number; skillsActivated: number; workflowApplied: boolean }> {
  let profilesAdded = 0;
  let skillsActivated = 0;
  let workflowApplied = false;

  // 1) Agent profiles — append (don't replace) so existing profiles stay.
  if (payload.agent_profiles && selection.profileIndices.size > 0) {
    const toAdd: AgentProfile[] = [];
    payload.agent_profiles.forEach((rec, i) => {
      if (!selection.profileIndices.has(i)) return;
      const profile = toAgentProfile(rec, i);
      if (profile) toAdd.push(profile);
    });
    if (toAdd.length > 0) {
      deps.saveProfiles([...deps.currentProfiles, ...toAdd]);
      profilesAdded = toAdd.length;
    }
  }

  // 2) Skills — union with currently active, only names present in registry.
  if (selection.skills.size > 0) {
    const toActivate = [...selection.skills].filter((n) => deps.availableSkillNames.has(n));
    if (toActivate.length > 0) {
      const merged = Array.from(new Set([...deps.currentActiveSkills, ...toActivate]));
      deps.setActiveSkills(merged);
      skillsActivated = toActivate.length;
    }
  }

  // 3) Workflow defaults — persist via appStore (global for now; per-project
  //    plan §10 open question, deferred).
  if (selection.applyWorkflow && payload.workflow) {
    const w = payload.workflow;
    if (w.context_mode && ["auto", "lite", "standard", "full"].includes(w.context_mode)) {
      const current = await getSetting<{ mode: string; totalCap: number }>(
        "contextBudgetConfig",
        { mode: "auto", totalCap: 60000 },
      );
      setSetting("contextBudgetConfig", { ...current, mode: w.context_mode });
    }
    // review_track + rt_participants — stored as hints for UI defaults.
    setSetting("onboardingWorkflowHints", {
      reviewTrack: w.review_track ?? null,
      rtParticipants: w.rt_participants ?? [],
    });
    workflowApplied = true;
  }

  return { profilesAdded, skillsActivated, workflowApplied };
}
