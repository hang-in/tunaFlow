import { useState } from "react";
import { MessageSquare, Plus, Trash2, GitBranch, Users, ChevronRight, ChevronDown } from "lucide-react";
import { cn } from "@/lib/utils";
import { TreeRow, SectionHeader } from "./TreeRow";
import { InlineRename } from "../InlineRename";
import type { Conversation, Branch } from "@/types";

interface ChatsSectionProps {
  chatsOpen: boolean;
  setChatsOpen: (v: boolean) => void;
  filteredChats: Conversation[];
  selectedConversationId: string | null;
  activeBranchId: string | null;
  threadBranchId: string | null;
  selectConversation: (id: string) => void;
  renameConversation: (id: string, label: string) => Promise<void>;
  handleCreateChat: (e: React.MouseEvent) => void;
  handleDelete: (id: string, label: string, e: React.MouseEvent) => void;
  /** All branches grouped by parent conversation ID */
  branchesByConv: Map<string, Branch[]>;
  childMap: Map<string, Branch[]>;
  openThread: (branchId: string) => void;
  handleRenameBranch: (branchId: string, newLabel: string) => Promise<void>;
  onDeleteBranch: (branchId: string, label: string) => void;
  onCreateRT: () => void;
}

export function ChatsSection({
  chatsOpen, setChatsOpen, filteredChats, selectedConversationId,
  activeBranchId, threadBranchId,
  selectConversation, renameConversation, handleCreateChat, handleDelete,
  branchesByConv, childMap, openThread, handleRenameBranch, onDeleteBranch, onCreateRT,
}: ChatsSectionProps) {
  // Track which chats have their children expanded
  const [expandedChats, setExpandedChats] = useState<Set<string>>(() => new Set());

  const toggleChatExpand = (convId: string, e: React.MouseEvent) => {
    e.stopPropagation();
    setExpandedChats((prev) => {
      const next = new Set(prev);
      next.has(convId) ? next.delete(convId) : next.add(convId);
      return next;
    });
  };

  const renderBranch = (b: Branch, depth: number) => {
    const isActive = b.id === activeBranchId || b.id === threadBranchId;
    const isRT = b.mode === "roundtable";
    const children = childMap.get(b.id) ?? [];
    return (
      <div key={b.id}>
        <TreeRow depth={depth} active={isActive} isParent={children.length > 0}
          icon={isRT
            ? <Users className="w-3.5 h-3.5 text-agent-gemini/40" />
            : <GitBranch className="w-3.5 h-3.5" />}
          label={
            <span className="flex items-center gap-1">
              <InlineRename value={b.customLabel ?? b.label} onSave={(v) => handleRenameBranch(b.id, v)} inputClassName="text-[10px] w-full" />
              {isRT && <span className="text-[7px] text-agent-gemini/40 shrink-0">RT</span>}
            </span>
          }
          suffix={<span className={cn("text-[8px] font-medium uppercase px-1 rounded shrink-0",
            b.status === "active" && "text-primary/60 bg-primary/8",
            b.status === "adopted" && "text-status-approved/60 bg-status-approved/8",
            b.status === "archived" && "text-sidebar-foreground/30 bg-white/5",
          )}>{b.status}</span>}
          actions={
            <button onClick={(e) => { e.stopPropagation(); onDeleteBranch(b.id, b.customLabel ?? b.label); }}
              className="p-0.5 rounded text-sidebar-foreground/20 hover:text-destructive transition-colors" title="Delete">
              <Trash2 className="w-3 h-3" />
            </button>
          }
          onClick={() => openThread(b.id)} />
        {children.map((child) => renderBranch(child, depth + 1))}
      </div>
    );
  };

  return (
    <>
      <SectionHeader title="Chats" expanded={chatsOpen} onToggle={() => setChatsOpen(!chatsOpen)}
        actions={
          <span className="flex items-center gap-0.5">
            <button onClick={(e) => { e.stopPropagation(); onCreateRT(); }}
              className="p-0.5 rounded text-sidebar-foreground/30 hover:text-agent-gemini hover:bg-agent-gemini/10 transition-colors" title="New roundtable">
              <Users className="w-3 h-3" />
            </button>
            <button onClick={handleCreateChat} className="p-0.5 rounded text-sidebar-foreground/30 hover:text-sidebar-foreground hover:bg-white/10 transition-colors" title="New chat">
              <Plus className="w-3 h-3" />
            </button>
          </span>
        } />
      {chatsOpen && (
        <>
          {filteredChats.length === 0 && (
            <TreeRow depth={1} className="cursor-default" icon={<MessageSquare className="w-3.5 h-3.5 text-sidebar-foreground/15" />}
              label={<span className="text-[10px] text-sidebar-foreground/25 italic">No conversations</span>} />
          )}
          {filteredChats.map((conv) => {
            const isActive = conv.id === selectedConversationId;
            const convBranches = (branchesByConv.get(conv.id) ?? []).filter((b) => !b.parentBranchId);
            const hasChildren = convBranches.length > 0;
            const isExpanded = expandedChats.has(conv.id) || isActive;
            return (
              <div key={conv.id}>
                <TreeRow depth={1} active={isActive} isParent={hasChildren}
                  icon={
                    hasChildren ? (
                      <span onClick={(e) => toggleChatExpand(conv.id, e)} className="cursor-pointer">
                        {isExpanded
                          ? <ChevronDown className="w-3.5 h-3.5" />
                          : <ChevronRight className="w-3.5 h-3.5" />}
                      </span>
                    ) : <MessageSquare className="w-3.5 h-3.5" />
                  }
                  label={<InlineRename value={conv.customLabel ?? conv.label} onSave={(v) => renameConversation(conv.id, v)} inputClassName="text-[10px] w-full" />}
                  suffix={
                    <span className="flex items-center gap-1 shrink-0">
                      {hasChildren && (
                        <span className="text-[8px] text-sidebar-foreground/30 font-mono">{convBranches.length}</span>
                      )}
                      {isActive && <span className="w-1.5 h-1.5 rounded-full bg-primary shrink-0 mr-1" />}
                    </span>
                  }
                  actions={<button onClick={(e) => handleDelete(conv.id, conv.customLabel ?? conv.label, e)} className="p-0.5 rounded text-sidebar-foreground/20 hover:text-destructive transition-colors"><Trash2 className="w-3 h-3" /></button>}
                  onClick={() => selectConversation(conv.id)} />
                {/* Child branches (RT + regular) */}
                {isExpanded && convBranches.map((b) => renderBranch(b, 2))}
              </div>
            );
          })}
        </>
      )}
    </>
  );
}
