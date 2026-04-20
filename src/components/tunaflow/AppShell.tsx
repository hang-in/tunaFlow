import { useEffect, useState, useCallback, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useChatStore } from "@/stores/chatStore";
import { getSetting, setSetting } from "@/lib/appStore";
import { Sidebar } from "./Sidebar";
import { CenterPanel } from "./CenterPanel";
import { BranchThreadPanel } from "./BranchThreadPanel";
import { RuntimeStatusBar } from "./RuntimeStatusBar";
import { ProjectStartup } from "./ProjectStartup";
// ResizeHandle removed — main area border serves as drag handle
import { FileViewer } from "./chat/FileViewer";
import { FileViewerContext } from "./chat/fileViewerContext";
import { Toaster } from "sonner";
import { CommandPalette } from "./CommandPalette";
import { TitleBar } from "./TitleBar";
import { MetaFloatingChat } from "./MetaFloatingChat";
import { ProjectOnboardingModal } from "./ProjectOnboardingModal";

// ─── Panel width constraints ─────────────────────────────────────────────────
const SIDEBAR_MIN = 220;
const SIDEBAR_MAX = 360;
const SIDEBAR_DEFAULT = 244;

const DRAWER_MIN = 360;
const DRAWER_DEFAULT = 480;

// ─── Helpers ─────────────────────────────────────────────────────────────────
const clamp = (v: number, min: number, max: number) => Math.min(Math.max(v, min), max);

export function AppShell() {
  const { loadProjects, createProject, loadEngineModels, threadBranchId } = useChatStore();
  const drawerPinned = useChatStore((s) => s.drawerPinned);

  const [sidebarW, setSidebarW] = useState(SIDEBAR_DEFAULT);
  const [drawerW, setDrawerW] = useState(DRAWER_DEFAULT);
  const [loaded, setLoaded] = useState(false);
  const [themeMode, setThemeMode] = useState<"dark" | "light">("dark");
  // Auto-hide sidebar when window is too narrow (< sidebar + min chat width)
  const SIDEBAR_HIDE_THRESHOLD = SIDEBAR_MIN + 680; // ~900px
  const [sidebarAutoHidden, setSidebarAutoHidden] = useState(() => window.innerWidth < SIDEBAR_MIN + 680);
  const [sidebarResizing, setSidebarResizing] = useState(false);

  useEffect(() => {
    const init = async () => {
      const [sw, dw, themeMode] = await Promise.all([
        getSetting<number>("sidebarWidth", SIDEBAR_DEFAULT),
        getSetting<number>("drawerWidth", DRAWER_DEFAULT),
        getSetting<string>("themeMode", "dark"),
      ]);
      // Apply theme class to <html> — light mode uses CSS variable overrides
      const mode = themeMode === "light" ? "light" : "dark";
      document.documentElement.classList.toggle("light", mode === "light");
      setThemeMode(mode as "dark" | "light");
      setSidebarW(clamp(sw, SIDEBAR_MIN, SIDEBAR_MAX));
      setDrawerW(Math.max(dw, DRAWER_MIN));
      setLoaded(true);

      // Cleanup stale jobs/messages from interrupted background runs
      invoke("cleanup_stale_jobs").catch((e) => console.debug("[cleanup]", e));
      // Clear in-memory running state (processes died on restart)
      useChatStore.setState({ runningThreadIds: [] });

      await loadProjects();
      loadEngineModels();
      useChatStore.getState().loadProfiles();
      const { projects, selectProject } = useChatStore.getState();

      const lastKey = await getSetting<string>("lastProjectKey", "");
      let proj = lastKey ? projects.find((p) => p.key === lastKey) : null;
      if (!proj) proj = projects[0];
      // No auto-create: if no projects, show ProjectStartup instead
      if (proj) {
        await selectProject(proj.key);
        // Restore last conversation
        const lastConvId = await getSetting<string>("lastConversationId", "");
        if (lastConvId) {
          const { conversations, selectConversation } = useChatStore.getState();
          if (conversations.some((c) => c.id === lastConvId)) {
            selectConversation(lastConvId);
          }
        }
      }
    };
    init();
  }, []);

  // Auto-hide sidebar on window resize
  useEffect(() => {
    const onResize = () => setSidebarAutoHidden(window.innerWidth < SIDEBAR_HIDE_THRESHOLD);
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, [SIDEBAR_HIDE_THRESHOLD]);

  // Persist on resize end
  const persistSidebar = useCallback(() => { setSetting("sidebarWidth", sidebarW); }, [sidebarW]);
  const persistDrawer = useCallback(() => { setSetting("drawerWidth", drawerW); }, [drawerW]);

  const handleSidebarResize = useCallback((delta: number) => {
    setSidebarW((w) => clamp(w + delta, SIDEBAR_MIN, SIDEBAR_MAX));
  }, []);

  // Drawer max = 90% of work area (everything right of sidebar)
  const drawerMax = useCallback(() => {
    const workArea = window.innerWidth - sidebarW - 8; // 8px for resize handles
    return Math.max(DRAWER_MIN, Math.floor(workArea * 0.9));
  }, [sidebarW]);

  const handleDrawerResize = useCallback((delta: number) => {
    setDrawerW((w) => clamp(w + delta, DRAWER_MIN, drawerMax()));
  }, [drawerMax]);

  const drawerOpen = !!threadBranchId;

  // FileViewer — shared across ChatPanel and BranchThreadPanel
  const [viewerFile, setViewerFile] = useState<{ path: string; line?: number } | null>(null);
  const selectedProjectKey = useChatStore((s) => s.selectedProjectKey);
  const projects = useChatStore((s) => s.projects);
  const projectPath = useMemo(() => {
    const proj = projects.find((p) => p.key === selectedProjectKey);
    return proj?.path ?? null;
  }, [projects, selectedProjectKey]);
  const fileViewerCtx = useMemo(() => ({
    openFile: (path: string, line?: number) => setViewerFile({ path, line }),
  }), []);

  // 로딩 전에는 UI 를 뿌리지 않지만, null 을 반환하면 webview 기본 배경이
  // 한순간 보인 뒤 bg-sidebar 가 덮여 플래시처럼 느껴짐. sidebar 와 같은
  // 어두운 색으로 화면 전체를 미리 칠해 깜빡임 완화.
  if (!loaded) return <div className="fixed inset-0 bg-sidebar" />;

  // Project-first startup: show selector if no project is selected
  if (!selectedProjectKey) {
    return <ProjectStartup />;
  }

  return (
    <FileViewerContext.Provider value={fileViewerCtx}>
    <Toaster position="bottom-right" theme={themeMode} richColors closeButton />
    <div className="flex flex-col h-screen w-screen overflow-hidden bg-sidebar text-foreground font-sans relative">
      {/* ── Title bar (macOS overlay) ── */}
      <TitleBar />

      {/* ── Body: sidebar + main ── */}
      <div className="flex flex-1 min-h-0">
        {/* Sidebar — flat, darkest layer (auto-hidden when window too narrow) */}
        <div
          style={{ width: sidebarAutoHidden ? 0 : sidebarW }}
          className={`shrink-0 h-full overflow-hidden ${sidebarResizing ? "" : "transition-[width] duration-200 ease-in-out"}`}
        >
          <Sidebar />
        </div>

        {/* Resize handle — between sidebar and main area */}
        {!sidebarAutoHidden && (
          <div
            className="relative shrink-0 w-1 cursor-col-resize group"
            onMouseDown={(e) => {
              e.preventDefault();
              setSidebarResizing(true);
              let lastX = e.clientX;
              const onMove = (ev: MouseEvent) => {
                const delta = ev.clientX - lastX;
                lastX = ev.clientX;
                handleSidebarResize(delta);
              };
              const onUp = () => {
                document.removeEventListener("mousemove", onMove);
                document.removeEventListener("mouseup", onUp);
                document.body.style.cursor = "";
                document.body.style.userSelect = "";
                setSidebarResizing(false);
                persistSidebar();
              };
              document.addEventListener("mousemove", onMove);
              document.addEventListener("mouseup", onUp);
              document.body.style.cursor = "col-resize";
              document.body.style.userSelect = "none";
            }}
          >
            {/* 1px visual line, centered on boundary, hover-only */}
            <div className="absolute inset-y-0 left-1/2 -translate-x-px w-px bg-transparent group-hover:bg-border/50 transition-colors duration-150" />
          </div>
        )}

        {/* Main area */}
        <div className="flex-1 min-w-0 h-full relative flex">
          {/* Meta floating chat — anchored to top-left of main area */}
          {selectedProjectKey && (
            <MetaFloatingChat projectKey={selectedProjectKey} />
          )}

          {/* CenterPanel — full width when no drawer, or flex-1 when pinned */}
          <div className={drawerOpen && drawerPinned ? "flex-1 min-w-0 h-full" : "flex-1 min-w-0 h-full"}>
            <CenterPanel />
          </div>

          {/* ── Thread/RT Drawer ── */}
          {drawerOpen && drawerPinned ? (
            /* Pinned mode — side-by-side with CenterPanel, edge-grab resize */
            <div
              style={{ width: drawerW }}
              className="shrink-0 h-full bg-background rounded-l-xl overflow-hidden relative"
            >
              {/* Left edge resize handle — invisible, cursor changes on hover */}
              <div
                className="absolute left-0 top-0 bottom-0 w-1.5 cursor-col-resize z-10 hover:bg-primary/8 transition-colors"
                onMouseDown={(e) => {
                  e.preventDefault();
                  const startX = e.clientX;
                  const startW = drawerW;
                  const max = drawerMax();
                  const onMove = (ev: MouseEvent) => {
                    const delta = startX - ev.clientX;
                    setDrawerW(clamp(startW + delta, DRAWER_MIN, max));
                  };
                  const onUp = () => {
                    document.removeEventListener("mousemove", onMove);
                    document.removeEventListener("mouseup", onUp);
                    document.body.style.cursor = "";
                    document.body.style.userSelect = "";
                    persistDrawer();
                  };
                  document.addEventListener("mousemove", onMove);
                  document.addEventListener("mouseup", onUp);
                  document.body.style.cursor = "col-resize";
                  document.body.style.userSelect = "none";
                }}
              />
              <BranchThreadPanel />
            </div>
          ) : drawerOpen ? (
            /* Overlay mode — edge-grab resize, no visible line */
            <>
              {/* Backdrop — covers main area only, fade in.
                  \`bg-black/8\` 은 거의 투명 수준이라 드로어 열릴 때 뒤쪽 메인
                  영역이 "색이 급변하는" 인상을 줬음. 명확히 dim 되는 수준으로 강화. (s37) */}
              <div
                className="absolute inset-0 z-40 bg-black/30"
                style={{ animation: "fade-in 200ms ease-out" }}
                onClick={() => useChatStore.getState().closeThread()}
              />

              {/* Drawer — anchored to right edge, slide in from right */}
              <div
                style={{ width: drawerW, animation: "slide-in-from-right 200ms ease-out" }}
                className="absolute top-1.5 right-0 bottom-1.5 z-50"
              >
                {/* Left edge resize handle — invisible, overlays drawer left edge */}
                <div
                  className="absolute left-0 top-0 bottom-0 w-2 cursor-col-resize z-10 hover:bg-primary/8 transition-colors"
                  onMouseDown={(e) => {
                    e.preventDefault();
                    const startX = e.clientX;
                    const startW = drawerW;
                    const max = drawerMax();

                    const onMove = (ev: MouseEvent) => {
                      const delta = startX - ev.clientX;
                      setDrawerW(clamp(startW + delta, DRAWER_MIN, max));
                    };
                    const onUp = () => {
                      document.removeEventListener("mousemove", onMove);
                      document.removeEventListener("mouseup", onUp);
                      document.body.style.cursor = "";
                      document.body.style.userSelect = "";
                      persistDrawer();
                    };
                    document.addEventListener("mousemove", onMove);
                    document.addEventListener("mouseup", onUp);
                    document.body.style.cursor = "col-resize";
                    document.body.style.userSelect = "none";
                  }}
                />

                {/* Drawer content */}
                <div className="h-full bg-background shadow-[-4px_0_16px_-2px_rgba(0,0,0,0.2)] rounded-l-xl overflow-hidden">
                  <BranchThreadPanel />
                </div>
              </div>
            </>
          ) : null}
        </div>
      </div>

      {/* ── Status bar — full width, app bottom ── */}
      <RuntimeStatusBar />

      {viewerFile && projectPath && (
        <FileViewer
          filePath={viewerFile.path}
          projectPath={projectPath}
          lineNumber={viewerFile.line}
          onClose={() => setViewerFile(null)}
        />
      )}
    </div>
    <CommandPalette />
    <ProjectOnboardingModal />
    </FileViewerContext.Provider>
  );
}
