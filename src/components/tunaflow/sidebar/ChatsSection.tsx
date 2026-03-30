import { useState, useMemo } from "react";
import { MessageSquare, Trash2, GitBranch, Users, ChevronRight, ChevronDown, Archive } from "lucide-react";
import { cn } from "@/lib/utils";
import { TreeRow } from "./TreeRow";
import { InlineRename } from "../InlineRename";
import type { Conversation, Branch } from "@/types";

// Status dot colors
const STATUS_DOT: Record<string, string> = {
  active: "bg-primary",
  adopted: "bg-status-approved",
  archived: "bg-sidebar-foreground/25",
};

interface ChatsSectionProps {
  filteredChats: Conversation[];
  selectedConversationId: string | null;
  activeBranchId: string | null;
  threadBranchId: string | null;
  selectConversation: (id: string) => void;
  renameConversation: (id: string, label: string) => Promise<void>;
  handleDelete: (id: string, label: string, e: React.MouseEvent) => void;
  branchesByConv: Map<string, Branch[]>;
  childMap: Map<string, Branch[]>;
  openThread: (branchId: string) => void;
  handleRenameBranch: (branchId: string, newLabel: string) => Promise<void>;
  onDeleteBranch: (branchId: string, label: string) => void;
  onCreateRT?: () => void;
}

export function ChatsSection({
  filteredChats, selectedConversationId,
  activeBranchId, threadBranchId,
  selectConversation, renameConversation, handleDelete,
  branchesByConv, childMap, openThread, handleRenameBranch, onDeleteBranch, onCreateRT,
}: ChatsSectionProps) {
  // Build set of branch IDs that should be expanded (ancestors of active thread)
  const expandedBranchIds = useMemo(() => {
    const set = new Set<string>();
    if (!threadBranchId) return set;
    const allBranches = [...(childMap.entries())].flatMap(([, children]) => children);
    for (const [, branches] of branchesByConv) {
      allBranches.push(...branches);
    }
    let currentId: string | undefined = threadBranchId;
    while (currentId) {
      set.add(currentId);
      const branch = allBranches.find((b) => b.id === currentId);
      currentId = branch?.parentBranchId ?? undefined;
    }
    return set;
  }, [threadBranchId, childMap, branchesByConv]);

  // Collect all adopted/archived branches across all conversations
  const allBranchesList = useMemo(() => {
    const list: Branch[] = [];
    for (const [, branches] of branchesByConv) list.push(...branches);
    for (const [, children] of childMap) list.push(...children);
    // Deduplicate
    const seen = new Set<string>();
    return list.filter((b) => { if (seen.has(b.id)) return false; seen.add(b.id); return true; });
  }, [branchesByConv, childMap]);

  const completedBranches = allBranchesList.filter((b) => b.status === "adopted" || b.status === "archived");

  const renderBranch = (b: Branch, depth: number) => {
    const isActive = b.id === activeBranchId || b.id === threadBranchId;
    const isRT = b.mode === "roundtable";
    const children = childMap.get(b.id) ?? [];
    // Only show active children in the main tree
    const activeChildren = children.filter((c) => c.status === "active");
    const hasChildren = activeChildren.length > 0;
    const isExpanded = expandedBranchIds.has(b.id);

    return (
      <div key={b.id}>
        <TreeRow depth={depth} active={isActive} isParent={hasChildren}
          icon={isRT
            ? <Users className="w-3.5 h-3.5 text-agent-gemini/40" />
            : hasChildren
              ? (isExpanded
                ? <ChevronDown className="w-3.5 h-3.5" />
                : <ChevronRight className="w-3.5 h-3.5" />)
              : <GitBranch className="w-3.5 h-3.5" />}
          label={
            <span className="flex items-center gap-1">
              <InlineRename value={b.customLabel ?? b.label} onSave={(v) => handleRenameBranch(b.id, v)} inputClassName="text-[10px] w-full" />
              {isRT && <span className="text-[7px] text-agent-gemini/40 shrink-0">RT</span>}
              {b.gitBranch && <span className="text-[7px] text-sidebar-foreground/20 font-mono shrink-0 truncate max-w-[60px]">{b.gitBranch}</span>}
            </span>
          }
          suffix={
            <span className={cn("w-1.5 h-1.5 rounded-full shrink-0 mr-1", STATUS_DOT[b.status] ?? STATUS_DOT.active)} />
          }
          actions={b.status === "active" ? (
            <button onClick={(e) => { e.stopPropagation(); onDeleteBranch(b.id, b.customLabel ?? b.label); }}
              className="p-0.5 rounded text-sidebar-foreground/20 hover:text-destructive transition-colors" title="Delete">
              <Trash2 className="w-3 h-3" />
            </button>
          ) : undefined}
          onClick={() => openThread(b.id)} />
        {isExpanded && activeChildren.map((child) => renderBranch(child, depth + 1))}
      </div>
    );
  };

  return (
    <>
      {/* Active chat tree */}
      {filteredChats.map((conv) => {
        const isActive = conv.id === selectedConversationId;
        const convBranches = (branchesByConv.get(conv.id) ?? []).filter((b) => !b.parentBranchId && b.status === "active");
        const hasChildren = convBranches.length > 0;
        const isExpanded = isActive || expandedBranchIds.size > 0;
        return (
          <div key={conv.id}>
            <TreeRow depth={0} active={isActive} isParent={hasChildren}
              icon={
                hasChildren ? (
                  isExpanded
                    ? <ChevronDown className="w-3.5 h-3.5" />
                    : <ChevronRight className="w-3.5 h-3.5" />
                ) : <MessageSquare className="w-3.5 h-3.5" />
              }
              label={
                <InlineRename
                  value={conv.customLabel ?? conv.label === "Main" ? "Chat" : conv.customLabel ?? conv.label}
                  onSave={(v) => renameConversation(conv.id, v)}
                  inputClassName="text-[10px] w-full"
                />
              }
              suffix={
                onCreateRT ? (
                  <button onClick={(e) => { e.stopPropagation(); onCreateRT(); }}
                    className="p-0.5 rounded text-sidebar-foreground/30 hover:text-agent-gemini hover:bg-agent-gemini/10 transition-colors shrink-0 mr-0.5"
                    title="New roundtable">
                    <Users className="w-3 h-3" />
                  </button>
                ) : undefined
              }
              actions={filteredChats.length > 1 ? (
                <button onClick={(e) => handleDelete(conv.id, conv.customLabel ?? conv.label, e)} className="p-0.5 rounded text-sidebar-foreground/20 hover:text-destructive transition-colors"><Trash2 className="w-3 h-3" /></button>
              ) : undefined}
              onClick={() => selectConversation(conv.id)} />
            {isExpanded && convBranches.map((b) => renderBranch(b, 1))}
          </div>
        );
      })}

      {/* Completed branches (adopted/archived) — separate section */}
      {completedBranches.length > 0 && (
        <CompletedSection branches={completedBranches} threadBranchId={threadBranchId} openThread={openThread} />
      )}
    </>
  );
}

// ─── Completed branches section ─────────────────────────────────────────────

function CompletedSection({ branches, threadBranchId, openThread }: {
  branches: Branch[];
  threadBranchId: string | null;
  openThread: (id: string) => void;
}) {
  const adopted = branches.filter((b) => b.status === "adopted");
  const archived = branches.filter((b) => b.status === "archived");
  return <CompletedList adopted={adopted} archived={archived} threadBranchId={threadBranchId} openThread={openThread} />;
}

function CompletedList({ adopted, archived, threadBranchId, openThread }: {
  adopted: Branch[]; archived: Branch[];
  threadBranchId: string | null; openThread: (id: string) => void;
}) {
  const [open, setOpen] = useState(false);
  // Override SectionHeader's toggle
  return (
    <>
      <div onClick={() => setOpen(!open)} className="group flex items-center h-7 px-3 cursor-pointer select-none hover:bg-sidebar-accent/50 transition-colors rounded-lg">
        {open ? <ChevronDown className="w-3 h-3 text-sidebar-foreground/40 shrink-0" />
          : <ChevronRight className="w-3 h-3 text-sidebar-foreground/40 shrink-0" />}
        <span className="text-[12px] font-medium text-muted-foreground pl-1.5 flex-1">
          History
        </span>
        <span className="text-[8px] text-sidebar-foreground/30 font-mono">{adopted.length + archived.length}</span>
      </div>
      {open && (
        <div className="space-y-0.5">
          {adopted.map((b) => (
            <TreeRow key={b.id} depth={1} active={b.id === threadBranchId}
              icon={b.mode === "roundtable"
                ? <Users className="w-3.5 h-3.5 text-agent-gemini/20" />
                : <GitBranch className="w-3.5 h-3.5 text-sidebar-foreground/20" />}
              label={<span className="text-sidebar-foreground/40">{b.customLabel ?? b.label}</span>}
              suffix={<span className="w-1.5 h-1.5 rounded-full bg-status-approved shrink-0 mr-1" />}
              onClick={() => openThread(b.id)} />
          ))}
          {archived.map((b) => (
            <TreeRow key={b.id} depth={1} active={b.id === threadBranchId}
              icon={<Archive className="w-3.5 h-3.5 text-sidebar-foreground/15" />}
              label={<span className="text-sidebar-foreground/30">{b.customLabel ?? b.label}</span>}
              suffix={<span className="w-1.5 h-1.5 rounded-full bg-sidebar-foreground/20 shrink-0 mr-1" />}
              onClick={() => openThread(b.id)} />
          ))}
        </div>
      )}
    </>
  );
}

