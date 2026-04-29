import { useEffect, useState, useCallback, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
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
import { SettingsPanel } from "./SettingsPanel";

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

  // Settings 진입점은 RuntimeStatusBar 내부 state 였으나 (s40 기준), Cmd+, /
  // macOS 메뉴 / Command Palette 등 어디서든 — 프로젝트 선택 전 ProjectStartup
  // 화면에서도 — 열려야 하므로 AppShell 루트로 끌어올렸다. RuntimeStatusBar
  // 의 SettingsPanel mount + 'tunaflow:open-settings' listener 는 제거되고,
  // 기존의 이벤트 디스패치 경로 (CommandPalette / roleAssignments) 는 그대로
  // 본 컴포넌트가 받는다 — 즉 진입점만 추가, 기존 경로 영향 0.
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [settingsInitialSection, setSettingsInitialSection] = useState<string | undefined>(undefined);
  // Auto-hide sidebar when window is too narrow (< sidebar + min chat width)
  const SIDEBAR_HIDE_THRESHOLD = SIDEBAR_MIN + 680; // ~900px
  const [sidebarAutoHidden, setSidebarAutoHidden] = useState(() => window.innerWidth < SIDEBAR_MIN + 680);
  const [sidebarResizing, setSidebarResizing] = useState(false);
  const [loadingStep, setLoadingStep] = useState<string>("시작 중...");

  useEffect(() => {
    const init = async () => {
      try {
        setLoadingStep("환경 설정 로드 중...");
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

        // Cleanup stale jobs/messages from interrupted background runs
        invoke("cleanup_stale_jobs").catch((e) => console.debug("[cleanup]", e));
        // Clear in-memory running state (processes died on restart)
        useChatStore.setState({ runningThreadIds: [] });

        setLoadingStep("프로젝트 목록 로드 중...");
        await loadProjects();
        setLoadingStep("엔진 / 모델 감지 중...");
        loadEngineModels();
        useChatStore.getState().loadProfiles();
        const { projects, selectProject } = useChatStore.getState();

        const lastKey = await getSetting<string>("lastProjectKey", "");
        let proj = lastKey ? projects.find((p) => p.key === lastKey) : null;
        if (!proj) proj = projects[0];
        // No auto-create: if no projects, show ProjectStartup instead
        if (proj) {
          setLoadingStep(`프로젝트 열기: ${proj.name ?? proj.key}...`);
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
      } catch (e) {
        // selectProject 등 실패해도 사용자가 빈 화면에 갇히지 않도록 항상 loaded 마킹.
        // 실제 에러는 console + toast (Toaster) 로 표면화 (caller 측 catch 가 처리).
        console.error("[init]", e);
      } finally {
        setLoaded(true);
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

  // 'tunaflow:open-settings' 이벤트 listener — 기존에 RuntimeStatusBar 안에
  // 있던 코드를 root level 로 끌어올림. ProjectStartup 화면에서도 동작해야 하기
  // 때문. (CommandPalette / roleAssignments / 본 컴포넌트의 Cmd+, 핸들러가
  // 디스패처 측.)
  useEffect(() => {
    const handler = (e: Event) => {
      const detail = (e as CustomEvent<{ section?: string }>).detail;
      setSettingsInitialSection(detail?.section);
      setSettingsOpen(true);
    };
    window.addEventListener("tunaflow:open-settings", handler);
    return () => window.removeEventListener("tunaflow:open-settings", handler);
  }, []);

  // Cmd+, (macOS) / Ctrl+, (Win/Linux) — 어디서든 Settings 열림.
  // - IME 충돌 방지: composing 중인 IME 입력은 무시 (Korean/Japanese 등).
  // - 일반 텍스트 입력 (textarea/input) focus 와 무관 — 사용자가 명시적으로
  //   "," 키와 modifier 를 함께 누른 경우에만 trigger 되므로 textarea 안에서
  //   ',' 를 그냥 입력하는 흐름과 충돌하지 않는다.
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "," && !e.altKey && !e.shiftKey) {
        // IME 조합 중인 키는 브라우저가 isComposing=true 로 표시.
        if ((e as any).isComposing) return;
        e.preventDefault();
        e.stopPropagation();
        window.dispatchEvent(new CustomEvent("tunaflow:open-settings"));
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  // macOS 메뉴 → Settings... 클릭 시 Rust 가 'tunaflow:menu-open-settings'
  // 이벤트를 emit. 같은 internal `tunaflow:open-settings` window event 로
  // 합류시켜 단일 처리 경로를 유지한다 (macOS 메뉴 / Cmd+, / Command Palette
  // / 톱니 버튼 모두 동일).
  useEffect(() => {
    const unlistenPromise = listen("tunaflow:menu-open-settings", () => {
      window.dispatchEvent(new CustomEvent("tunaflow:open-settings"));
    });
    return () => { unlistenPromise.then((u) => u()).catch(() => {}); };
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

  // 로딩 화면 — 사용자에게 "앱이 hang 됐는지 / 로딩 중인지" 즉시 알리는 splash.
  // Windows 첫 실행 + 기존 프로젝트 DB load 시 수십 초 걸릴 수 있어 spinner +
  // 단계 텍스트로 hang 인지 가시화. tauri.conf.json 의 visible: false 로 React
  // mount 전에는 창 자체가 안 보이고, mount 직후 본 splash 가 첫 화면.
  if (!loaded) return (
    <div className="fixed inset-0 bg-sidebar flex flex-col items-center justify-center gap-4 select-none">
      <div className="w-10 h-10 rounded-full border-2 border-primary/20 border-t-primary animate-spin" />
      <div className="text-foreground/60 text-[13px] font-medium">tunaFlow</div>
      <div className="text-foreground/40 text-[11px]">{loadingStep}</div>
    </div>
  );

  // Project-first startup: show selector if no project is selected.
  // SettingsPanel 은 ProjectStartup 화면에서도 Cmd+, / 메뉴로 열 수 있어야
  // 하므로 함께 mount. (RuntimeStatusBar 가 미렌더링 상태이기 때문)
  if (!selectedProjectKey) {
    return (
      <>
        <ProjectStartup />
        <Toaster position="bottom-right" theme={themeMode} richColors closeButton />
        {settingsOpen && (
          <SettingsPanel
            onClose={() => { setSettingsOpen(false); setSettingsInitialSection(undefined); }}
            initialSection={settingsInitialSection}
          />
        )}
      </>
    );
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
        {/* `min-h-0` 필수 — 누락 시 자식 (BranchThreadPanel 안의 NewMessageInput 등)
             이 길어지면 flex parent 가 viewport 밖으로 stretch 되어 메인 UI 전체가
             밀려 올라가는 현상 발생 (#191). */}
        <div className="flex-1 min-w-0 min-h-0 h-full relative flex">
          {/* Meta floating chat — anchored to top-left of main area */}
          {selectedProjectKey && (
            <MetaFloatingChat projectKey={selectedProjectKey} />
          )}

          {/* CenterPanel — full width when no drawer, or flex-1 when pinned.
               양쪽 className 이 동일해 삼항 의미 없음 — 단일 string 으로 정리하면서
               `min-h-0` 추가 (메인 area 와 동일 이유). */}
          <div className="flex-1 min-w-0 min-h-0 h-full">
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
      {settingsOpen && (
        <SettingsPanel
          onClose={() => { setSettingsOpen(false); setSettingsInitialSection(undefined); }}
          initialSection={settingsInitialSection}
        />
      )}
    </div>
    <CommandPalette />
    <ProjectOnboardingModal />
    </FileViewerContext.Provider>
  );
}
