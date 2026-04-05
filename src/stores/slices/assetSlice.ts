import { invoke } from "@tauri-apps/api/core";
import { errorMessage } from "@/lib/utils";
import { getSetting, setSetting } from "@/lib/appStore";
import { expandSkillRefs } from "@/lib/skillSets";
import { mapKeywordsToSkills, matchPromptToSkills } from "@/lib/skillMappings";
import type {
  SetState,
  GetState,
  Memo,
  Artifact,
  SkillDef,
  AgentProfile,
  CreateMemoInput,
  CreateArtifactInput,
  UpdateArtifactStatusInput,
} from "./types";

const DEFAULT_PROFILES: AgentProfile[] = [
  { id: "architect-claude", label: "Architect Claude", engine: "claude", defaultSkills: [] },
  { id: "reviewer-codex", label: "Reviewer Codex", engine: "codex", defaultSkills: [] },
  { id: "tester-gemini", label: "Tester Gemini", engine: "gemini", defaultSkills: [] },
  { id: "general-opencode", label: "General OpenCode", engine: "opencode", defaultSkills: [] },
];

/** Per-conversation engine/profile snapshot */
export interface ConversationEngineState {
  profileId: string | null;
  engine: string;
  model?: string;
}

export interface AssetSlice {
  memos: Memo[];
  artifacts: Artifact[];
  skills: SkillDef[];
  activeSkills: string[];
  /** Phase→skills mapping for automatic workflow skill injection */
  workflowSkills: Record<string, string[]>;
  crossSessionIds: string[];
  handoffSource: { type: string; content: string } | null;
  scrollToMessageId: string | null;
  personaFragment: string | null;
  personaLabel: string | null;
  // Agent profiles — shared between Settings and NewMessageInput
  agentProfiles: AgentProfile[];
  /** Per-conversation engine/profile memory (SSOT for profile selection) */
  _convEngineMap: Record<string, ConversationEngineState>;
  loadProfiles: () => Promise<void>;
  saveProfiles: (profiles: AgentProfile[]) => void;
  /** Save current engine/profile state for a conversation */
  saveConversationEngine: (conversationId: string, state: ConversationEngineState) => void;
  /** Restore engine/profile state for a conversation. Returns null if none saved. */
  getConversationEngine: (conversationId: string) => ConversationEngineState | null;
  setHandoffSource: (source: { type: string; content: string } | null) => void;
  loadMemos: () => Promise<void>;
  createMemo: (messageId: string, content: string) => Promise<void>;
  deleteMemo: (id: string) => Promise<void>;
  loadArtifacts: () => Promise<void>;
  createArtifact: (input: CreateArtifactInput) => Promise<void>;
  updateArtifactStatus: (id: string, status: "draft" | "approved" | "rejected") => Promise<void>;
  deleteArtifact: (id: string) => Promise<void>;
  loadSkills: () => Promise<void>;
  toggleSkill: (name: string) => void;
  /** Load workflow skill mappings from appStore */
  loadWorkflowSkills: () => Promise<void>;
  /** Save workflow skill mappings */
  saveWorkflowSkills: (config: Record<string, string[]>) => void;
  /** Get effective skills: manual activeSkills ∪ phase-based workflow skills */
  getEffectiveSkills: (planPhase: string | null) => string[];
  /** Detect project tech stack and recommend matching skills */
  detectAndRecommendSkills: () => Promise<void>;
  /** Accept recommended skills — bulk-set and persist per project */
  acceptRecommendedSkills: (skillNames: string[]) => void;
  /** Dismiss recommendation banner without applying */
  dismissRecommendation: () => void;
  /** Recommended skills from detection (null = hidden) */
  recommendedSkills: string[] | null;
  toggleCrossSession: (conversationId: string) => void;
}

export const createAssetSlice = (set: SetState, get: GetState): AssetSlice => ({
  memos: [],
  artifacts: [],
  skills: [],
  activeSkills: [],
  workflowSkills: {},
  crossSessionIds: [],
  handoffSource: null,
  scrollToMessageId: null,
  personaFragment: null,
  personaLabel: null,
  agentProfiles: [],
  _convEngineMap: {},
  recommendedSkills: null,

  loadProfiles: async () => {
    const profiles = await getSetting<AgentProfile[]>("agentProfiles", DEFAULT_PROFILES);
    const convMap = await getSetting<Record<string, ConversationEngineState>>("convEngineMap", {});
    // Backfill: if any conversation has a profile but no model, fill from profile default
    let updated = false;
    for (const [convId, state] of Object.entries(convMap)) {
      if (state.profileId && !state.model) {
        const profile = profiles.find((p) => p.id === state.profileId);
        if (profile?.model) {
          convMap[convId] = { ...state, model: profile.model };
          updated = true;
        }
      }
    }
    if (updated) setSetting("convEngineMap", convMap);
    set({ agentProfiles: profiles, _convEngineMap: convMap });
  },

  saveProfiles: (profiles: AgentProfile[]) => {
    set({ agentProfiles: profiles });
    setSetting("agentProfiles", profiles);
  },

  saveConversationEngine: (conversationId: string, state: ConversationEngineState) => {
    set((prev) => {
      const map = { ...prev._convEngineMap, [conversationId]: state };
      setSetting("convEngineMap", map);
      return { _convEngineMap: map };
    });
  },

  getConversationEngine: (conversationId: string) => {
    return get()._convEngineMap[conversationId] ?? null;
  },

  setHandoffSource: (source) => set({ handoffSource: source }),

  // ─── Skill detection & recommendation ────────────────────────────────────
  detectAndRecommendSkills: async () => {
    const pk = get().selectedProjectKey;
    if (!pk) return;
    // Skip if project already has saved skills
    const storeKey = `activeSkills:${pk}`;
    const existing = await getSetting<string[] | null>(storeKey, null);
    if (existing !== null) {
      set({ recommendedSkills: null });
      return;
    }
    // Skip if user already dismissed for this project
    const dismissed = await getSetting<boolean>(`skillDetectionDismissed:${pk}`, false);
    if (dismissed) {
      set({ recommendedSkills: null });
      return;
    }
    try {
      const project = await invoke<{ path?: string }>("get_project", { key: pk });
      if (!project.path) { set({ recommendedSkills: null }); return; }
      // Guard against stale result from project switch
      if (get().selectedProjectKey !== pk) return;
      const result = await invoke<{ keywords: string[]; detectedFiles: string[] }>("detect_project_stack", { projectPath: project.path });
      if (get().selectedProjectKey !== pk) return;
      if (result.keywords.length === 0) { set({ recommendedSkills: null }); return; }
      const recommended = mapKeywordsToSkills(result.keywords);
      // Filter to only skills that are actually installed
      const installed = new Set(get().skills.map((s) => s.name));
      const valid = recommended.filter((s) => installed.has(s));
      set({ recommendedSkills: valid.length > 0 ? valid : null });
    } catch {
      set({ recommendedSkills: null });
    }
  },

  acceptRecommendedSkills: (skillNames: string[]) => {
    const pk = get().selectedProjectKey;
    const storeKey = pk ? `activeSkills:${pk}` : "lastActiveSkills";
    set({ activeSkills: skillNames, recommendedSkills: null });
    setSetting(storeKey, skillNames);
  },

  dismissRecommendation: () => {
    const pk = get().selectedProjectKey;
    set({ recommendedSkills: null });
    if (pk) {
      setSetting(`skillDetectionDismissed:${pk}`, true);
    }
  },

  // ─── Cross-session ───────────────────────────────────────────────────────
  toggleCrossSession: (conversationId: string) => {
    set((state) => {
      const ids = state.crossSessionIds.includes(conversationId)
        ? state.crossSessionIds.filter((id) => id !== conversationId)
        : [...state.crossSessionIds, conversationId];
      return { crossSessionIds: ids };
    });
  },

  // ─── Skill ───────────────────────────────────────────────────────────────
  loadSkills: async () => {
    try {
      const skills = await invoke<SkillDef[]>("list_skills");
      const pk = get().selectedProjectKey;
      const storeKey = pk ? `activeSkills:${pk}` : "lastActiveSkills";
      let saved = await getSetting<string[] | null>(storeKey, null);
      // Migration: fall back to global if no project-specific skills saved yet
      let migrated = false;
      if (saved === null && pk) {
        saved = await getSetting<string[]>("lastActiveSkills", []);
        migrated = true;
      }
      const validNames = new Set(skills.map((s) => s.name));
      const restored = (saved ?? []).filter((n) => validNames.has(n));
      set({ skills, activeSkills: restored });
      // Persist migration so detectAndRecommendSkills sees existing skills
      if (migrated && pk && restored.length > 0) {
        setSetting(`activeSkills:${pk}`, restored);
      }
    } catch (e) {
      set({ error: errorMessage(e) });
    }
  },

  loadWorkflowSkills: async () => {
    const pk = get().selectedProjectKey;
    const key = pk ? `workflowSkills:${pk}` : "workflowSkills";
    const config = await getSetting<Record<string, string[]>>(key, {});
    set({ workflowSkills: config });
  },

  saveWorkflowSkills: (config: Record<string, string[]>) => {
    const pk = get().selectedProjectKey;
    const key = pk ? `workflowSkills:${pk}` : "workflowSkills";
    set({ workflowSkills: config });
    setSetting(key, config);
  },

  getEffectiveSkills: (planPhase: string | null, prompt?: string) => {
    const { activeSkills, workflowSkills } = get();
    const phase = planPhase ?? "chat";
    const phaseKey =
      phase === "drafting" || phase === "subtask_review" || phase === "approval"
        ? "chat"
        : phase === "rework"
          ? "implementation"
          : phase;
    const phaseRefs = workflowSkills[phaseKey] ?? [];
    // Expand set: refs and individual skills, then union with manual activeSkills
    const expanded = expandSkillRefs([...activeSkills, ...phaseRefs]);
    // Dynamic prompt-based skill injection (feature C)
    if (prompt) {
      const dynamicSkills = matchPromptToSkills(prompt, expanded);
      for (const s of dynamicSkills) expanded.push(s);
    }
    return [...new Set(expanded)];
  },

  toggleSkill: (name: string) => {
    const pk = get().selectedProjectKey;
    set((state) => {
      const active = state.activeSkills.includes(name)
        ? state.activeSkills.filter((s) => s !== name)
        : [...state.activeSkills, name];
      setSetting(pk ? `activeSkills:${pk}` : "lastActiveSkills", active);
      return { activeSkills: active };
    });
  },

  // ─── Memo ────────────────────────────────────────────────────────────────
  loadMemos: async () => {
    const { selectedConversationId } = get();
    if (!selectedConversationId) return;
    try {
      const memos = await invoke<Memo[]>("list_memos_by_conversation", {
        conversationId: selectedConversationId,
      });
      set({ memos });
    } catch (e) {
      set({ error: errorMessage(e) });
    }
  },

  createMemo: async (messageId: string, content: string) => {
    const { selectedProjectKey, selectedConversationId } = get();
    if (!selectedProjectKey || !selectedConversationId) return;
    try {
      const input: CreateMemoInput = {
        messageId,
        conversationId: selectedConversationId,
        projectKey: selectedProjectKey,
        content,
      };
      await invoke<Memo>("create_memo", { input });
      await get().loadMemos();
    } catch (e) {
      set({ error: errorMessage(e) });
    }
  },

  deleteMemo: async (id: string) => {
    try {
      await invoke("delete_memo", { id });
      await get().loadMemos();
    } catch (e) {
      set({ error: errorMessage(e) });
    }
  },

  // ─── Artifact ────────────────────────────────────────────────────────────
  loadArtifacts: async () => {
    const { selectedConversationId } = get();
    if (!selectedConversationId) return;
    try {
      const artifacts = await invoke<Artifact[]>("list_artifacts", {
        conversationId: selectedConversationId,
      });
      set({ artifacts });
    } catch (e) {
      set({ error: errorMessage(e) });
    }
  },

  createArtifact: async (input: CreateArtifactInput) => {
    try {
      await invoke<Artifact>("create_artifact", { input });
      await get().loadArtifacts();
    } catch (e) {
      set({ error: errorMessage(e) });
    }
  },

  updateArtifactStatus: async (id: string, status: "draft" | "approved" | "rejected") => {
    try {
      const input: UpdateArtifactStatusInput = { id, status };
      await invoke("update_artifact_status", { input });
      await get().loadArtifacts();
    } catch (e) {
      set({ error: errorMessage(e) });
    }
  },

  deleteArtifact: async (id: string) => {
    try {
      await invoke("delete_artifact", { id });
      await get().loadArtifacts();
    } catch (e) {
      set({ error: errorMessage(e) });
    }
  },
});
