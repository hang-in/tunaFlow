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

  const [sidebarW, setSidebarW] = useState(SIDEBAR_DEFAULT);
  const [drawerW, setDrawerW] = useState(DRAWER_DEFAULT);
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    const init = async () => {
      const [sw, dw] = await Promise.all([
        getSetting<number>("sidebarWidth", SIDEBAR_DEFAULT),
        getSetting<number>("drawerWidth", DRAWER_DEFAULT),
      ]);
      setSidebarW(clamp(sw, SIDEBAR_MIN, SIDEBAR_MAX));
      setDrawerW(Math.max(dw, DRAWER_MIN));
      setLoaded(true);

      // Cleanup stale jobs/messages from interrupted background runs
      invoke("cleanup_stale_jobs").catch(() => {});
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

  if (!loaded) return null;

  // Project-first startup: show selector if no project is selected
  if (!selectedProjectKey) {
    return <ProjectStartup />;
  }

  return (
    <FileViewerContext.Provider value={fileViewerCtx}>
    <Toaster position="bottom-right" theme="dark" richColors closeButton />
    <div className="flex flex-col h-screen w-screen overflow-hidden bg-sidebar text-foreground font-sans">
      {/* ── Body: sidebar + main ── */}
      <div className="flex flex-1 min-h-0">
        {/* Sidebar — flat, darkest layer */}
        <div style={{ width: sidebarW }} className="shrink-0 h-full">
          <Sidebar />
        </div>

        {/* Resize handle — between sidebar and main area */}
        <div
          className="shrink-0 w-1.5 cursor-col-resize hover:bg-primary/10 transition-colors"
          onMouseDown={(e) => {
            e.preventDefault();
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
              persistSidebar();
            };
            document.addEventListener("mousemove", onMove);
            document.addEventListener("mouseup", onUp);
            document.body.style.cursor = "col-resize";
            document.body.style.userSelect = "none";
          }}
        />

        {/* Main area */}
        <div className="flex-1 min-w-0 h-full relative">
          <CenterPanel />

          {/* ── Thread/RT Drawer overlay ── */}
          {drawerOpen && (
            <>
              {/* Backdrop — covers main area only, very subtle */}
              <div
                className="absolute inset-0 z-40 bg-black/8"
                onClick={() => useChatStore.getState().closeThread()}
              />

              {/* Drawer — anchored to right edge */}
              <div
                style={{ width: drawerW }}
                className="absolute top-1.5 right-1.5 bottom-1.5 z-50 flex"
              >
                {/* Left-edge resize handle */}
                <div className="shrink-0 w-2 cursor-col-resize relative group"
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
                >
                  <div className="absolute top-0 bottom-0 left-1/2 -translate-x-1/2 w-px bg-border/40 group-hover:w-0.5 group-hover:bg-primary/30 transition-all duration-150" />
                </div>

                {/* Drawer content */}
                <div className="flex-1 min-w-0 bg-background shadow-[-4px_0_16px_-2px_rgba(0,0,0,0.2)] rounded-l-md overflow-hidden border-l border-border/30">
                  <BranchThreadPanel />
                </div>
              </div>
            </>
          )}
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
    </FileViewerContext.Provider>
  );
}
