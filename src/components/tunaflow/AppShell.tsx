import { useEffect, useState, useCallback, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useChatStore } from "@/stores/chatStore";
import { getSetting, setSetting } from "@/lib/appStore";
import { Sidebar } from "./Sidebar";
import { ChatPanel } from "./ChatPanel";
import { ContextPanel } from "./ContextPanel";
import { BranchThreadPanel } from "./BranchThreadPanel";
import { ResizeHandle } from "./ResizeHandle";
import { FileViewer } from "./chat/FileViewer";
import { FileViewerContext } from "./chat/fileViewerContext";

// ─── Panel width constraints ─────────────────────────────────────────────────
const SIDEBAR_MIN = 220;
const SIDEBAR_MAX = 360;
const SIDEBAR_DEFAULT = 224;

const CONTEXT_MIN = 260;
const CONTEXT_MAX = 520;
const CONTEXT_DEFAULT = 280;

const DRAWER_MIN = 360;
const DRAWER_DEFAULT = 480;

// ─── Helpers ─────────────────────────────────────────────────────────────────
const clamp = (v: number, min: number, max: number) => Math.min(Math.max(v, min), max);

export function AppShell() {
  const { loadProjects, createProject, loadEngineModels, threadBranchId } = useChatStore();

  const [sidebarW, setSidebarW] = useState(SIDEBAR_DEFAULT);
  const [contextW, setContextW] = useState(CONTEXT_DEFAULT);
  const [drawerW, setDrawerW] = useState(DRAWER_DEFAULT);
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    const init = async () => {
      const [sw, cw, dw] = await Promise.all([
        getSetting<number>("sidebarWidth", SIDEBAR_DEFAULT),
        getSetting<number>("contextPanelWidth", CONTEXT_DEFAULT),
        getSetting<number>("drawerWidth", DRAWER_DEFAULT),
      ]);
      setSidebarW(clamp(sw, SIDEBAR_MIN, SIDEBAR_MAX));
      setContextW(clamp(cw, CONTEXT_MIN, CONTEXT_MAX));
      setDrawerW(Math.max(dw, DRAWER_MIN));
      setLoaded(true);

      // Cleanup stale jobs/messages from interrupted background runs
      invoke("cleanup_stale_jobs").catch(() => {});

      await loadProjects();
      loadEngineModels();
      const { projects, selectProject } = useChatStore.getState();

      const lastKey = await getSetting<string>("lastProjectKey", "");
      let proj = lastKey ? projects.find((p) => p.key === lastKey) : null;
      if (!proj) proj = projects[0];
      if (!proj) {
        await createProject({ key: "default", name: "Workspace", type: "chat", source: "configured" });
        proj = useChatStore.getState().projects[0];
      }
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
  const persistContext = useCallback(() => { setSetting("contextPanelWidth", contextW); }, [contextW]);
  const persistDrawer = useCallback(() => { setSetting("drawerWidth", drawerW); }, [drawerW]);

  const handleSidebarResize = useCallback((delta: number) => {
    setSidebarW((w) => clamp(w + delta, SIDEBAR_MIN, SIDEBAR_MAX));
  }, []);

  const handleContextResize = useCallback((delta: number) => {
    setContextW((w) => clamp(w + delta, CONTEXT_MIN, CONTEXT_MAX));
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

  return (
    <FileViewerContext.Provider value={fileViewerCtx}>
    <div className="flex h-screen w-screen overflow-hidden bg-background text-foreground font-sans">
      {/* ── Sidebar ── */}
      <div style={{ width: sidebarW }} className="shrink-0 h-full">
        <Sidebar />
      </div>
      <ResizeHandle side="left" onResize={handleSidebarResize} onResizeEnd={persistSidebar} />

      {/* ── Work area: center + workspace panel ── */}
      {/* This is `relative` so the drawer overlay covers both center AND workspace panel */}
      <div className="flex-1 flex min-w-0 h-full relative">
        {/* Center — chat */}
        <div className="flex-1 min-w-0 h-full">
          <ChatPanel />
        </div>

        <ResizeHandle side="right" onResize={handleContextResize} onResizeEnd={persistContext} />

        {/* Workspace panel */}
        <div style={{ width: contextW }} className="shrink-0 h-full">
          <ContextPanel />
        </div>

        {/* ── Thread/RT Drawer overlay ── */}
        {/* Covers the entire work area (center + workspace panel) */}
        {drawerOpen && (
          <>
            {/* Backdrop — dims center + workspace panel, click to close */}
            <div
              className="absolute inset-0 z-40 bg-black/15 backdrop-blur-[1px]"
              onClick={() => useChatStore.getState().closeThread()}
            />

            {/* Drawer — anchored to right edge, overlays workspace panel */}
            <div
              style={{ width: drawerW }}
              className="absolute top-0 right-0 bottom-0 z-50 flex"
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
      {viewerFile && projectPath && (
        <FileViewer
          filePath={viewerFile.path}
          projectPath={projectPath}
          lineNumber={viewerFile.line}
          onClose={() => setViewerFile(null)}
        />
      )}
    </div>
    </FileViewerContext.Provider>
  );
}
