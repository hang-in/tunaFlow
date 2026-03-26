import { cn, AGENT_TEXT_COLORS, AGENT_DISPLAY_NAMES, formatTimestamp, normalizeEngine } from "@/lib/utils";
import { GitBranch } from "lucide-react";
import type { Message, Branch } from "@/types";

interface MessageMetaProps {
  message: Message;
  isCompact?: boolean;
  threadBranches?: Branch[];
  onOpenThread?: (branchId: string) => void;
}

export function MessageMeta({ message, isCompact = false, threadBranches, onOpenThread }: MessageMetaProps) {
  const isUser = message.role === "user";
  const isStreaming = message.status === "streaming";
  const engine = normalizeEngine(message.engine);
  const displayName = message.persona ?? (engine ? AGENT_DISPLAY_NAMES[engine] : "Assistant");
  const nameColorClass = engine ? AGENT_TEXT_COLORS[engine] : "text-foreground/80";

  return (
    <div className={cn("flex items-baseline gap-2 mb-1", isCompact && "mb-0.5")}>
      {isUser ? (
        <span className={cn("font-medium text-foreground/80", isCompact ? "text-[11px]" : "text-[13px]")}>You</span>
      ) : (
        <>
          <span className={cn("font-medium", nameColorClass, isCompact ? "text-[11px]" : "text-[13px]")}>
            {displayName}
          </span>
          {message.model && (
            <span className="text-sidebar-foreground/50 font-mono text-[10px]">{message.model}</span>
          )}
        </>
      )}
      <span className={cn("text-sidebar-foreground/50 font-mono", isCompact ? "text-[9px]" : "text-[10px]")}>
        {formatTimestamp(message.timestamp)}
      </span>
      {/* Branch badges inline in header */}
      {threadBranches && threadBranches.length > 0 && !isCompact && threadBranches.map((branch) => (
        <button
          key={branch.id}
          onClick={(e) => { e.stopPropagation(); onOpenThread?.(branch.id); }}
          className="inline-flex items-center gap-0.5 text-[9px] font-medium text-primary/80 bg-primary/10 hover:bg-primary/18 px-1.5 py-0.5 rounded transition-colors"
        >
          <GitBranch className="w-2 h-2" />
          <span className="truncate max-w-[60px]">{branch.customLabel ?? branch.label}</span>
          <span className={cn("uppercase",
            branch.status === "active" && "text-primary/70",
            branch.status === "adopted" && "text-status-approved/70",
          )}>{branch.status}</span>
        </button>
      ))}
      {isStreaming && (
        <span className="text-primary/50 font-mono text-[9px] animate-pulse">streaming</span>
      )}
      {message.status === "error" && (
        <span className="text-destructive/60 font-mono text-[9px]">error</span>
      )}
    </div>
  );
}
