import { Users, Trash2, Plus } from "lucide-react";
import { cn } from "@/lib/utils";
import { TreeRow, SectionHeader } from "./TreeRow";
import { InlineRename } from "../InlineRename";
import type { Conversation, Branch } from "@/types";

interface RoundtablesSectionProps {
  rtOpen: boolean;
  setRtOpen: (v: boolean) => void;
  rtConvs: Conversation[];
  topLevelRT: Branch[];
  childMap: Map<string, Branch[]>;
  selectedConversationId: string | null;
  activeBranchId: string | null;
  threadBranchId: string | null;
  selectConversation: (id: string) => void;
  renameConversation: (id: string, label: string) => Promise<void>;
  openThread: (branchId: string) => void;
  handleDelete: (id: string, label: string, e: React.MouseEvent) => void;
  handleRenameBranch: (branchId: string, newLabel: string) => Promise<void>;
  onCreateRT?: () => void;
}

export function RoundtablesSection({
  rtOpen, setRtOpen, rtConvs, topLevelRT, childMap,
  selectedConversationId, activeBranchId, threadBranchId,
  selectConversation, renameConversation, openThread,
  handleDelete, handleRenameBranch, onCreateRT,
}: RoundtablesSectionProps) {
  const renderBranch = (b: Branch, depth: number) => {
    const isActive = b.id === activeBranchId || b.id === threadBranchId;
    const children = childMap.get(b.id) ?? [];
    return (
      <div key={b.id}>
        <TreeRow depth={depth} active={isActive} isParent={children.length > 0}
          icon={<Users className="w-3.5 h-3.5 text-agent-gemini/60" />}
          label={<InlineRename value={b.customLabel ?? b.label} onSave={(v) => handleRenameBranch(b.id, v)} inputClassName="text-[10px] w-full" />}
          suffix={<span className={cn("text-[8px] font-medium uppercase px-1 rounded shrink-0",
            b.status === "active" && "text-primary/60 bg-primary/8",
            b.status === "adopted" && "text-status-approved/60 bg-status-approved/8",
            b.status === "archived" && "text-sidebar-foreground/30 bg-white/5",
          )}>{b.status}</span>}
          onClick={() => openThread(b.id)} />
        {children.map((child) => renderBranch(child, depth + 1))}
      </div>
    );
  };

  return (
    <>
      <SectionHeader title="Roundtables" expanded={rtOpen} onToggle={() => setRtOpen(!rtOpen)}
        actions={onCreateRT ? (
          <button onClick={(e) => { e.stopPropagation(); onCreateRT(); }}
            className="p-0.5 rounded text-sidebar-foreground/30 hover:text-agent-gemini hover:bg-agent-gemini/10 transition-colors" title="New roundtable">
            <Plus className="w-3 h-3" />
          </button>
        ) : undefined} />
      {rtOpen && (
        <>
          {rtConvs.map((conv) => {
            const isActive = conv.id === selectedConversationId;
            return (
              <TreeRow key={conv.id} depth={1} active={isActive}
                icon={<Users className="w-3.5 h-3.5 text-agent-gemini/60" />}
                label={<InlineRename value={conv.customLabel ?? conv.label} onSave={(v) => renameConversation(conv.id, v)} inputClassName="text-[10px] w-full" />}
                suffix={isActive ? <span className="w-1.5 h-1.5 rounded-full bg-agent-gemini shrink-0 mr-1" /> : undefined}
                actions={<button onClick={(e) => handleDelete(conv.id, conv.customLabel ?? conv.label, e)} className="p-0.5 rounded text-sidebar-foreground/20 hover:text-destructive transition-colors"><Trash2 className="w-3 h-3" /></button>}
                onClick={() => selectConversation(conv.id)} />
            );
          })}
          {topLevelRT.map((b) => renderBranch(b, 1))}
          {rtConvs.length === 0 && topLevelRT.length === 0 && (
            <TreeRow depth={1} className="cursor-default" icon={<Users className="w-3.5 h-3.5 text-sidebar-foreground/15" />}
              label={<span className="text-[10px] text-sidebar-foreground/25 italic">No roundtables</span>} />
          )}
        </>
      )}
    </>
  );
}
