import { useState, useRef, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useChatStore } from "@/stores/chatStore";
import { Waves, ChevronDown, FolderOpen, Folder, Trash2, Loader2, Settings, GitBranch } from "lucide-react";
import { ask } from "@tauri-apps/plugin-dialog";
import type { Branch } from "@/types";

import { ChatsSection } from "./sidebar/ChatsSection";
import { ScratchpadSection } from "./sidebar/ScratchpadSection";
import { CreateRoundtableDialog } from "./CreateRoundtableDialog";
import { FilesSection } from "./sidebar/FilesSection";
import { AddProjectForm } from "./sidebar/AddProjectForm";
import { useProjectBranches } from "./sidebar/useProjectBranches";
import { SettingsPanel } from "./SettingsPanel";

export function Sidebar() {
  const projects = useChatStore((s) => s.projects);
  const selectedProjectKey = useChatStore((s) => s.selectedProjectKey);
  const selectProject = useChatStore((s) => s.selectProject);
  const createProject = useChatStore((s) => s.createProject);
  const hideProject = useChatStore((s) => s.hideProject);
  const conversations = useChatStore((s) => s.conversations);
  const selectedConversationId = useChatStore((s) => s.selectedConversationId);
  const selectConversation = useChatStore((s) => s.selectConversation);
  const deleteConversation = useChatStore((s) => s.deleteConversation);
  const renameConversation = useChatStore((s) => s.renameConversation);
  const storeBranches = useChatStore((s) => s.branches);
  const renameBranch = useChatStore((s) => s.renameBranch);
  const deleteBranch = useChatStore((s) => s.deleteBranch);
  const activeBranchId = useChatStore((s) => s.activeBranchId);
  const threadBranchId = useChatStore((s) => s.threadBranchId);
  const openThread = useChatStore((s) => s.openThread);
  const runningThreadIds = useChatStore((s) => s.runningThreadIds);
  const messageQueue = useChatStore((s) => s.messageQueue);
  const [renameCounter, setRenameCounter] = useState(0);
  const [filesOpen, setFilesOpen] = useState(false);
  const [showCreateRT, setShowCreateRT] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);

  // Project dropdown state
  const [projectDropdownOpen, setProjectDropdownOpen] = useState(false);
  const [showAddProject, setShowAddProject] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);

  // Close dropdown on outside click
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

  // Current project data
  const chatConvs = conversations.filter((c) => !c.id.startsWith("branch:") && c.mode !== "roundtable" && c.type !== "scratchpad");
  const scratchpads = conversations.filter((c) => c.type === "scratchpad");

  const allBranches = useProjectBranches(conversations, storeBranches, renameCounter);
  const childMap = new Map<string, Branch[]>();
  for (const b of allBranches) { if (b.parentBranchId) { const a = childMap.get(b.parentBranchId) ?? []; a.push(b); childMap.set(b.parentBranchId, a); } }
  const branchesByConv = new Map<string, Branch[]>();
  for (const b of allBranches) {
    if (b.parentBranchId) continue;
    const a = branchesByConv.get(b.conversationId) ?? [];
    a.push(b);
    branchesByConv.set(b.conversationId, a);
  }

  // Concurrency status
  const currentConvIds = new Set(conversations.map((c) => c.id));
  const currentRunning = runningThreadIds.filter((id) => currentConvIds.has(id)).length;

  const handleRenameBranch = async (branchId: string, newLabel: string) => {
    await renameBranch(branchId, newLabel);
    setRenameCounter((c) => c + 1);
  };

  const handleDeleteBranch = async (branchId: string, label: string) => {
    // Check if any descendant branches are adopted/archived
    const allBr = useChatStore.getState().branches;
    const descendants: string[] = [];
    const queue = [branchId];
    while (queue.length) {
      const id = queue.shift()!;
      const children = allBr.filter((b) => b.parentBranchId === id);
      for (const c of children) { descendants.push(c.id); queue.push(c.id); }
    }
    const hasAdopted = allBr.some((b) => descendants.includes(b.id) && (b.status === "adopted" || b.status === "archived"));

    const message = hasAdopted
      ? `"${label}" 브랜치에 채택된 결과가 포함되어 있습니다.\n하위 브랜치와 이력이 모두 삭제됩니다. 계속하시겠습니까?`
      : `"${label}" 브랜치를 삭제하시겠습니까?`;

    const yes = await ask(message, { title: "브랜치 삭제", kind: "warning" });
    if (yes) {
      await deleteBranch(branchId);
      setRenameCounter((c) => c + 1);
    }
  };

  const handleDelete = async (id: string, label: string, e: React.MouseEvent) => {
    e.stopPropagation();
    const yes = await ask(`"${label}" 대화를 삭제하시겠습니까?`, { title: "대화 삭제", kind: "warning" });
    if (!yes) return;
    await deleteConversation(id);
  };

  return (
    <aside data-testid="sidebar" className="flex flex-col w-full bg-sidebar h-full overflow-hidden text-sidebar-foreground">
      {/* Logo */}
      <div className="flex items-center gap-2 px-3 h-[52px] shrink-0">
        <div className="w-5 h-5 rounded bg-primary/15 flex items-center justify-center">
          <Waves className="w-3 h-3 text-primary" />
        </div>
        <span className="font-medium text-[14px] tracking-[-0.1px] text-sidebar-accent-foreground">tunaFlow</span>
        <span className="ml-auto text-[8px] text-sidebar-foreground/35 bg-white/[0.04] px-1 py-0.5 rounded font-mono">beta</span>
      </div>

      {/* Project selector dropdown */}
      <div className="px-2 py-1 shrink-0 relative" ref={dropdownRef}>
        <button
          onClick={() => setProjectDropdownOpen((v) => !v)}
          className="w-full flex items-center gap-2 px-2 py-1.5 rounded-md bg-sidebar-accent/50 hover:bg-sidebar-accent transition-colors"
        >
          <FolderOpen className="w-3.5 h-3.5 text-primary shrink-0" />
          <span className="flex-1 text-[14px] font-[550] tracking-[-0.1px] text-sidebar-accent-foreground truncate text-left">
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

        {/* Dropdown menu */}
        {projectDropdownOpen && (
          <div className="absolute left-2 right-2 top-full mt-0.5 z-50 bg-popover border border-border/40 rounded-lg shadow-xl overflow-hidden">
            {/* Project list */}
            <div className="max-h-[200px] overflow-y-auto py-1">
              {projects.map((p) => {
                const isSelected = p.key === selectedProjectKey;
                return (
                  <div key={p.key} className="group flex items-center">
                    <button
                      onClick={() => { selectProject(p.key); setProjectDropdownOpen(false); }}
                      className={`flex-1 flex items-center gap-2 px-3 py-1.5 text-[13px] font-medium text-left transition-colors ${
                        isSelected ? "bg-accent text-foreground" : "text-foreground/70 hover:bg-accent/50"
                      }`}
                    >
                      {isSelected
                        ? <FolderOpen className="w-3.5 h-3.5 text-primary shrink-0" />
                        : <Folder className="w-3.5 h-3.5 text-muted-foreground/40 shrink-0" />}
                      <span className="truncate">{p.name}</span>
                      {p.path && (
                        <span className="ml-auto text-[8px] text-muted-foreground/30 truncate max-w-[60px]">
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

            {/* Add project */}
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

      {/* Workspace tree */}
      <nav className="flex-1 overflow-y-auto py-1 px-3">
        {selectedProjectKey && (
          <>
            <ChatsSection
              filteredChats={chatConvs}
              selectedConversationId={selectedConversationId}
              activeBranchId={activeBranchId}
              threadBranchId={threadBranchId}
              selectConversation={selectConversation}
              renameConversation={renameConversation}
              handleDelete={handleDelete}
              branchesByConv={branchesByConv}
              childMap={childMap}
              openThread={openThread}
              handleRenameBranch={handleRenameBranch}
              onDeleteBranch={handleDeleteBranch}
              onCreateRT={() => setShowCreateRT(true)}
            />

            <ScratchpadSection
              scratchpads={scratchpads}
              selectedConversationId={selectedConversationId}
              selectConversation={selectConversation}
              renameConversation={renameConversation}
            />

            <FilesSection
              filesOpen={filesOpen}
              setFilesOpen={setFilesOpen}
              projectPath={currentProject?.path}
            />
          </>
        )}
      </nav>

      {/* Settings button — bottom left */}
      <div className="shrink-0 px-3 py-2">
        <button
          onClick={() => setSettingsOpen(true)}
          className="p-1.5 rounded-lg text-muted-foreground/50 hover:text-foreground hover:bg-sidebar-accent/50 transition-colors"
          title="Settings"
        >
          <Settings className="w-4 h-4" />
        </button>
      </div>

      <CreateRoundtableDialog open={showCreateRT} onClose={() => setShowCreateRT(false)} />
      {settingsOpen && <SettingsPanel onClose={() => setSettingsOpen(false)} />}
    </aside>
  );
}
