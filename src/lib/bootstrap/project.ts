/**
 * Project bootstrap — extracted from `projectSlice.selectProject` (Finding 1-2
 * in `docs/plans/refactorRoadmap_2026-04-20.md`). Before this split, a single
 * 163-line action owned 9 independent subsystems (conversations, profile,
 * skills, stack info, rawq, fs watcher, pty listeners, …) and there was no
 * way to tell which step failed when startup logs went quiet.
 *
 * Each step is a top-level function so a failure surfaces as
 * `ProjectBootstrapError { step: "loadConversations", ... }` or, for the
 * fire-and-forget steps that must not abort the chain, as a discrete
 * `[bootstrap/project] <step>:` console entry.
 *
 * The module keeps three cleanup closures (rawq listener, pty listener,
 * fs watcher) at module scope — these are inherently process-global
 * subscriptions that outlive any single `selectProject` call, so parking
 * them in state would only obscure that they exist.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Project, Conversation, RawqStatus, AgentProfile } from "@/types";

export class ProjectBootstrapError extends Error {
  constructor(public step: string, message: string, public cause?: unknown) {
    super(message);
    this.name = "ProjectBootstrapError";
  }
}

// Module-level cleanups. These are NOT moved into zustand state because
// they are not data — they are references to `listen()` unsubscribe
// functions whose lifetime spans multiple `selectProject` calls and whose
// type leaks AbortController-style ergonomics that don't fit state shape.
let rawqListenerCleanup: (() => void) | null = null;
let ptyListenerCleanup: (() => void) | null = null;
let fsWatcherCleanup: (() => void) | null = null;
// Path of the project whose rawq build was started by the most recent
// `bootstrapRawq` call. Tracked at module scope so `teardownPreviousProject`
// can ask the rust side to cancel the in-flight build before swapping
// projects (plan rawqIndexCancelChannelPlan_2026-04-25.md INV-1/INV-2).
let activeRawqProjectPath: string | null = null;

// ─── Callback contract ─────────────────────────────────────────────────

export interface BootstrapCallbacks {
  /** Zustand setState — accepts full or partial ChatState updates. */
  setState: (patch: Record<string, unknown>) => void;
  /** Read current selected project key to bail out of stale async work. */
  getSelectedProjectKey: () => string | null;
  /** Delegate conversation selection to conversationSlice. */
  selectConversation: (id: string) => Promise<void>;
  /** Per-conversation engine/model memory. Null return matches the
   *  canonical signature in `ChatState`. */
  getConversationEngine: (convId: string) =>
    | { engine: string; model?: string; profileId: string | null }
    | null;
  saveConversationEngine: (
    convId: string,
    engine: { engine: string; model?: string; profileId: string | null },
  ) => void;
  getAgentProfiles: () => AgentProfile[];
  loadWorkflowSkills: () => Promise<void>;
  loadSkills: () => Promise<void>;
  detectAndRecommendSkills: () => Promise<void>;
  /** PTY auto-restart — respawn session when pty:exit fires. Needs access
   *  to both the current conversation and project path. */
  getSelectedConversationId: () => string | null;
}

// ─── Individual steps ─────────────────────────────────────────────────

/** Step 1. Release previous-project subscriptions. Idempotent.
 *
 * Also asks the rust side to cancel any in-flight rawq index build for the
 * previous project. The cancel is fire-and-forget — the backend treats an
 * unknown path as a no-op (see `cancel_rawq_index`), so this is safe to
 * call when no build is running. */
export function teardownPreviousProject(): void {
  if (rawqListenerCleanup) { rawqListenerCleanup(); rawqListenerCleanup = null; }
  if (ptyListenerCleanup) { ptyListenerCleanup(); ptyListenerCleanup = null; }
  if (fsWatcherCleanup) { fsWatcherCleanup(); fsWatcherCleanup = null; }

  if (activeRawqProjectPath) {
    const prevPath = activeRawqProjectPath;
    activeRawqProjectPath = null;
    invoke("cancel_rawq_index", { projectPath: prevPath })
      .catch((e) => console.debug("[bootstrap/project] cancel rawq:", e));
  }
}

/** Step 2. Reset transient state + remember the selection for next launch. */
export function setInitialState(
  key: string,
  setState: BootstrapCallbacks["setState"],
): void {
  setState({
    selectedProjectKey: key,
    selectedConversationId: null,
    messages: [],
    branches: [],
    rawqStatus: null,
    projectLoading: "Loading project...",
  });
  import("@/lib/appStore")
    .then(({ setSetting }) => setSetting("lastProjectKey", key))
    .catch((e) => console.debug("[bootstrap/project] persist last key:", e));
  // recent-projects ordering 을 위해 last_opened_at 갱신 (fire-and-forget).
  // ProjectStartup 화면의 "최근 열었던 프로젝트" 섹션은 이 timestamp 의
  // DESC 순으로 노출 (Plan C Task 02). 갱신 실패는 UX 영향만 — bootstrap
  // 체인을 막지 않는다.
  invoke("touch_project_opened_at", { key })
    .catch((e) => console.debug("[bootstrap/project] touch last_opened_at:", e));
}

/**
 * Step 3. Fetch conversations — if none exist, create a Main conversation
 * so the UI always has something to select.
 */
export async function loadConversations(key: string): Promise<Conversation[]> {
  try {
    let conversations = await invoke<Conversation[]>("list_conversations", {
      projectKey: key,
    });
    if (conversations.length === 0) {
      const main = await invoke<Conversation>("create_conversation", {
        input: { projectKey: key, label: "Main", type: "main", mode: "chat", source: "tunadish" },
      });
      conversations = [main];
    }
    return conversations;
  } catch (e) {
    throw new ProjectBootstrapError(
      "loadConversations",
      `failed to fetch conversations: ${e}`,
      e,
    );
  }
}

/**
 * Step 4. Activate the Main conversation (or the first one) and assign a
 * default agent profile when the saved engine map has no entry.
 */
export async function ensureMainConversation(
  conversations: Conversation[],
  cb: BootstrapCallbacks,
): Promise<void> {
  const mainConv = conversations.find((c) => c.type === "main") ?? conversations[0];
  if (!mainConv) return;
  await cb.selectConversation(mainConv.id);
  const saved = cb.getConversationEngine(mainConv.id);
  if (saved) return;
  const defaultProfile = cb.getAgentProfiles()[0]; // architect-claude by convention
  if (!defaultProfile) return;
  cb.saveConversationEngine(mainConv.id, {
    profileId: defaultProfile.id,
    engine: defaultProfile.engine,
    model: defaultProfile.model,
  });
}

/** Step 5. Skills — fire-and-forget. Individual failures are logged but
 *  must not abort the bootstrap chain. */
export function bootstrapSkills(cb: BootstrapCallbacks): void {
  cb.loadWorkflowSkills()
    .catch((e) => console.debug("[bootstrap/project] workflow-skills:", e));
  cb.loadSkills()
    .then(() =>
      cb.detectAndRecommendSkills()
        .catch((e) => console.debug("[bootstrap/project] skills-detect:", e)),
    )
    .catch((e) => console.debug("[bootstrap/project] skills-load:", e));
}

/** Step 6. Workflow agent templates + project stack info. Fire-and-forget. */
export function bootstrapStackInfo(key: string): void {
  invoke<Project>("get_project", { key })
    .then((p) => {
      if (!p.path) return;
      invoke("ensure_project_workflow_templates", { projectPath: p.path })
        .catch((e) => console.debug("[bootstrap/project] workflow-templates:", e));
      invoke("refresh_project_stack_info", { projectPath: p.path, projectName: p.name })
        .catch((e) => console.debug("[bootstrap/project] stack-info:", e));
    })
    .catch((e) => console.debug("[bootstrap/project] get-project (stack):", e));
}

/**
 * Step 7. rawq status probe + optional background index + event listeners.
 * Returns the project path so the fs-watcher step can reuse it without
 * re-invoking `get_project`.
 */
export async function bootstrapRawq(
  key: string,
  cb: BootstrapCallbacks,
): Promise<string | null> {
  try {
    const project = await invoke<Project>("get_project", { key });
    if (!project.path) {
      cb.setState({
        rawqStatus: { available: false, indexed: false, status: "unavailable", message: "no project path" },
      });
      return null;
    }
    const projectPath = project.path;

    const status = await invoke<RawqStatus>("get_rawq_status", { projectPath });
    if (cb.getSelectedProjectKey() !== key) return projectPath;
    cb.setState({ rawqStatus: status });

    if (status.available && !status.indexed) {
      cb.setState({
        rawqStatus: { ...status, status: "indexing", message: "building index..." },
      });
      const ulDone = await listen<RawqStatus>("rawq:indexed", (e) => {
        if (cb.getSelectedProjectKey() === key) cb.setState({ rawqStatus: e.payload });
        cleanup();
      });
      const ulErr = await listen<RawqStatus>("rawq:error", (e) => {
        if (cb.getSelectedProjectKey() === key) cb.setState({ rawqStatus: e.payload });
        cleanup();
      });
      const ulCancelled = await listen<{ projectPath: string }>(
        "rawq:cancelled",
        (e) => {
          // Surface a non-error idle state — the user dismissed the
          // project before the build completed. We do NOT clear listeners
          // here because the same project could still be the selected one
          // (rebuild_rawq_index path); the success/error listener will
          // clean up on the next attempt.
          if (cb.getSelectedProjectKey() === key) {
            cb.setState({
              rawqStatus: {
                available: true,
                indexed: false,
                status: "ready",
                message: "indexing cancelled",
              },
            });
          }
          console.debug(`[bootstrap/project] rawq cancelled for ${e.payload.projectPath}`);
          cleanup();
        },
      );
      const cleanup = () => {
        ulDone();
        ulErr();
        ulCancelled();
        if (rawqListenerCleanup === cleanup) rawqListenerCleanup = null;
      };
      rawqListenerCleanup = cleanup;

      activeRawqProjectPath = projectPath;
      await invoke("start_rawq_index", { projectPath });
    }
    return projectPath;
  } catch (e) {
    console.debug("[bootstrap/project] rawq probe failed:", e);
    if (cb.getSelectedProjectKey() === key) {
      cb.setState({
        rawqStatus: { available: false, indexed: false, status: "unavailable", message: "rawq not found" },
      });
    }
    return null;
  }
}

/**
 * Step 8. fs watcher — debounce 3 s, re-runs `start_rawq_index` when
 * anything under the project path changes.
 */
export async function bootstrapFileWatcher(
  key: string,
  projectPath: string | null,
  cb: BootstrapCallbacks,
): Promise<void> {
  if (!projectPath) return;
  if (fsWatcherCleanup) { fsWatcherCleanup(); fsWatcherCleanup = null; }
  try {
    const { watch } = await import("@tauri-apps/plugin-fs");
    let debounceTimer: ReturnType<typeof setTimeout> | null = null;
    const stopWatcher = await watch(
      projectPath,
      () => {
        if (debounceTimer) clearTimeout(debounceTimer);
        debounceTimer = setTimeout(() => {
          if (cb.getSelectedProjectKey() === key) {
            // Track the path so a subsequent project switch can cancel
            // this fs-watcher-triggered build, not just the initial one.
            activeRawqProjectPath = projectPath;
            invoke("start_rawq_index", { projectPath })
              .catch((e) => console.debug("[bootstrap/project] rawq-reindex:", e));
          }
        }, 3000);
      },
      { recursive: true },
    );
    fsWatcherCleanup = () => {
      stopWatcher();
      if (debounceTimer) clearTimeout(debounceTimer);
    };
  } catch {
    console.debug("[bootstrap/project] fs watcher unavailable — @tauri-apps/plugin-fs missing");
  }
}

/**
 * Step 9. PTY project-scoped event listeners. The actual spawn happens in
 * `conversationSlice.spawnPtyForConversation`; this step only wires the
 * shared `pty:output` and `pty:exit` channels and sets up auto-restart on
 * unexpected exits.
 */
export async function bootstrapPtyListeners(cb: BootstrapCallbacks): Promise<void> {
  if (ptyListenerCleanup) { ptyListenerCleanup(); ptyListenerCleanup = null; }
  try {
    const { usePtyStore, PTY_ENGINES } = await import("@/stores/ptyStore");
    const { listen: tauriListen } = await import("@tauri-apps/api/event");

    const ulOutput = await tauriListen<{ sessionId: number; data: string }>("pty:output", () => {});
    const ulExit = await tauriListen<{ sessionId: number }>("pty:exit", (e) => {
      const store = usePtyStore.getState();
      for (const engine of PTY_ENGINES) {
        const sid = store.getSession(engine);
        if (sid === e.payload.sessionId) {
          store.clearSession(engine);
          console.warn(`[pty] ${engine} session ${sid} exited — attempting auto-restart`);
          const convId = cb.getSelectedConversationId();
          const projKey = cb.getSelectedProjectKey();
          if (convId && projKey) {
            Promise.all([
              import("@tauri-apps/api/core").then((m) => m.invoke),
              import("@/stores/slices/conversationSlice").then((m) => m.spawnPtyForConversation),
            ])
              .then(async ([invokeCmd, spawnPty]) => {
                const project = await invokeCmd<{ path?: string }>("get_project", { key: projKey });
                if (!project.path) return;
                const conv = await invokeCmd<Conversation>("get_conversation", { id: convId });
                await spawnPty(conv, project.path);
                console.log(`[pty] ${engine} auto-restarted for conv ${convId}`);
              })
              .catch((err) => console.warn("[pty] auto-restart failed:", err));
          }
          break;
        }
      }
    });
    ptyListenerCleanup = () => {
      ulOutput();
      ulExit();
    };
  } catch (e) {
    console.debug("[bootstrap/project] pty-init:", e);
  }
}

// ─── Orchestrator ─────────────────────────────────────────────────────

/**
 * Run the full bootstrap sequence. Critical steps (conversations) throw
 * `ProjectBootstrapError`; the remaining steps degrade gracefully so a
 * rawq / pty / fs-watcher failure does not block the UI from rendering.
 */
export async function runProjectBootstrap(
  key: string,
  cb: BootstrapCallbacks,
): Promise<void> {
  teardownPreviousProject();
  setInitialState(key, cb.setState);

  let conversations: Conversation[];
  try {
    conversations = await loadConversations(key);
  } catch (e) {
    const msg = e instanceof ProjectBootstrapError ? e.message : String(e);
    cb.setState({ error: msg, projectLoading: null });
    return;
  }
  cb.setState({ conversations, error: null, projectLoading: null });

  try {
    await ensureMainConversation(conversations, cb);
  } catch (e) {
    console.debug("[bootstrap/project] ensureMainConversation:", e);
  }

  // Fire-and-forget subsystems (no await)
  bootstrapSkills(cb);
  bootstrapStackInfo(key);

  // rawq probe returns project path so fs watcher can reuse it
  const projectPath = await bootstrapRawq(key, cb);
  await bootstrapFileWatcher(key, projectPath, cb);
  await bootstrapPtyListeners(cb);
}
