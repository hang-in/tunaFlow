import { invoke } from "@tauri-apps/api/core";
import { errorMessage } from "@/lib/utils";
import type { SetState, GetState, Project, CreateProjectInput } from "./types";
import { runProjectBootstrap } from "@/lib/bootstrap/project";

export interface OnboardingProject {
  key: string;
  path: string;
  name: string;
}

export interface ProjectSlice {
  projects: Project[];
  selectedProjectKey: string | null;
  projectLoading: string | null; // loading message or null
  onboardingProject: OnboardingProject | null;
  loadProjects: () => Promise<void>;
  createProject: (input: CreateProjectInput) => Promise<void>;
  hideProject: (key: string) => Promise<void>;
  selectProject: (key: string) => Promise<void>;
  clearOnboardingProject: () => void;
}

export const createProjectSlice = (set: SetState, get: GetState): ProjectSlice => ({
  projects: [],
  selectedProjectKey: null,
  projectLoading: null,
  onboardingProject: null,

  loadProjects: async () => {
    try {
      const projects = await invoke<Project[]>("list_projects");
      set({ projects, error: null });
    } catch (e) {
      set({ error: errorMessage(e) });
    }
  },

  createProject: async (input: CreateProjectInput) => {
    try {
      await invoke<Project>("create_project", { input });
      await get().loadProjects();
      // Trigger onboarding analysis if path is available
      if (input.path) {
        set({ onboardingProject: { key: input.key, path: input.path, name: input.name } });
      }
    } catch (e) {
      set({ error: errorMessage(e) });
    }
  },

  clearOnboardingProject: () => {
    set({ onboardingProject: null });
  },

  hideProject: async (key: string) => {
    try {
      await invoke("hide_project", { key });
      const { selectedProjectKey } = get();
      await get().loadProjects();
      // If hiding the currently selected project, clear selection. Each
      // slice owns the cleanup of its own fields (Finding 1-1). Order
      // matters only insofar as `conversations` must be cleared after
      // selection is dropped — handled inside resetConversationData.
      if (selectedProjectKey === key) {
        set({ selectedProjectKey: null, rawqStatus: null });
        get().resetConversationData();
        get().resetBranchState();
        get().clearConversationAssets();
        // Auto-select first remaining project
        const { projects, selectProject } = get();
        if (projects.length > 0) {
          await selectProject(projects[0].key);
        }
      }
    } catch (e) {
      set({ error: errorMessage(e) });
    }
  },

  selectProject: async (key: string) => {
    // Per-step bootstrap lives in `src/lib/bootstrap/project.ts` — see
    // Finding 1-2 in `docs/plans/refactorRoadmap_2026-04-20.md`. This
    // action now only assembles the callback surface that exposes
    // slice-owned state (set, selectConversation, agentProfiles, …) to
    // the lifecycle runner.
    await runProjectBootstrap(key, {
      setState: set,
      getSelectedProjectKey: () => get().selectedProjectKey,
      getSelectedConversationId: () => get().selectedConversationId,
      selectConversation: (id) => get().selectConversation(id),
      getConversationEngine: (convId) => get().getConversationEngine(convId),
      saveConversationEngine: (convId, engine) => get().saveConversationEngine(convId, engine),
      getAgentProfiles: () => get().agentProfiles,
      loadWorkflowSkills: () => get().loadWorkflowSkills(),
      loadSkills: () => get().loadSkills(),
      detectAndRecommendSkills: () => get().detectAndRecommendSkills(),
    });
  },
});
