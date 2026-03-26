import { useState } from "react";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { Waves, Search } from "lucide-react";
import type { Branch } from "@/types";

import { ProjectsSection } from "./sidebar/ProjectsSection";
import { ChatsSection } from "./sidebar/ChatsSection";
import { RoundtablesSection } from "./sidebar/RoundtablesSection";
import { BranchesSection } from "./sidebar/BranchesSection";
import { CreateRoundtableDialog } from "./CreateRoundtableDialog";
import { FilesSection } from "./sidebar/FilesSection";
import { AddProjectForm } from "./sidebar/AddProjectForm";
import { useProjectBranches } from "./sidebar/useProjectBranches";

export function Sidebar() {
  const {
    projects, selectedProjectKey, selectProject, createProject,
    conversations, selectedConversationId, selectConversation,
    createConversation, deleteConversation, renameConversation,
    branches: storeBranches, renameBranch,
    activeBranchId, threadBranchId, openThread, rawqStatus,
  } = useChatStore();

  const [searchQuery, setSearchQuery] = useState("");
  const [showAddProject, setShowAddProject] = useState(false);
  const [renameCounter, setRenameCounter] = useState(0);

  const [chatsOpen, setChatsOpen] = useState(true);
  const [rtOpen, setRtOpen] = useState(true);
  const [branchesOpen, setBranchesOpen] = useState(true);
  const [filesOpen, setFilesOpen] = useState(false);
  const [showCreateRT, setShowCreateRT] = useState(false);

  const currentProject = projects.find((p) => p.key === selectedProjectKey);

  // Current project data
  const chatConvs = conversations.filter((c) => !c.id.startsWith("branch:") && c.mode !== "roundtable");
  const rtConvs = conversations.filter((c) => !c.id.startsWith("branch:") && c.mode === "roundtable");
  const filteredChats = searchQuery.trim()
    ? chatConvs.filter((c) => (c.customLabel ?? c.label).toLowerCase().includes(searchQuery.toLowerCase()))
    : chatConvs;

  const allBranches = useProjectBranches(conversations, storeBranches, renameCounter);
  const rtBranches = allBranches.filter((b) => b.mode === "roundtable");
  const regularBranches = allBranches.filter((b) => b.mode !== "roundtable");
  const childMap = new Map<string, Branch[]>();
  for (const b of allBranches) { if (b.parentBranchId) { const a = childMap.get(b.parentBranchId) ?? []; a.push(b); childMap.set(b.parentBranchId, a); } }
  const topLevelRT = rtBranches.filter((b) => !b.parentBranchId);
  const topLevelBranches = regularBranches.filter((b) => !b.parentBranchId);

  const handleRenameBranch = async (branchId: string, newLabel: string) => {
    await renameBranch(branchId, newLabel);
    setRenameCounter((c) => c + 1);
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
    <aside className="flex flex-col w-full bg-sidebar h-full overflow-hidden text-sidebar-foreground">
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
        <ProjectsSection
          projects={projects}
          selectedProjectKey={selectedProjectKey}
          selectProject={selectProject}
        />

        {/* Below sections show data for the selected project only */}
        {selectedProjectKey && (
          <>
            <ChatsSection
              chatsOpen={chatsOpen}
              setChatsOpen={setChatsOpen}
              filteredChats={filteredChats}
              selectedConversationId={selectedConversationId}
              selectConversation={selectConversation}
              renameConversation={renameConversation}
              handleCreateChat={handleCreateChat}
              handleDelete={handleDelete}
            />

            <RoundtablesSection
              rtOpen={rtOpen}
              setRtOpen={setRtOpen}
              rtConvs={rtConvs}
              topLevelRT={topLevelRT}
              childMap={childMap}
              selectedConversationId={selectedConversationId}
              activeBranchId={activeBranchId}
              threadBranchId={threadBranchId}
              selectConversation={selectConversation}
              renameConversation={renameConversation}
              openThread={openThread}
              handleDelete={handleDelete}
              handleRenameBranch={handleRenameBranch}
              onCreateRT={() => setShowCreateRT(true)}
            />

            <BranchesSection
              branchesOpen={branchesOpen}
              setBranchesOpen={setBranchesOpen}
              topLevelBranches={topLevelBranches}
              childMap={childMap}
              activeBranchId={activeBranchId}
              threadBranchId={threadBranchId}
              openThread={openThread}
              handleRenameBranch={handleRenameBranch}
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
