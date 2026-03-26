import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { cn } from "@/lib/utils";
import type { Message, Branch } from "@/types";
import { AgentAvatar } from "./AgentAvatar";
import { markdownComponents } from "./chat/MarkdownComponents";
import { MessageMeta } from "./message/MessageMeta";
import { MessageActions } from "./message/MessageActions";
import { TypingIndicator, ProgressBlock, ProgressSummary } from "./message/ProgressSurface";

function MarkdownBody({ content, className }: { content: string; className?: string }) {
  return (
    <div className={cn("prose prose-sm prose-invert max-w-none [&>*:first-child]:mt-0 [&>*:last-child]:mb-0 [&>hr:last-child]:hidden [&>hr]:border-sidebar-foreground/20 [&>hr]:my-3", className)}>
      <ReactMarkdown remarkPlugins={[remarkGfm]} components={markdownComponents}>
        {content}
      </ReactMarkdown>
    </div>
  );
}

interface MessageItemProps {
  message: Message;
  onBranch?: (messageId: string) => void;
  onBranchRT?: (messageId: string) => void;
  onMemo?: (messageId: string) => void;
  onFollowup?: (engine: string, content: string) => void;
  onDeletePair?: (messageId: string) => void;
  threadBranches?: Branch[];
  onOpenThread?: (branchId: string) => void;
  showActions?: boolean;
  variant?: "default" | "compact";
}

export function MessageItem({ message, onBranch, onBranchRT, onMemo, onFollowup, onDeletePair, threadBranches, onOpenThread, showActions = true, variant = "default" }: MessageItemProps) {
  const isUser = message.role === "user";
  const isStreaming = message.status === "streaming";
  const isCompact = variant === "compact";

  return (
    <div
      className={cn(
        "group relative flex gap-2.5 px-4 py-2.5 transition-colors",
        isCompact && "px-3 py-1.5",
        "hover:bg-accent/20",
      )}
    >
      {/* Avatar — vertically centered with header row */}
      <div className="shrink-0 self-start pt-0.5">
        <AgentAvatar engine={message.engine} isUser={isUser} size="md" />
      </div>

      {/* Content */}
      <div className={cn("flex-1 min-w-0", isCompact && "space-y-0.5")}>
        {/* Header */}
        <MessageMeta
          message={message}
          isCompact={isCompact}
          threadBranches={threadBranches}
          onOpenThread={onOpenThread}
        />

        {/* Body */}
        <div className={cn("text-foreground/90 leading-relaxed", isCompact ? "text-xs" : "text-[13px]")}>
          {isStreaming && message.content === "" && !message.progressContent ? (
            <TypingIndicator />
          ) : isStreaming ? (
            <ProgressBlock content={message.progressContent || message.content} />
          ) : isUser ? (
            <p className={cn("bg-white/[0.035] rounded-lg px-3 py-2 inline-block", isCompact && "line-clamp-3")}>{message.content}</p>
          ) : (
            <>
              {message.progressContent && <ProgressSummary content={message.progressContent} />}
              <MarkdownBody content={message.content} className={cn(isCompact && "line-clamp-3")} />
            </>
          )}
        </div>
      </div>

      {/* Branch badges moved to header — this section intentionally removed */}

      {/* Hover actions — icon-only toolbar, CSS group-hover for instant show/hide */}
      {showActions && (
        <MessageActions
          messageId={message.id}
          messageContent={message.content}
          isUser={isUser}
          onBranch={onBranch}
          onBranchRT={onBranchRT}
          onMemo={onMemo}
          onFollowup={onFollowup}
          onDeletePair={onDeletePair}
        />
      )}
    </div>
  );
}
