import { memo, useMemo, useState, useEffect } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { cn } from "@/lib/utils";
import type { Message, Branch } from "@/types";
import { AgentAvatar } from "./AgentAvatar";
import { markdownComponents } from "./chat/MarkdownComponents";
import { PlanProposalCard } from "./chat/PlanProposalCard";
import { MessageMeta } from "./message/MessageMeta";
import { MessageActions } from "./message/MessageActions";
import { TypingIndicator } from "./message/ProgressSurface";
import { ToolStepsView } from "./message/ToolStepsView";
import { useToolStepsStore } from "@/stores/toolStepsStore";
import { deserializeSteps } from "@/lib/toolSteps";
import { hasPlanProposal, splitPlanProposals } from "@/lib/planProposalParser";
import { copyToClipboard } from "@/lib/clipboard";
import { MessageContextMenu } from "./ContextMenu";

const PROSE_CLS = "prose prose-invert prose-chat max-w-none [&>*:first-child]:mt-0 [&>*:last-child]:mb-0 [&>hr:last-child]:hidden [&>hr]:border-sidebar-foreground/20 [&>hr]:my-3";

/** Detect if user message content contains markdown that benefits from rich rendering. */
function hasMarkdownSignal(content: string): boolean {
  if (content.length < 100) return false;
  return /^#{1,3} |```|\n- |\n\d+\. |<!-- tunaflow:|\*\*[^*]+\*\*/m.test(content);
}

/** Clean all tunaflow markers from message display.
 *  Markers are for the workflow pipeline only — users should never see them.
 */
function vizMarkers(text: string): string {
  return text
    // Remove full blocks (verdict, impl-plan — rendered separately in PlanCard)
    .replace(/<!-- ?tunaflow:review-verdict ?-->[\s\S]*?<!-- ?\/?tunaflow:review-verdict ?-->/g, "")
    .replace(/<!-- ?tunaflow:impl-plan ?-->[\s\S]*?<!-- ?\/?tunaflow:impl-plan ?-->/g, "")
    // Remove remaining single markers EXCEPT plan-proposal (parsed by splitPlanProposals for PlanProposalCard)
    .replace(/<!-- ?\/?(?:tunaflow:(?!plan-proposal)[a-z_-]+(?::\d+)?|subtask-done:\d+|impl-complete) ?-->/g, "")
    // Clean up leftover blank lines from removed markers
    .replace(/\n{3,}/g, "\n\n");
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const REMARK_PLUGINS: any[] = [[remarkGfm, { singleTilde: false }]];

function MarkdownBody({ content, className, conversationId, isStreaming }: { content: string; className?: string; conversationId?: string; isStreaming?: boolean }) {
  // Skip expensive marker processing during streaming — apply only on final render
  const processed = useMemo(() => isStreaming ? content : vizMarkers(content), [content, isStreaming]);
  const segments = useMemo(
    () => (hasPlanProposal(processed) ? splitPlanProposals(processed) : null),
    [processed],
  );

  if (segments && conversationId) {
    return (
      <div className={cn(PROSE_CLS, className)}>
        {segments.map((seg, i) =>
          seg.type === "plan-proposal" ? (
            <PlanProposalCard key={i} proposal={seg.proposal} conversationId={conversationId} />
          ) : (
            <ReactMarkdown key={i} remarkPlugins={REMARK_PLUGINS} components={markdownComponents}>
              {seg.content}
            </ReactMarkdown>
          ),
        )}
      </div>
    );
  }

  return (
    <div className={cn(PROSE_CLS, className)}>
      <ReactMarkdown remarkPlugins={REMARK_PLUGINS} components={markdownComponents}>
        {processed}
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
  onSaveArtifact?: (content: string) => void;
  threadBranches?: Branch[];
  onOpenThread?: (branchId: string) => void;
  showActions?: boolean;
  variant?: "default" | "compact";
  /** True when previous message has the same sender — hides avatar/name */
  grouped?: boolean;
}

export const MessageItem = memo(function MessageItem({ message, onBranch, onBranchRT, onMemo, onFollowup, onDeletePair, onSaveArtifact, threadBranches, onOpenThread, showActions = true, variant = "default", grouped = false }: MessageItemProps) {
  const isUser = message.role === "user";
  const isStreaming = message.status === "streaming";
  const isCompact = variant === "compact";

  // Tool steps — live from store during streaming, from progressContent after completion
  const liveSteps = useToolStepsStore((s) => s.stepsMap[message.id]);
  const startTime = useToolStepsStore((s) => s.startTimeMap[message.id] ?? 0);
  const [elapsed, setElapsed] = useState(0);
  useEffect(() => {
    if (!isStreaming || !startTime) { setElapsed(0); return; }
    const interval = setInterval(() => setElapsed(Date.now() - startTime), 1000);
    return () => clearInterval(interval);
  }, [isStreaming, startTime]);
  const toolSteps = useMemo(() => {
    if (isStreaming && liveSteps?.length) return liveSteps;
    if (!isStreaming && message.progressContent) return deserializeSteps(message.progressContent);
    return [];
  }, [isStreaming, liveSteps, message.progressContent]);

  return (
    <MessageContextMenu
      isUser={isUser}
      onCopy={() => copyToClipboard(message.content)}
      onBranch={onBranch ? () => onBranch(message.id) : undefined}
      onBranchRT={onBranchRT ? () => onBranchRT(message.id) : undefined}
      onMemo={onMemo ? () => onMemo(message.id) : undefined}
      onSaveArtifact={onSaveArtifact ? () => onSaveArtifact(message.content) : undefined}
      onFollowup={onFollowup ? () => onFollowup("claude", message.content) : undefined}
      onDelete={onDeletePair ? async () => {
        const { ask } = await import("@tauri-apps/plugin-dialog");
        if (await ask("이 메시지를 삭제하시겠습니까?", { title: "메시지 삭제", kind: "warning" })) onDeletePair(message.id);
      } : undefined}
    >
      <div
        className={cn(
          "relative flex gap-2.5 px-4 transition-colors",
          grouped ? "py-1" : "py-2",
          isCompact && "px-3 py-1",
          "hover:bg-accent/20",
        )}
      >
        {/* Avatar — hidden for grouped messages, placeholder for alignment */}
        <div className="shrink-0 self-start pt-0.5 w-7">
          {!grouped && <AgentAvatar engine={message.engine} isUser={isUser} size="sm" />}
        </div>

        {/* Content */}
        <div className={cn("flex-1 min-w-0", isCompact && "space-y-0.5")}>
          {/* Header — hidden for grouped consecutive messages */}
          {!grouped && (
            <MessageMeta
              message={message}
              isCompact={isCompact}
              threadBranches={threadBranches}
              onOpenThread={onOpenThread}
            />
          )}

          {/* Tool steps — streaming live or completed collapsed */}
          {!isUser && toolSteps.length > 0 && (
            <ToolStepsView steps={toolSteps} isStreaming={isStreaming} durationMs={isStreaming ? elapsed : undefined} />
          )}

          {/* Body */}
          <div className={cn("text-foreground leading-relaxed max-w-4xl", isCompact ? "text-xs" : "text-sm")}>
            {isUser && hasMarkdownSignal(message.content) ? (
              <div className={cn("bg-white/[0.035] rounded-lg px-3 py-2 inline-block max-w-4xl", isCompact && "line-clamp-3")}>
                <MarkdownBody content={message.content} conversationId={message.conversationId} />
              </div>
            ) : isUser ? (
              <p className={cn("bg-white/[0.035] rounded-lg px-3 py-2 inline-block max-w-4xl", isCompact && "line-clamp-3")}>{message.content}</p>
            ) : isStreaming && !message.content ? (
              <TypingIndicator />
            ) : isStreaming ? (
              <MarkdownBody content={message.content} conversationId={message.conversationId} isStreaming />
            ) : (
              <MarkdownBody content={message.content} conversationId={message.conversationId} className={cn(isCompact && "line-clamp-3")} />
            )}
          </div>
        </div>

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
            onSaveArtifact={onSaveArtifact}
          />
        )}
      </div>
    </MessageContextMenu>
  );
}, (prev, next) => {
  // Only re-render when message content/status changes, or branches change
  if (prev.message !== next.message) return false;
  if (prev.threadBranches !== next.threadBranches) return false;
  if (prev.showActions !== next.showActions) return false;
  if (prev.variant !== next.variant) return false;
  if (prev.grouped !== next.grouped) return false;
  return true; // Skip re-render for callback prop changes
});
