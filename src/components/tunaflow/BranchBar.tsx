import { useState } from "react";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { InlineRename } from "./InlineRename";
import { GitBranch, ChevronDown } from "lucide-react";

/**
 * BranchBar — compact branch indicator below conversation header.
 * Shows active branch count, current branch (if in branch stream),
 * and expands to a quick-switch list on click.
 */
export function BranchBar() {
  const {
    branches,
    selectedConversationId,
    activeBranchId,
    adoptBranch,
    deleteBranch,
    renameBranch,
    openThread,
    openBranchStream,
    closeBranchStream,
  } = useChatStore();

  const [expanded, setExpanded] = useState(false);

  // In branch stream — show "Back to main" bar
  if (activeBranchId) {
    const activeBranch = branches.find((b) => b.id === activeBranchId);
    return (
      <div className="flex items-center gap-2 px-4 h-8 border-b border-border/40 bg-primary/3 shrink-0">
        <GitBranch className="w-3 h-3 text-primary shrink-0" />
        <span className="text-[11px] font-medium text-primary truncate flex-1 min-w-0">
          <InlineRename
            value={activeBranch?.customLabel ?? activeBranch?.label ?? "Branch"}
            onSave={(v) => renameBranch(activeBranchId, v)}
            inputClassName="text-[11px]"
          />
        </span>
        {activeBranch?.mode === "roundtable" && (
          <span className="text-[8px] font-semibold text-agent-gemini bg-agent-gemini/10 border border-agent-gemini/20 px-1 py-0 rounded-full">
            RT
          </span>
        )}
        <button
          onClick={closeBranchStream}
          className="text-[10px] text-primary hover:underline shrink-0"
        >
          ← Back to main
        </button>
      </div>
    );
  }

  if (branches.length === 0) return null;

  const activeCount = branches.filter((b) => b.status === "active").length;

  return (
    <div className="border-b border-border/30 shrink-0">
      {/* Collapsed bar */}
      <button
        onClick={() => setExpanded(!expanded)}
        className="flex items-center gap-2 w-full px-4 py-1.5 hover:bg-accent/30 transition-colors"
      >
        <GitBranch className="w-3 h-3 text-muted-foreground shrink-0" />
        <span className="text-[11px] text-muted-foreground flex-1 text-left">
          {branches.length} branch{branches.length !== 1 ? "es" : ""}
          {activeCount > 0 && activeCount < branches.length && (
            <span className="text-primary/70"> ({activeCount} active)</span>
          )}
        </span>
        <ChevronDown className={cn(
          "w-3 h-3 text-muted-foreground transition-transform",
          expanded && "rotate-180"
        )} />
      </button>

      {/* Expanded list */}
      {expanded && (
        <div className="px-3 pb-2 space-y-1">
          {branches.map((b) => (
            <div
              key={b.id}
              className="flex items-center gap-1.5 px-2 py-1 rounded-md hover:bg-accent/50 transition-colors group"
            >
              <GitBranch className="w-2.5 h-2.5 text-primary/60 shrink-0" />
              <span className="text-[10px] font-medium text-foreground flex-1 truncate min-w-0">
                <InlineRename
                  value={b.customLabel ?? b.label}
                  onSave={(v) => renameBranch(b.id, v)}
                  inputClassName="text-[10px]"
                />
              </span>
              {b.mode === "roundtable" && (
                <span className="text-[8px] font-medium text-agent-gemini/70 bg-agent-gemini/8 px-1 py-0 rounded">
                  RT
                </span>
              )}
              {b.subtaskId && (
                <span className="text-[7px] font-medium text-primary/50 bg-primary/5 border border-primary/10 px-0.5 rounded">
                  task
                </span>
              )}
              <span className={cn(
                "text-[7px] font-medium uppercase px-1 py-0 rounded tracking-wide",
                b.status === "active" && "text-primary/70 bg-primary/8",
                b.status === "adopted" && "text-status-approved/70 bg-status-approved/8",
                b.status === "archived" && "text-muted-foreground/50 bg-muted",
              )}>
                {b.status}
              </span>
              <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                <button
                  onClick={() => openThread(b.id)}
                  className="text-[9px] text-primary hover:underline"
                >
                  Open
                </button>
                {b.status === "active" && (
                  <>
                    <button
                      onClick={() => openBranchStream(b.id)}
                      className="text-[9px] text-muted-foreground hover:text-foreground hover:underline"
                    >
                      Full
                    </button>
                    <button
                      onClick={() => adoptBranch(b.id, selectedConversationId!)}
                      className="text-[9px] text-status-approved hover:underline"
                    >
                      Adopt
                    </button>
                  </>
                )}
                <button
                  onClick={() => {
                    if (window.confirm(`"${b.customLabel ?? b.label}" 브랜치를 삭제하시겠습니까?`)) {
                      deleteBranch(b.id);
                    }
                  }}
                  className="text-[9px] text-muted-foreground hover:text-destructive"
                >
                  Del
                </button>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
