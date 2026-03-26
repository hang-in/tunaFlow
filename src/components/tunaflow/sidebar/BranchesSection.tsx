import { GitBranch, Users } from "lucide-react";
import { cn } from "@/lib/utils";
import { TreeRow, SectionHeader } from "./TreeRow";
import { InlineRename } from "../InlineRename";
import type { Branch } from "@/types";

interface BranchesSectionProps {
  branchesOpen: boolean;
  setBranchesOpen: (v: boolean) => void;
  topLevelBranches: Branch[];
  childMap: Map<string, Branch[]>;
  activeBranchId: string | null;
  threadBranchId: string | null;
  openThread: (branchId: string) => void;
  handleRenameBranch: (branchId: string, newLabel: string) => Promise<void>;
}

export function BranchesSection({
  branchesOpen, setBranchesOpen, topLevelBranches, childMap,
  activeBranchId, threadBranchId, openThread, handleRenameBranch,
}: BranchesSectionProps) {
  const renderBranch = (b: Branch, depth: number) => {
    const isActive = b.id === activeBranchId || b.id === threadBranchId;
    const children = childMap.get(b.id) ?? [];
    return (
      <div key={b.id}>
        <TreeRow depth={depth} active={isActive} isParent={children.length > 0}
          icon={b.mode === "roundtable" ? <Users className="w-3.5 h-3.5 text-agent-gemini/60" /> : <GitBranch className="w-3.5 h-3.5" />}
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
      <SectionHeader title="Branches" expanded={branchesOpen} onToggle={() => setBranchesOpen(!branchesOpen)} />
      {branchesOpen && (
        <>
          {topLevelBranches.length === 0 ? (
            <TreeRow depth={1} className="cursor-default" icon={<GitBranch className="w-3.5 h-3.5 text-sidebar-foreground/15" />}
              label={<span className="text-[10px] text-sidebar-foreground/25 italic">No branches</span>} />
          ) : topLevelBranches.map((b) => renderBranch(b, 1))}
        </>
      )}
    </>
  );
}
