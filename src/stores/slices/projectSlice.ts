import { invoke } from "@tauri-apps/api/core";
import type { SetState, GetState, Project, CreateProjectInput, RawqStatus } from "./types";

export interface ProjectSlice {
  projects: Project[];
  selectedProjectKey: string | null;
  loadProjects: () => Promise<void>;
  createProject: (input: CreateProjectInput) => Promise<void>;
  selectProject: (key: string) => Promise<void>;
}

export const createProjectSlice = (set: SetState, get: GetState): ProjectSlice => ({
  projects: [],
  selectedProjectKey: null,

  loadProjects: async () => {
    try {
      const projects = await invoke<Project[]>("list_projects");
      set({ projects, error: null });
    } catch (e) {
      set({ error: String(e) });
    }
  },

  createProject: async (input: CreateProjectInput) => {
    try {
      await invoke<Project>("create_project", { input });
      await get().loadProjects();
    } catch (e) {
      set({ error: String(e) });
    }
  },

  selectProject: async (key: string) => {
    set({ selectedProjectKey: key, selectedConversationId: null, messages: [], branches: [], rawqStatus: null });
    // 마지막 프로젝트 기억
    import("@/lib/appStore").then(({ setSetting }) => setSetting("lastProjectKey", key)).catch(() => {});
    try {
      const conversations = await invoke<import("./types").Conversation[]>("list_conversations", {
        projectKey: key,
      });
      set({ conversations, error: null });

      // rawq: check status → ensure index → update status
      const project = await invoke<Project>("get_project", { key });
      if (project.path) {
        // 1. Quick status check
        const initialStatus = await invoke<RawqStatus>("get_rawq_status", { projectPath: project.path });
        set({ rawqStatus: initialStatus });

        // 2. If not indexed, trigger build (shows "indexing..." in UI)
        if (initialStatus.available && !initialStatus.indexed) {
          set({ rawqStatus: { ...initialStatus, status: "indexing", message: "building index..." } });
          const result = await invoke<RawqStatus>("ensure_rawq_index", { projectPath: project.path });
          set({ rawqStatus: result });
        }
      }
    } catch (e) {
      set({ error: String(e) });
    }
  },
});
