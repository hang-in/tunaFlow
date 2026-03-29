import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { GitBranch } from "lucide-react";

function BranchBriefPreview({ branchId }: { branchId: string }) {
  const [brief, setBrief] = useState<string | null>(null);
  useEffect(() => {
    invoke<string | null>("get_branch_brief", { branchId }).then(setBrief).catch(() => {});
  }, [branchId]);
  if (!brief) return null;
  // Extract "Key Positions" section or first 2 lines
  const lines = brief.split("\n").filter(Boolean);
  const posIdx = lines.findIndex((l) => l.includes("Key Positions"));
  const preview = posIdx >= 0
    ? lines.slice(posIdx + 1, posIdx + 4).join(" ").slice(0, 120)
    : lines.slice(0, 2).join(" ").slice(0, 120);
  return (
    <p className="text-[9px] text-agent-gemini/60 leading-snug pl-5 mb-1 line-clamp-2">
      📋 {preview}{preview.length >= 120 ? "..." : ""}
    </p>
  );
}

export function BranchesPanel() {
  const {
    branches,
    messages,
    selectedConversationId,
    activeBranchId,
    adoptBranch,
    deleteBranch,
    openBranchStream,
    closeBranchStream,
    openThread,
  } = useChatStore();

  if (!selectedConversationId) return <p className="text-xs text-muted-foreground px-2">No conversation selected</p>;

  if (activeBranchId) {
    return (
      <button
        onClick={closeBranchStream}
        className="w-full flex items-center gap-2 px-2 py-1.5 rounded-md text-xs text-primary hover:bg-primary/10 transition-colors"
      >
        ← Back to main
      </button>
    );
  }

  if (branches.length === 0) {
    return (
      <div className="text-center py-6">
        <GitBranch className="w-5 h-5 text-muted-foreground/40 mx-auto mb-2" />
        <p className="text-xs text-muted-foreground">No threads yet</p>
        <p className="text-[10px] text-muted-foreground/60 mt-1">Hover a message and click "Start thread"</p>
      </div>
    );
  }

  const handleDelete = (branchId: string, label: string) => {
    if (!window.confirm(`"${label}" 브랜치를 삭제하시겠습니까?\n\n브랜치 내 모든 메시지가 삭제됩니다.`)) return;
    deleteBranch(branchId);
  };

  return (
    <div className="space-y-2">
      {branches.map((b) => {
        const originMsg = b.checkpointId
          ? messages.find((m) => m.id === b.checkpointId)
          : null;

        return (
          <div
            key={b.id}
            className="group rounded-lg border border-border bg-card p-2.5 hover:border-border/80 transition-colors"
          >
            <div className="flex items-center gap-2 mb-1.5">
              <GitBranch className="w-3 h-3 text-primary shrink-0" />
              <span className="text-xs font-medium text-foreground flex-1 truncate">
                {b.customLabel ?? b.label}
              </span>
              <span className={cn(
                "text-[9px] font-semibold uppercase tracking-wide px-1.5 py-0.5 rounded-full border",
                b.status === "active" && "text-primary bg-primary/10 border-primary/20",
                b.status === "adopted" && "text-status-approved bg-status-approved/10 border-status-approved/20",
                b.status === "archived" && "text-muted-foreground bg-accent border-border",
              )}>
                {b.status}
              </span>
              {b.subtaskId && (
                <span className="text-[8px] font-medium text-primary/50 bg-primary/5 border border-primary/10 px-1 py-0 rounded" title="Developer lane — linked to subtask">
                  task
                </span>
              )}
              {b.gitBranch && (
                <span className="text-[8px] font-mono text-muted-foreground/50 truncate max-w-[80px]" title={`git: ${b.gitBranch}`}>
                  ⎇ {b.gitBranch}
                </span>
              )}
              {b.mode === "roundtable" && (
                <span className="text-[8px] font-semibold text-agent-gemini bg-agent-gemini/10 border border-agent-gemini/20 px-1 py-0 rounded-full">
                  RT
                </span>
              )}
            </div>

            {originMsg && (
              <p className="text-[10px] text-muted-foreground leading-relaxed line-clamp-2 mb-1 pl-5">
                {originMsg.content.slice(0, 120)}{originMsg.content.length > 120 ? "..." : ""}
              </p>
            )}
            {b.mode === "roundtable" && <BranchBriefPreview branchId={b.id} />}

            <div className="flex items-center gap-1.5 pl-5">
              <button
                onClick={() => openThread(b.id)}
                className="text-[10px] font-medium text-primary hover:underline"
              >
                Open thread
              </button>
              {b.status === "active" && (
                <>
                  <span className="text-border">·</span>
                  <button
                    onClick={() => adoptBranch(b.id, selectedConversationId)}
                    className="text-[10px] text-status-approved hover:underline"
                  >
                    Adopt
                  </button>
                </>
              )}
              <span className="flex-1" />
              <button
                onClick={() => handleDelete(b.id, b.customLabel ?? b.label)}
                className="text-[10px] text-muted-foreground opacity-0 group-hover:opacity-100 hover:text-destructive transition-all"
              >
                Delete
              </button>
            </div>
          </div>
        );
      })}
    </div>
  );
}
