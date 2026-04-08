import { cn, AGENT_TEXT_COLORS, AGENT_DISPLAY_NAMES, formatTimestamp, normalizeEngine } from "@/lib/utils";
import { GitBranch } from "lucide-react";
import { AgentAvatar } from "../AgentAvatar";
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
    <div className={cn("flex items-baseline gap-1.5 mb-1", isCompact && "mb-0.5")}>
      <AgentAvatar engine={message.engine} isUser={isUser} size="xs" />
      {isUser ? (
        <span className={cn("font-medium text-prose-base", isCompact ? "text-tf-sm" : "text-tf-caption")}>You</span>
      ) : (
        <>
          <span className={cn("font-medium", nameColorClass, isCompact ? "text-tf-sm" : "text-tf-caption")}>
            {displayName}
          </span>
          {message.model && (
            <span className="text-prose-disabled font-mono text-tf-micro">{message.model}</span>
          )}
        </>
      )}
      <span className={cn("text-prose-disabled font-mono", isCompact ? "text-tf-micro" : "text-tf-xs")}>
        {formatTimestamp(message.timestamp)}
      </span>
      {!isUser && message.durationMs != null && message.durationMs > 0 && (
        <span className="text-prose-disabled font-mono text-tf-micro">
          {message.durationMs >= 60000
            ? `${Math.floor(message.durationMs / 60000)}m ${(message.durationMs % 60000 / 1000).toFixed(1)}s`
            : `${(message.durationMs / 1000).toFixed(1)}s`}
          {message.inputTokens || message.outputTokens ? " · " : ""}
          {message.inputTokens ? `${message.inputTokens}in` : ""}
          {message.inputTokens && message.outputTokens ? "/" : ""}
          {message.outputTokens ? `${message.outputTokens}out` : ""}
        </span>
      )}
      {/* Branch badges inline in header */}
      {threadBranches && threadBranches.length > 0 && !isCompact && threadBranches.map((branch) => (
        <button
          key={branch.id}
          onClick={(e) => { e.stopPropagation(); onOpenThread?.(branch.id); }}
          className="inline-flex items-center gap-0.5 text-tf-micro font-medium text-primary/80 bg-primary/10 hover:bg-primary/18 px-1.5 py-0.5 rounded transition-colors"
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
        <span className="text-primary/50 font-mono text-tf-micro animate-pulse">streaming</span>
      )}
      {message.status === "error" && (
        <span className="text-destructive/60 font-mono text-tf-micro">error</span>
      )}
    </div>
  );
}
