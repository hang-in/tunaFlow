import { useState, useRef, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useChatStore } from "@/stores/chatStore";
import { ChevronDown, ChevronRight, FolderOpen, Folder, Trash2, Loader2, GitBranch, Archive, Users, Plus, Pencil } from "lucide-react";
import { cn } from "@/lib/utils";
import { ask } from "@tauri-apps/plugin-dialog";
import type { Branch } from "@/types";

import { ScratchpadSection } from "./sidebar/ScratchpadSection";
import { SidebarContextMenu, type ContextMenuItem, type ContextMenuState } from "./sidebar/SidebarContextMenu";
import { CreateRoundtableDialog } from "./CreateRoundtableDialog";
import { DocsSection } from "./sidebar/DocsSection";
import { AddProjectForm } from "./sidebar/AddProjectForm";
import { useProjectBranches } from "./sidebar/useProjectBranches";
import { getSetting, setSetting } from "@/lib/appStore";

// ─── Types ───────────────────────────────────────────────────────────────────

type TopSectionKey = "branches" | "roundtables" | "scratchpad";
type RefTab = "docs" | "archive";

const TOP_SECTION_ORDER: TopSectionKey[] = ["branches", "roundtables", "scratchpad"];

const MIN_SECTION_HEIGHT = 60;
const MIN_AREA_HEIGHT = 80;
const DEFAULT_TOP_AREA_HEIGHT = 300;

const DEFAULT_HEIGHTS: Record<TopSectionKey, number> = {
  branches: 150,
  roundtables: 100,
  scratchpad: 100,
};

const DEFAULT_SECTION_STATE: Record<TopSectionKey, boolean> = {
  branches: true,
  roundtables: true,
  scratchpad: true,
};

// ─── CollapsibleSection ───────────────────────────────────────────────────────

function CollapsibleSection({
  title, count, expanded, onToggle, children, action, height, fillRemaining,
}: {
  title: string; count?: number; expanded: boolean;
  onToggle: () => void; children: React.ReactNode;
  action?: React.ReactNode; height?: number; fillRemaining?: boolean;
}) {
  const style = expanded
    ? fillRemaining
      ? { flex: 1, minHeight: MIN_SECTION_HEIGHT }
      : { height, flexShrink: 0 }
    : undefined;
  return (
    <div className="flex flex-col overflow-hidden" style={style}>
      <div className="shrink-0 flex items-center px-3 py-1">
        <button
          onClick={onToggle}
          className="flex items-center gap-1 text-[10px] font-semibold uppercase tracking-wider text-sidebar-foreground/55 hover:text-sidebar-foreground/75 transition-colors flex-1"
        >
          <ChevronRight className={cn("w-3 h-3 transition-transform", expanded && "rotate-90")} />
          <span>{title}</span>
          {!expanded && count != null && count > 0 && (
            <span className="text-[9px] text-sidebar-foreground/25 font-normal ml-1">({count})</span>
          )}
        </button>
        {action && <div className="shrink-0">{action}</div>}
      </div>
      {expanded && (
        <div className="flex-1 overflow-y-auto px-3 pb-1 min-h-0">
          {children}
        </div>
      )}
    </div>
  );
}

// ─── ResizeHandle ─────────────────────────────────────────────────────────────

function SectionResizeHandle({ onDrag, onDragEnd }: { onDrag: (delta: number) => void; onDragEnd: () => void }) {
  return (
    <div
      className="h-1.5 shrink-0 cursor-row-resize hover:bg-primary/20 active:bg-primary/30 transition-colors group"
      onMouseDown={(e) => {
        e.preventDefault();
        let lastY = e.clientY;
        const onMove = (ev: MouseEvent) => { onDrag(ev.clientY - lastY); lastY = ev.clientY; };
        const onUp = () => {
          document.removeEventListener("mousemove", onMove);
          document.removeEventListener("mouseup", onUp);
          document.body.style.cursor = "";
          onDragEnd();
        };
        document.addEventListener("mousemove", onMove);
        document.addEventListener("mouseup", onUp);
        document.body.style.cursor = "row-resize";
      }}
    >
      <div className="h-px bg-border/20 group-hover:bg-primary/30 transition-colors mx-3 mt-0.5" />
    </div>
  );
}


// ─── Sidebar ──────────────────────────────────────────────────────────────────

export function Sidebar() {
  const projects = useChatStore((s) => s.projects);
  const selectedProjectKey = useChatStore((s) => s.selectedProjectKey);
  const selectProject = useChatStore((s) => s.selectProject);
  const createProject = useChatStore((s) => s.createProject);
  const hideProject = useChatStore((s) => s.hideProject);
  const conversations = useChatStore((s) => s.conversations);
  const selectedConversationId = useChatStore((s) => s.selectedConversationId);
  const selectConversation = useChatStore((s) => s.selectConversation);
  const renameConversation = useChatStore((s) => s.renameConversation);
  const createConversation = useChatStore((s) => s.createConversation);
  const storeBranches = useChatStore((s) => s.branches);
  const threadBranchId = useChatStore((s) => s.threadBranchId);
  const openThread = useChatStore((s) => s.openThread);
  const runningThreadIds = useChatStore((s) => s.runningThreadIds);
  const deleteBranch = useChatStore((s) => s.deleteBranch);
  const renameBranch = useChatStore((s) => s.renameBranch);
  const [renameCounter] = useState(0);
  const [showCreateRT, setShowCreateRT] = useState(false);
  const [ctxMenu, setCtxMenu] = useState<ContextMenuState | null>(null);

  const openCtx = useCallback((e: React.MouseEvent, items: ContextMenuItem[]) => {
    e.preventDefault();
    e.stopPropagation();
    setCtxMenu({ x: e.clientX, y: e.clientY, items });
  }, []);

  // Top 3-section collapsed state (persisted)
  const [sectionState, setSectionState] = useState<Record<TopSectionKey, boolean>>(DEFAULT_SECTION_STATE);
  useEffect(() => {
    getSetting<Record<string, boolean>>("sidebarSections", DEFAULT_SECTION_STATE)
      .then((v) => setSectionState({
        branches: v.branches ?? DEFAULT_SECTION_STATE.branches,
        roundtables: v.roundtables ?? DEFAULT_SECTION_STATE.roundtables,
        scratchpad: v.scratchpad ?? DEFAULT_SECTION_STATE.scratchpad,
      }));
  }, []);
  const toggleSection = useCallback((key: TopSectionKey) => {
    setSectionState((prev) => {
      const next = { ...prev, [key]: !prev[key] };
      setSetting("sidebarSections", next);
      return next;
    });
  }, []);

  // Top section heights (persisted)
  const [sectionHeights, setSectionHeights] = useState<Record<TopSectionKey, number>>(DEFAULT_HEIGHTS);
  const sectionHeightsRef = useRef(sectionHeights);
  sectionHeightsRef.current = sectionHeights;
  useEffect(() => {
    getSetting<Record<string, number>>("sidebarSectionHeights", DEFAULT_HEIGHTS)
      .then((v) => setSectionHeights({
        branches: v.branches ?? DEFAULT_HEIGHTS.branches,
        roundtables: v.roundtables ?? DEFAULT_HEIGHTS.roundtables,
        scratchpad: v.scratchpad ?? DEFAULT_HEIGHTS.scratchpad,
      }));
  }, []);
  const persistHeights = useCallback(() => {
    setSetting("sidebarSectionHeights", sectionHeightsRef.current);
  }, []);

  const adjustTwoHeights = useCallback((topKey: TopSectionKey, bottomKey: TopSectionKey, delta: number) => {
    setSectionHeights((prev) => {
      const newTop = Math.max(MIN_SECTION_HEIGHT, prev[topKey] + delta);
      const actualDelta = newTop - prev[topKey];
      const newBottom = Math.max(MIN_SECTION_HEIGHT, prev[bottomKey] - actualDelta);
      return { ...prev, [topKey]: newTop, [bottomKey]: newBottom };
    });
  }, []);

  // Top/bottom area split height (persisted)
  const [topAreaHeight, setTopAreaHeight] = useState(DEFAULT_TOP_AREA_HEIGHT);
  const topAreaHeightRef = useRef(topAreaHeight);
  topAreaHeightRef.current = topAreaHeight;
  useEffect(() => {
    getSetting<number>("sidebarTopAreaHeight", DEFAULT_TOP_AREA_HEIGHT)
      .then((v) => setTopAreaHeight(v));
  }, []);
  const persistTopAreaHeight = useCallback(() => {
    setSetting("sidebarTopAreaHeight", topAreaHeightRef.current);
  }, []);

  // Bottom ref tab (persisted)
  const [refTab, setRefTab] = useState<RefTab>("docs");
  useEffect(() => {
    getSetting<string>("sidebarRefTab", "docs").then((v) =>
      setRefTab(v === "archive" ? "archive" : "docs")
    );
  }, []);
  const switchRefTab = useCallback((tab: RefTab) => {
    setRefTab(tab);
    setSetting("sidebarRefTab", tab);
  }, []);

  // Last expanded section fills remaining top area
  const lastExpanded = [...TOP_SECTION_ORDER].reverse().find((k) => sectionState[k]) ?? null;

  // Project dropdown
  const [projectDropdownOpen, setProjectDropdownOpen] = useState(false);
  const [showAddProject, setShowAddProject] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!projectDropdownOpen) return;
    const handler = (e: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
        setProjectDropdownOpen(false);
        setShowAddProject(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [projectDropdownOpen]);

  const currentProject = projects.find((p) => p.key === selectedProjectKey);

  // Git status
  const [gitBranch, setGitBranch] = useState<string | null>(null);
  const [gitDirty, setGitDirty] = useState(false);
  useEffect(() => {
    if (!currentProject?.path) { setGitBranch(null); return; }
    invoke<{ isRepo: boolean; branch: string | null; dirty: boolean }>("get_git_status", { projectPath: currentProject.path })
      .then((s) => { setGitBranch(s.isRepo ? s.branch : null); setGitDirty(s.dirty); })
      .catch(() => setGitBranch(null));
  }, [currentProject?.path]);

  // Data
  const scratchpads = conversations.filter((c) => c.type === "scratchpad");
  const activeChatBranches = storeBranches.filter((b) => b.status === "active" && b.mode !== "roundtable");
  const activeRTBranches = storeBranches.filter((b) => b.status === "active" && b.mode === "roundtable");
  const archivedBranches = storeBranches.filter((b) => b.status === "archived" || b.status === "adopted");

  useProjectBranches(conversations, storeBranches, renameCounter);

  // Concurrency status
  const currentConvIds = new Set(conversations.map((c) => c.id));
  const currentRunning = runningThreadIds.filter((id) => currentConvIds.has(id)).length;

  // Helper: resize handle between two adjacent top sections
  function ResizeHandleBetween({ top, bottom }: { top: TopSectionKey; bottom: TopSectionKey }) {
    if (!sectionState[top] || !sectionState[bottom]) return null;
    if (lastExpanded === top) return null;
    return (
      <SectionResizeHandle
        onDrag={(d) => adjustTwoHeights(top, bottom, d)}
        onDragEnd={persistHeights}
      />
    );
  }

  const handleSidebarCtx = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    // Only show menu when clicking on empty space (not on a button/input/a)
    const tag = (e.target as HTMLElement).tagName.toLowerCase();
    if (["button", "input", "a", "svg", "path", "span"].includes(tag)) return;
    if (!selectedProjectKey) return;
    setCtxMenu({
      x: e.clientX,
      y: e.clientY,
      items: [
        {
          label: "새 Scratchpad",
          icon: <Plus className="w-3.5 h-3.5" />,
          onClick: async () => {
            const scratchCount = conversations.filter((c) => c.type === "scratchpad").length;
            const conv = await createConversation({
              projectKey: selectedProjectKey,
              label: `Scratch ${scratchCount + 1}`,
              type: "scratchpad",
              mode: "chat",
              source: "user",
            });
            if (conv) selectConversation(conv.id);
          },
        },
      ],
    });
  }, [selectedProjectKey, conversations, createConversation, selectConversation]);

  return (
    <aside
      data-testid="sidebar"
      role="navigation"
      aria-label="프로젝트 사이드바"
      className="flex flex-col w-full bg-sidebar h-full overflow-hidden text-sidebar-foreground"
      onContextMenu={handleSidebarCtx}
    >
      {/* Traffic light spacer */}
      <div className="h-[28px] shrink-0" />

      {/* Project selector */}
      <div className="px-2 py-1 shrink-0 relative" ref={dropdownRef}>
        <button
          onClick={() => setProjectDropdownOpen((v) => !v)}
          className="w-full flex items-center gap-2 px-2 py-1.5 rounded-md bg-sidebar-accent/50 hover:bg-sidebar-accent transition-colors"
        >
          <FolderOpen className="w-3.5 h-3.5 text-primary shrink-0" />
          <span className="flex-1 text-tf-base font-[550] tracking-[-0.1px] text-sidebar-accent-foreground truncate text-left">
            {currentProject?.name ?? "Select project"}
          </span>
          {gitBranch && (
            <span className="flex items-center gap-0.5 text-[9px] text-sidebar-foreground/30 font-mono shrink-0">
              <GitBranch className="w-2.5 h-2.5" />
              {gitBranch}
              {gitDirty && <span className="text-status-draft ml-1">●</span>}
            </span>
          )}
          {currentRunning > 0 && (
            <Loader2 className="w-3 h-3 animate-spin text-primary/70 shrink-0" />
          )}
          <ChevronDown className="w-3 h-3 text-sidebar-foreground/40 shrink-0" />
        </button>

        {projectDropdownOpen && (
          <div className="absolute left-2 right-2 top-full mt-0.5 z-50 bg-popover border border-border/40 rounded-lg shadow-xl overflow-hidden">
            <div className="max-h-[200px] overflow-y-auto py-1">
              {projects.map((p) => {
                const isSelected = p.key === selectedProjectKey;
                return (
                  <div key={p.key} className="group flex items-center">
                    <button
                      onClick={() => { selectProject(p.key); setProjectDropdownOpen(false); }}
                      className={`flex-1 flex items-center gap-2 px-3 py-1.5 text-tf-caption font-medium text-left transition-colors ${
                        isSelected ? "bg-accent text-foreground" : "text-foreground/70 hover:bg-accent/50"
                      }`}
                    >
                      {isSelected
                        ? <FolderOpen className="w-3.5 h-3.5 text-primary shrink-0" />
                        : <Folder className="w-3.5 h-3.5 text-muted-foreground/40 shrink-0" />}
                      <span className="truncate">{p.name}</span>
                      {p.path && (
                        <span className="ml-auto text-[9px] text-muted-foreground/30 truncate max-w-[60px]">
                          {p.path.split(/[\\/]/).pop()}
                        </span>
                      )}
                    </button>
                    <button
                      onClick={async (e) => {
                        e.stopPropagation();
                        const yes = await ask(`"${p.name}" 프로젝트를 삭제하시겠습니까?\n(프로젝트 데이터는 보존되며, 같은 경로로 다시 추가할 수 있습니다)`, { title: "프로젝트 삭제", kind: "warning" });
                        if (yes) {
                          hideProject(p.key);
                          if (projects.length <= 1) setProjectDropdownOpen(false);
                        }
                      }}
                      className="shrink-0 p-1.5 text-muted-foreground/20 hover:text-destructive opacity-0 group-hover:opacity-100 transition-all"
                      title="Delete project"
                    >
                      <Trash2 className="w-3 h-3" />
                    </button>
                  </div>
                );
              })}
            </div>
            <div className="border-t border-border/30 px-2 py-1.5">
              <AddProjectForm
                showAddProject={showAddProject}
                setShowAddProject={setShowAddProject}
                createProject={async (input) => {
                  await createProject(input);
                  setProjectDropdownOpen(false);
                  setShowAddProject(false);
                }}
                selectProject={selectProject}
              />
            </div>
          </div>
        )}
      </div>

      {/* 2-panel workspace */}
      {selectedProjectKey ? (
        <div className="flex-1 flex flex-col overflow-hidden min-h-0">

          {/* ── Top: Work area (Branches / Roundtables / Scratchpad) ── */}
          <div className="flex flex-col overflow-hidden shrink-0" style={{ height: topAreaHeight }}>

            {/* 1. Branches */}
            <CollapsibleSection
              title="Branches"
              count={activeChatBranches.length}
              expanded={sectionState.branches}
              onToggle={() => toggleSection("branches")}
              height={sectionHeights.branches}
              fillRemaining={lastExpanded === "branches"}
            >
              {activeChatBranches.length === 0 ? (
                <p className="text-[10px] text-sidebar-foreground/25 italic py-1">No active branches</p>
              ) : (
                activeChatBranches.map((b) => (
                  <button key={b.id} onClick={() => openThread(b.id)}
                    onContextMenu={(e) => openCtx(e, [
                      { label: "이름 변경", icon: <Pencil className="w-3.5 h-3.5" />, onClick: () => {
                        const val = window.prompt("새 이름을 입력하세요", b.customLabel ?? b.label ?? "");
                        if (val) renameBranch(b.id, val);
                      }},
                      { separator: true, label: "", onClick: () => {} },
                      { label: "아카이브", icon: <Archive className="w-3.5 h-3.5" />, onClick: async () => {
                        const yes = await ask("이 브랜치를 아카이브하시겠습니까?", { title: "브랜치 아카이브", kind: "warning" });
                        if (yes) invoke("archive_branch", { id: b.id }).catch(console.error);
                      }},
                      { label: "삭제", icon: <Trash2 className="w-3.5 h-3.5" />, danger: true, onClick: async () => {
                        const yes = await ask("이 브랜치를 삭제하시겠습니까?", { title: "브랜치 삭제", kind: "warning" });
                        if (yes) deleteBranch(b.id);
                      }},
                    ])}
                    className={cn(
                      "w-full flex items-center gap-1.5 px-1 py-0.5 text-[11px] rounded transition-colors group select-none",
                      b.id === threadBranchId
                        ? "bg-sidebar-accent text-sidebar-foreground"
                        : "text-sidebar-foreground/60 hover:text-sidebar-foreground hover:bg-sidebar-accent/40"
                    )}
                  >
                    <GitBranch className="w-3 h-3 shrink-0" />
                    <span className="truncate flex-1 text-left">{b.customLabel ?? b.label}</span>
                    {runningThreadIds.includes(`branch:${b.id}`) && (
                      <Loader2 className="w-3 h-3 animate-spin text-primary/70 shrink-0" />
                    )}
                  </button>
                ))
              )}
            </CollapsibleSection>

            <ResizeHandleBetween top="branches" bottom="roundtables" />

            {/* 2. Roundtables */}
            <CollapsibleSection
              title="Roundtables"
              count={activeRTBranches.length}
              expanded={sectionState.roundtables}
              onToggle={() => toggleSection("roundtables")}
              height={sectionHeights.roundtables}
              fillRemaining={lastExpanded === "roundtables"}
              action={
                <button onClick={() => setShowCreateRT(true)}
                  className="p-0.5 rounded text-sidebar-foreground/25 hover:text-agent-gemini transition-colors"
                  title="New roundtable">
                  <Plus className="w-3 h-3" />
                </button>
              }
            >
              {activeRTBranches.length === 0 ? (
                <p className="text-[10px] text-sidebar-foreground/25 italic py-1">No roundtables</p>
              ) : (
                activeRTBranches.map((b) => (
                  <button key={b.id} onClick={() => openThread(b.id)}
                    onContextMenu={(e) => openCtx(e, [
                      { label: "이름 변경", icon: <Pencil className="w-3.5 h-3.5" />, onClick: () => {
                        const val = window.prompt("새 이름을 입력하세요", b.customLabel ?? b.label ?? "");
                        if (val) renameBranch(b.id, val);
                      }},
                      { separator: true, label: "", onClick: () => {} },
                      { label: "삭제", icon: <Trash2 className="w-3.5 h-3.5" />, danger: true, onClick: async () => {
                        const yes = await ask("이 Roundtable을 삭제하시겠습니까?", { title: "RT 삭제", kind: "warning" });
                        if (yes) deleteBranch(b.id);
                      }},
                    ])}
                    className={cn(
                      "w-full flex items-center gap-1.5 px-1 py-0.5 text-[11px] rounded transition-colors select-none",
                      b.id === threadBranchId
                        ? "bg-sidebar-accent text-sidebar-foreground"
                        : "text-sidebar-foreground/60 hover:text-sidebar-foreground hover:bg-sidebar-accent/40"
                    )}
                  >
                    <Users className="w-3 h-3 shrink-0 text-agent-gemini/50" />
                    <span className="truncate flex-1 text-left">{b.customLabel ?? b.label}</span>
                  </button>
                ))
              )}
            </CollapsibleSection>

            <ResizeHandleBetween top="roundtables" bottom="scratchpad" />

            {/* 3. Scratchpad */}
            <CollapsibleSection
              title="Scratchpad"
              count={scratchpads.length}
              expanded={sectionState.scratchpad}
              onToggle={() => toggleSection("scratchpad")}
              height={sectionHeights.scratchpad}
              fillRemaining={lastExpanded === "scratchpad"}
            >
              <ScratchpadSection
                scratchpads={scratchpads}
                selectedConversationId={selectedConversationId}
                selectConversation={selectConversation}
                renameConversation={renameConversation}
              />
            </CollapsibleSection>

          </div>

          {/* ── Bottom: Reference area (Docs / Artifacts / Archive) ── */}
          {/* Top edge of this area IS the resize handle (replaces PanelDivider) */}
          <div className="flex-1 flex flex-col overflow-hidden min-h-0 bg-sidebar-accent/50 rounded-lg mx-1 mb-1">
            {/* Drag edge at top — invisible until hover */}
            <div
              className="h-2 shrink-0 cursor-row-resize group flex items-center justify-center"
              onMouseDown={(e) => {
                e.preventDefault();
                let lastY = e.clientY;
                const onMove = (ev: MouseEvent) => {
                  const delta = ev.clientY - lastY;
                  lastY = ev.clientY;
                  setTopAreaHeight((h) => {
                    const next = Math.max(MIN_AREA_HEIGHT, h + delta);
                    topAreaHeightRef.current = next;
                    return next;
                  });
                };
                const onUp = () => {
                  document.removeEventListener("mousemove", onMove);
                  document.removeEventListener("mouseup", onUp);
                  document.body.style.cursor = "";
                  persistTopAreaHeight();
                };
                document.addEventListener("mousemove", onMove);
                document.addEventListener("mouseup", onUp);
                document.body.style.cursor = "row-resize";
              }}
            >
              <div className="w-6 h-0.5 rounded-full bg-transparent group-hover:bg-sidebar-foreground/25 transition-colors" />
            </div>
            {/* Tab bar — px-2 so content aligns with top area's px-3 (mx-1 outer + px-2 inner = 12px) */}
            <div className="shrink-0 flex items-center px-2 pb-0.5 gap-0.5">
              {(["docs", "archive"] as RefTab[]).map((tab) => (
                <button
                  key={tab}
                  onClick={() => switchRefTab(tab)}
                  className={cn(
                    "text-[9px] font-semibold uppercase tracking-wider px-2 py-0.5 rounded transition-colors",
                    refTab === tab
                      ? "bg-sidebar-accent text-sidebar-foreground"
                      : "text-sidebar-foreground/40 hover:text-sidebar-foreground/60 hover:bg-sidebar-accent/40"
                  )}
                >
                  {tab === "docs" ? "Docs" : "Archive"}
                </button>
              ))}
            </div>

            {/* Tab content — px-2 to align with top area (mx-1 + px-2 = 12px from edge) */}
            <div className="flex-1 overflow-y-auto px-2 pb-2 min-h-0">
              {refTab === "docs" && (
                <DocsSection projectPath={currentProject?.path} />
              )}
              {refTab === "archive" && (
                archivedBranches.length === 0 ? (
                  <p className="text-[10px] text-sidebar-foreground/25 italic py-2">No archived branches</p>
                ) : (
                  archivedBranches.map((b) => (
                    <button key={b.id} onClick={() => openThread(b.id)}
                      onContextMenu={(e) => openCtx(e, [
                        { label: "삭제", icon: <Trash2 className="w-3.5 h-3.5" />, danger: true, onClick: async () => {
                          const yes = await ask("이 아카이브를 삭제하시겠습니까?", { title: "아카이브 삭제", kind: "warning" });
                          if (yes) deleteBranch(b.id);
                        }},
                      ])}
                      className={cn(
                        "w-full flex items-center gap-1.5 px-1 py-0.5 text-[11px] rounded transition-colors select-none",
                        b.id === threadBranchId
                          ? "bg-sidebar-accent text-sidebar-foreground"
                          : "text-sidebar-foreground/35 hover:text-sidebar-foreground/60 hover:bg-sidebar-accent/40"
                      )}
                    >
                      <Archive className="w-3 h-3 shrink-0" />
                      <span className="truncate">{b.customLabel ?? b.label}</span>
                    </button>
                  ))
                )
              )}
            </div>
          </div>

        </div>
      ) : (
        <nav className="flex-1" />
      )}

      <CreateRoundtableDialog open={showCreateRT} onClose={() => setShowCreateRT(false)} />
      {ctxMenu && <SidebarContextMenu menu={ctxMenu} onClose={() => setCtxMenu(null)} />}
    </aside>
  );
}
