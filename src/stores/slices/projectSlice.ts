import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { errorMessage } from "@/lib/utils";
import type { SetState, GetState, Project, CreateProjectInput, RawqStatus } from "./types";

export interface ProjectSlice {
  projects: Project[];
  selectedProjectKey: string | null;
  projectLoading: string | null; // loading message or null
  loadProjects: () => Promise<void>;
  createProject: (input: CreateProjectInput) => Promise<void>;
  hideProject: (key: string) => Promise<void>;
  selectProject: (key: string) => Promise<void>;
}

// Cleanup function for previous rawq listeners
let rawqListenerCleanup: (() => void) | null = null;
// Cleanup function for fs watcher
let fsWatcherCleanup: (() => void) | null = null;

export const createProjectSlice = (set: SetState, get: GetState): ProjectSlice => ({
  projects: [],
  selectedProjectKey: null,
  projectLoading: null,

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
    } catch (e) {
      set({ error: errorMessage(e) });
    }
  },

  hideProject: async (key: string) => {
    try {
      await invoke("hide_project", { key });
      const { selectedProjectKey } = get();
      await get().loadProjects();
      // If hiding the currently selected project, clear selection
      if (selectedProjectKey === key) {
        set({
          selectedProjectKey: null, selectedConversationId: null,
          messages: [], branches: [], conversations: [],
          memos: [], artifacts: [], rawqStatus: null,
        });
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
    // Cleanup previous rawq listeners
    if (rawqListenerCleanup) {
      rawqListenerCleanup();
      rawqListenerCleanup = null;
    }

    set({ selectedProjectKey: key, selectedConversationId: null, messages: [], branches: [], rawqStatus: null, projectLoading: "Loading project..." });
    import("@/lib/appStore").then(({ setSetting }) => setSetting("lastProjectKey", key)).catch((e) => console.debug("[settings]", e));
    try {
      let conversations = await invoke<import("./types").Conversation[]>("list_conversations", {
        projectKey: key,
      });

      // Auto-create Main conversation if none exist
      if (conversations.length === 0) {
        const main = await invoke<import("./types").Conversation>("create_conversation", {
          input: { projectKey: key, label: "Main", type: "main", mode: "chat", source: "tunadish" },
        });
        conversations = [main];
      }

      set({ conversations, error: null, projectLoading: null });

      // Auto-select first main conversation + ensure default profile
      const mainConv = conversations.find((c) => c.type === "main") ?? conversations[0];
      if (mainConv) {
        await get().selectConversation(mainConv.id);
        // Set default profile (architect) for conversations without a saved profile
        const saved = get().getConversationEngine(mainConv.id);
        if (!saved) {
          const defaultProfile = get().agentProfiles[0]; // architect-claude
          if (defaultProfile) {
            get().saveConversationEngine(mainConv.id, {
              profileId: defaultProfile.id,
              engine: defaultProfile.engine,
              model: defaultProfile.model,
            });
          }
        }
      }
    } catch (e) {
      set({ error: errorMessage(e), projectLoading: null });
      return;
    }

    // Load project-scoped workflow skill mappings + skill selection
    get().loadWorkflowSkills().catch((e) => console.debug("[workflow-skills]", e));
    get().loadSkills().then(() => {
      get().detectAndRecommendSkills().catch((e) => console.debug("[skills-detect]", e));
    }).catch((e) => console.debug("[skills-load]", e));

    // Ensure workflow agent templates + refresh stack info (non-blocking, fire-and-forget)
    invoke<Project>("get_project", { key }).then((p) => {
      if (p.path) {
        invoke("ensure_project_workflow_templates", { projectPath: p.path }).catch((e) => console.debug("[workflow-templates]", e));
        invoke("refresh_project_stack_info", { projectPath: p.path, projectName: p.name }).catch((e) => console.debug("[stack-info]", e));
      }
    }).catch((e) => console.debug("[get-project]", e));

    // rawq: non-blocking — check status, then start background index if needed
    invoke<Project>("get_project", { key }).then(async (project) => {
      if (!project.path) {
        set({ rawqStatus: { available: false, indexed: false, status: "unavailable", message: "no project path" } });
        return;
      }
      const projectPath = project.path;
      try {
        const status = await invoke<RawqStatus>("get_rawq_status", { projectPath });
        if (get().selectedProjectKey !== key) return;
        set({ rawqStatus: status });

        if (status.available && !status.indexed) {
          set({ rawqStatus: { ...status, status: "indexing", message: "building index..." } });

          // Listen for background indexing events
          const ulDone = await listen<RawqStatus>("rawq:indexed", (e) => {
            if (get().selectedProjectKey === key) set({ rawqStatus: e.payload });
            cleanup();
          });
          const ulErr = await listen<RawqStatus>("rawq:error", (e) => {
            if (get().selectedProjectKey === key) set({ rawqStatus: e.payload });
            cleanup();
          });

          const cleanup = () => {
            ulDone(); ulErr();
            if (rawqListenerCleanup === cleanup) rawqListenerCleanup = null;
          };
          rawqListenerCleanup = cleanup;

          // Fire background index — returns immediately
          await invoke("start_rawq_index", { projectPath });
        }

        // Start fs watcher for project directory — triggers re-index on file changes
        if (fsWatcherCleanup) { fsWatcherCleanup(); fsWatcherCleanup = null; }
        try {
          const { watch } = await import("@tauri-apps/plugin-fs");
          let debounceTimer: ReturnType<typeof setTimeout> | null = null;
          const stopWatcher = await watch(projectPath, (_event) => {
            // Debounce: wait 3 seconds after last change before re-indexing
            if (debounceTimer) clearTimeout(debounceTimer);
            debounceTimer = setTimeout(() => {
              if (get().selectedProjectKey === key) {
                invoke("start_rawq_index", { projectPath }).catch((e) => console.debug("[rawq-reindex]", e));
              }
            }, 3000);
          }, { recursive: true });
          fsWatcherCleanup = () => { stopWatcher(); if (debounceTimer) clearTimeout(debounceTimer); };
        } catch {
          console.debug("[rawq] fs watcher unavailable — install @tauri-apps/plugin-fs");
        }
      } catch {
        if (get().selectedProjectKey === key) {
          set({ rawqStatus: { available: false, indexed: false, status: "unavailable", message: "rawq not found" } });
        }
      }
    }).catch(() => {
      set({ rawqStatus: { available: false, indexed: false, status: "unavailable", message: "rawq not found" } });
    });
  },
});
