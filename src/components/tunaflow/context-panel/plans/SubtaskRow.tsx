import { cn } from "@/lib/utils";
import { Forward, GitBranch } from "lucide-react";
import type { PlanSubtask, SubtaskStatus } from "@/types";
import { AgentAvatar } from "../../AgentAvatar";
import { useChatStore } from "@/stores/chatStore";
import { SUBTASK_STATUS_CFG } from "./constants";

export function SubtaskRow({
  subtask,
  planTitle,
  onStatusChange,
  onOwnerChange,
  onForwardSubtask,
  linkedBranch,
  onOpenThread,
}: {
  subtask: PlanSubtask;
  planTitle: string;
  onStatusChange: (id: string, status: SubtaskStatus) => void;
  onOwnerChange: (id: string, owner: string | null) => void;
  onForwardSubtask?: (engine: string, payload: string) => void;
  linkedBranch?: { id: string; label: string; customLabel?: string; status: string } | null;
  onOpenThread?: (branchId: string) => void;
}) {
  const profiles = useChatStore((s) => s.agentProfiles);
  const cfg = SUBTASK_STATUS_CFG[subtask.status];
  const owner = subtask.ownerAgent;

  // Build rich follow-up payload for this subtask
  const buildPayload = () => {
    const lines = [
      `[Task] ${subtask.title}`,
      `Plan: ${planTitle}`,
      `Status: ${subtask.status}`,
    ];
    if (owner) lines.push(`Owner: ${owner}`);
    if (subtask.details) lines.push(`\nDetails:\n${subtask.details}`);
    if (linkedBranch) lines.push(`\nLinked branch: ${linkedBranch.customLabel ?? linkedBranch.label} (${linkedBranch.status})`);
    lines.push("\n위 작업을 진행해주세요.");
    return lines.join("\n");
  };

  const canForward = subtask.status === "approved" || subtask.status === "in_progress";

  return (
    <div className="flex items-start gap-2 py-1.5 border-b border-border/30 last:border-0">
      <button
        title={`Click to → ${cfg.next}`}
        onClick={() => onStatusChange(subtask.id, cfg.next)}
        className={cn(
          "shrink-0 mt-0.5 text-[9px] font-semibold px-1.5 py-0.5 rounded-full border whitespace-nowrap",
          cfg.cls
        )}
      >
        {cfg.label}
      </button>
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-1.5">
          <p className="text-[11px] text-foreground leading-snug flex-1">{subtask.title}</p>
          {owner ? (
            <span className="inline-flex items-center gap-1 text-[8px] font-medium px-1.5 py-0.5 rounded bg-accent shrink-0" title={`Owner: ${owner}`}>
              <AgentAvatar engine={owner} size="sm" className="w-3 h-3" />
              {owner}
            </span>
          ) : (
            <span className="text-[8px] text-muted-foreground/30 shrink-0">unassigned</span>
          )}
        </div>
        {subtask.details ? (
          <p className="text-[10px] text-muted-foreground leading-snug mt-0.5 line-clamp-2">{subtask.details}</p>
        ) : (
          <p className="text-[10px] text-amber-600/40 italic mt-0.5">상세 설계 없음</p>
        )}
        {/* Linked branch — if any */}
        {linkedBranch && (
          <div className="flex items-center gap-1.5 mt-1">
            <button
              onClick={() => onOpenThread?.(linkedBranch.id)}
              className="inline-flex items-center gap-0.5 text-[8px] font-medium text-primary/60 bg-primary/6 hover:bg-primary/12 px-1 py-0 rounded transition-colors"
              title={`Open branch: ${linkedBranch.customLabel ?? linkedBranch.label}`}
            >
              <GitBranch className="w-2 h-2" />
              {linkedBranch.customLabel ?? linkedBranch.label}
              <span className="text-muted-foreground/40 ml-0.5">{linkedBranch.status}</span>
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
