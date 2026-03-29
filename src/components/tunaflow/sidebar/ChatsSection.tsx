import { useState, useMemo } from "react";
import { MessageSquare, Trash2, GitBranch, Users, ChevronRight, ChevronDown } from "lucide-react";
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
}

export function ChatsSection({
  filteredChats, selectedConversationId,
  activeBranchId, threadBranchId,
  selectConversation, renameConversation, handleDelete,
  branchesByConv, childMap, openThread, handleRenameBranch, onDeleteBranch,
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

  const renderBranch = (b: Branch, depth: number) => {
    const isActive = b.id === activeBranchId || b.id === threadBranchId;
    const isRT = b.mode === "roundtable";
    const children = childMap.get(b.id) ?? [];
    const hasChildren = children.length > 0;
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
        {isExpanded && children.map((child) => renderBranch(child, depth + 1))}
      </div>
    );
  };

  return (
    <>
      {filteredChats.map((conv) => {
        const isActive = conv.id === selectedConversationId;
        const convBranches = (branchesByConv.get(conv.id) ?? []).filter((b) => !b.parentBranchId);
        const hasChildren = convBranches.length > 0;
        // Always expanded when active or has active thread in descendants
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
              suffix={isActive ? <span className="w-1.5 h-1.5 rounded-full bg-primary shrink-0 mr-1" /> : undefined}
              actions={filteredChats.length > 1 ? (
                <button onClick={(e) => handleDelete(conv.id, conv.customLabel ?? conv.label, e)} className="p-0.5 rounded text-sidebar-foreground/20 hover:text-destructive transition-colors"><Trash2 className="w-3 h-3" /></button>
              ) : undefined}
              onClick={() => selectConversation(conv.id)} />
            {isExpanded && convBranches.map((b) => renderBranch(b, 1))}
          </div>
        );
      })}
    </>
  );
}
