import { useState } from "react";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { Waves, Search } from "lucide-react";
import type { Branch } from "@/types";

import { ProjectsSection } from "./sidebar/ProjectsSection";
import { ChatsSection } from "./sidebar/ChatsSection";
import { CreateRoundtableDialog } from "./CreateRoundtableDialog";
import { FilesSection } from "./sidebar/FilesSection";
import { AddProjectForm } from "./sidebar/AddProjectForm";
import { useProjectBranches } from "./sidebar/useProjectBranches";

export function Sidebar() {
  const projects = useChatStore((s) => s.projects);
  const selectedProjectKey = useChatStore((s) => s.selectedProjectKey);
  const selectProject = useChatStore((s) => s.selectProject);
  const createProject = useChatStore((s) => s.createProject);
  const conversations = useChatStore((s) => s.conversations);
  const selectedConversationId = useChatStore((s) => s.selectedConversationId);
  const selectConversation = useChatStore((s) => s.selectConversation);
  const createConversation = useChatStore((s) => s.createConversation);
  const deleteConversation = useChatStore((s) => s.deleteConversation);
  const renameConversation = useChatStore((s) => s.renameConversation);
  const storeBranches = useChatStore((s) => s.branches);
  const renameBranch = useChatStore((s) => s.renameBranch);
  const deleteBranch = useChatStore((s) => s.deleteBranch);
  const activeBranchId = useChatStore((s) => s.activeBranchId);
  const threadBranchId = useChatStore((s) => s.threadBranchId);
  const openThread = useChatStore((s) => s.openThread);
  const rawqStatus = useChatStore((s) => s.rawqStatus);
  const runningThreadIds = useChatStore((s) => s.runningThreadIds);
  const messageQueue = useChatStore((s) => s.messageQueue);

  const [searchQuery, setSearchQuery] = useState("");
  const [showAddProject, setShowAddProject] = useState(false);
  const [renameCounter, setRenameCounter] = useState(0);

  const [chatsOpen, setChatsOpen] = useState(true);
  const [filesOpen, setFilesOpen] = useState(false);
  const [showCreateRT, setShowCreateRT] = useState(false);

  const currentProject = projects.find((p) => p.key === selectedProjectKey);

  // Current project data
  const chatConvs = conversations.filter((c) => !c.id.startsWith("branch:") && c.mode !== "roundtable");
  const filteredChats = searchQuery.trim()
    ? chatConvs.filter((c) => (c.customLabel ?? c.label).toLowerCase().includes(searchQuery.toLowerCase()))
    : chatConvs;

  const allBranches = useProjectBranches(conversations, storeBranches, renameCounter);
  // Branch-to-branch child map (for nested branches)
  const childMap = new Map<string, Branch[]>();
  for (const b of allBranches) { if (b.parentBranchId) { const a = childMap.get(b.parentBranchId) ?? []; a.push(b); childMap.set(b.parentBranchId, a); } }
  // Top-level branches grouped by parent conversation
  const branchesByConv = new Map<string, Branch[]>();
  for (const b of allBranches) {
    if (b.parentBranchId) continue; // nested branches handled by childMap
    const a = branchesByConv.get(b.conversationId) ?? [];
    a.push(b);
    branchesByConv.set(b.conversationId, a);
  }

  const handleRenameBranch = async (branchId: string, newLabel: string) => {
    await renameBranch(branchId, newLabel);
    setRenameCounter((c) => c + 1);
  };

  const handleDeleteBranch = async (branchId: string, label: string) => {
    if (window.confirm(`"${label}" 브랜치를 삭제하시겠습니까?`)) {
      await deleteBranch(branchId);
      setRenameCounter((c) => c + 1); // trigger sidebar branch re-fetch
    }
  };

  const handleCreateChat = async (e: React.MouseEvent) => {
    e.stopPropagation();
    if (!selectedProjectKey) return;
    const convs = useChatStore.getState().conversations;
    const label = `Conversation ${convs.filter((c) => !c.id.startsWith("branch:")).length + 1}`;
    const conv = await createConversation({ projectKey: selectedProjectKey, label, type: "main", mode: "chat", source: "tunadish" });
    await selectConversation(conv.id);
  };

  const handleDelete = async (id: string, label: string, e: React.MouseEvent) => {
    e.stopPropagation();
    if (!window.confirm(`"${label}" 대화를 삭제하시겠습니까?`)) return;
    await deleteConversation(id);
  };

  return (
    <aside data-testid="sidebar" className="flex flex-col w-full bg-sidebar h-full overflow-hidden text-sidebar-foreground">
      {/* Logo */}
      <div className="flex items-center gap-2 px-3 h-9 border-b border-white/[0.06] shrink-0">
        <div className="w-5 h-5 rounded bg-primary/15 flex items-center justify-center">
          <Waves className="w-3 h-3 text-primary" />
        </div>
        <span className="font-medium text-[12px] tracking-tight">tunaFlow</span>
        <span className="ml-auto text-[8px] text-sidebar-foreground/35 bg-white/[0.04] px-1 py-0.5 rounded font-mono">beta</span>
      </div>

      {/* Search + Add project */}
      <div className="px-2 py-1.5 border-b border-white/[0.06] shrink-0 space-y-1">
        <div className="flex items-center gap-1.5 bg-white/[0.04] rounded px-2 py-[3px]">
          <Search className="w-3 h-3 text-sidebar-foreground/25" />
          <input className="flex-1 text-[11px] bg-transparent outline-none placeholder:text-sidebar-foreground/25"
            placeholder="Search…" value={searchQuery} onChange={(e) => setSearchQuery(e.target.value)} />
        </div>
        <AddProjectForm
          showAddProject={showAddProject}
          setShowAddProject={setShowAddProject}
          createProject={createProject}
          selectProject={selectProject}
        />
      </div>

      <nav className="flex-1 overflow-y-auto py-1">
        {/* PROJECTS */}
        {(() => {
          // Derive project-scoped concurrency status
          const currentConvIds = new Set(conversations.map((c) => c.id));
          const currentRunning = runningThreadIds.filter((id) => currentConvIds.has(id)).length;
          const currentQueued = messageQueue.filter((q) => currentConvIds.has(q.threadId)).length;
          const otherRunning = runningThreadIds.some((id) => !currentConvIds.has(id));
          return (
            <ProjectsSection
              projects={projects}
              selectedProjectKey={selectedProjectKey}
              selectProject={selectProject}
              runningCount={currentRunning}
              queuedCount={currentQueued}
              hasOtherRunning={otherRunning}
            />
          );
        })()}

        {/* Below sections show data for the selected project only */}
        {selectedProjectKey && (
          <>
            <ChatsSection
              chatsOpen={chatsOpen}
              setChatsOpen={setChatsOpen}
              filteredChats={filteredChats}
              selectedConversationId={selectedConversationId}
              activeBranchId={activeBranchId}
              threadBranchId={threadBranchId}
              selectConversation={selectConversation}
              renameConversation={renameConversation}
              handleCreateChat={handleCreateChat}
              handleDelete={handleDelete}
              branchesByConv={branchesByConv}
              childMap={childMap}
              openThread={openThread}
              handleRenameBranch={handleRenameBranch}
              onDeleteBranch={handleDeleteBranch}
              onCreateRT={() => setShowCreateRT(true)}
            />

            <FilesSection
              filesOpen={filesOpen}
              setFilesOpen={setFilesOpen}
              projectPath={currentProject?.path}
            />
          </>
        )}
      </nav>

      {/* rawq status */}
      {rawqStatus && (
        <div className="flex items-center justify-center gap-1.5 px-3 h-[24px] border-t border-white/[0.06] shrink-0 text-[10px] text-sidebar-foreground/50">
          <span className={cn("w-1.5 h-1.5 rounded-full shrink-0",
            rawqStatus.status === "ready" || rawqStatus.status === "built" ? "bg-status-approved"
            : rawqStatus.status === "indexing" ? "bg-primary animate-pulse"
            : rawqStatus.status === "unavailable" ? "bg-sidebar-foreground/15" : "bg-status-rejected"
          )} />
          <span className="truncate">{rawqStatus.message}</span>
        </div>
      )}
      <CreateRoundtableDialog open={showCreateRT} onClose={() => setShowCreateRT(false)} />
    </aside>
  );
}
