import { invoke } from "@tauri-apps/api/core";
import { getSetting, setSetting } from "@/lib/appStore";
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

export interface AssetSlice {
  memos: Memo[];
  artifacts: Artifact[];
  skills: SkillDef[];
  activeSkills: string[];
  crossSessionIds: string[];
  handoffSource: { type: string; content: string } | null;
  scrollToMessageId: string | null;
  personaFragment: string | null;
  personaLabel: string | null;
  // Agent profiles — shared between Settings and NewMessageInput
  agentProfiles: AgentProfile[];
  selectedProfileId: string | null;
  loadProfiles: () => Promise<void>;
  saveProfiles: (profiles: AgentProfile[]) => void;
  selectProfile: (profileId: string | null) => void;
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
  toggleCrossSession: (conversationId: string) => void;
}

export const createAssetSlice = (set: SetState, get: GetState): AssetSlice => ({
  memos: [],
  artifacts: [],
  skills: [],
  activeSkills: [],
  crossSessionIds: [],
  handoffSource: null,
  scrollToMessageId: null,
  personaFragment: null,
  personaLabel: null,
  agentProfiles: [],
  selectedProfileId: null,

  loadProfiles: async () => {
    const profiles = await getSetting<AgentProfile[]>("agentProfiles", DEFAULT_PROFILES);
    const lastId = await getSetting<string | null>("lastProfileId", null);
    const selectedId = lastId && profiles.some((p) => p.id === lastId) ? lastId : profiles[0]?.id ?? null;
    set({ agentProfiles: profiles, selectedProfileId: selectedId });
  },

  saveProfiles: (profiles: AgentProfile[]) => {
    set({ agentProfiles: profiles });
    setSetting("agentProfiles", profiles);
  },

  selectProfile: (profileId: string | null) => {
    set({ selectedProfileId: profileId });
    setSetting("lastProfileId", profileId);
  },

  setHandoffSource: (source) => set({ handoffSource: source }),

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
      const saved = await getSetting<string[]>("lastActiveSkills", []);
      // Restore only skills that still exist in the snapshot
      const validNames = new Set(skills.map((s) => s.name));
      const restored = saved.filter((n) => validNames.has(n));
      set({ skills, activeSkills: restored });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  toggleSkill: (name: string) => {
    set((state) => {
      const active = state.activeSkills.includes(name)
        ? state.activeSkills.filter((s) => s !== name)
        : [...state.activeSkills, name];
      setSetting("lastActiveSkills", active);
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
      set({ error: String(e) });
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
      set({ error: String(e) });
    }
  },

  deleteMemo: async (id: string) => {
    try {
      await invoke("delete_memo", { id });
      await get().loadMemos();
    } catch (e) {
      set({ error: String(e) });
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
      set({ error: String(e) });
    }
  },

  createArtifact: async (input: CreateArtifactInput) => {
    try {
      await invoke<Artifact>("create_artifact", { input });
      await get().loadArtifacts();
    } catch (e) {
      set({ error: String(e) });
    }
  },

  updateArtifactStatus: async (id: string, status: "draft" | "approved" | "rejected") => {
    try {
      const input: UpdateArtifactStatusInput = { id, status };
      await invoke("update_artifact_status", { input });
      await get().loadArtifacts();
    } catch (e) {
      set({ error: String(e) });
    }
  },

  deleteArtifact: async (id: string) => {
    try {
      await invoke("delete_artifact", { id });
      await get().loadArtifacts();
    } catch (e) {
      set({ error: String(e) });
    }
  },
});
