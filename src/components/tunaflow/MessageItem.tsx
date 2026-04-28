import { memo, useMemo, useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import ReactMarkdown from "react-markdown";
import { invoke } from "@tauri-apps/api/core";
import { cn } from "@/lib/utils";
import { REMARK_PLUGINS } from "@/lib/markdownPlugins";
import type { Message, Branch } from "@/types";

import { markdownComponents } from "./chat/MarkdownComponents";
import { PlanProposalCard } from "./chat/PlanProposalCard";
import { MessageMeta } from "./message/MessageMeta";
import { MessageActions } from "./message/MessageActions";
import { TypingIndicator } from "./message/ProgressSurface";
import { ToolStepsView } from "./message/ToolStepsView";
import { useToolStepsStore } from "@/stores/toolStepsStore";
import { deserializeSteps } from "@/lib/toolSteps";
import { hasPlanProposal, splitPlanProposals } from "@/lib/planProposalParser";
import { vizMarkers } from "@/lib/vizMarkers";
import { copyToClipboard } from "@/lib/clipboard";
import { MessageContextMenu } from "./ContextMenu";
import { ChevronRight } from "lucide-react";

/** Collapsible display for tool-request follow-up results (auto-injected by tunaFlow). */
function ToolResultCollapsible({ content, conversationId }: { content: string; conversationId?: string }) {
  const { t } = useTranslation("chat");
  const [open, setOpen] = useState(false);
  const lineCount = content.split("\n").length;
  return (
    <div className="rounded-lg border border-border/30 overflow-hidden text-[11px]">
      <button
        onClick={() => setOpen(!open)}
        className="flex items-center gap-1.5 w-full px-3 py-1.5 text-muted-foreground/70 hover:text-foreground hover:bg-accent/30 transition-colors"
      >
        <ChevronRight className={cn("w-3 h-3 transition-transform", open && "rotate-90")} />
        <span className="font-medium">{t("message.tool_result_label")}</span>
        <span className="text-muted-foreground/40 ml-1">{t("message.tool_result_lines", { count: lineCount })}</span>
      </button>
      {open && (
        <div className="px-3 py-2 border-t border-border/20 text-xs">
          <MarkdownBody content={content} conversationId={conversationId} isUser />
        </div>
      )}
    </div>
  );
}

const PROSE_CLS = "prose prose-invert prose-chat max-w-none [&>*:first-child]:mt-0 [&>*:last-child]:mb-0 [&>hr:last-child]:hidden [&>hr]:border-sidebar-foreground/20 [&>hr]:my-3";

/** Detect if user message content contains markdown that benefits from rich rendering. */
function hasMarkdownSignal(content: string): boolean {
  if (content.length < 100) return false;
  return /^#{1,3} |```|\n- |\n\d+\. |<!-- tunaflow:|\*\*[^*]+\*\*/m.test(content);
}

/**
 * Hide partially-arrived HTML comments during streaming so the raw
 * `<!-- tunaflow:... -->` marker text does not flash on screen before the
 * closing `-->` token arrives and react-markdown collapses it out.
 * Only applied while isStreaming=true; the final render goes through
 * the normal pipeline (PlanProposalCard / vizMarkers).
 */
function hideTrailingIncompleteComment(text: string): string {
  const lastOpen = text.lastIndexOf("<!--");
  const lastClose = text.lastIndexOf("-->");
  if (lastOpen > lastClose) {
    // There is an unclosed comment at the tail; trim it so it never renders.
    return text.slice(0, lastOpen);
  }
  return text;
}

function MarkdownBody({ content, className, conversationId, isStreaming, isUser }: { content: string; className?: string; conversationId?: string; isStreaming?: boolean; isUser?: boolean }) {
  // Skip expensive marker processing during streaming — apply only on final render.
  // Also suppress in-flight HTML-comment markers so they don't flash as plain text.
  const processed = useMemo(() => {
    if (!isStreaming) return vizMarkers(content);
    return hideTrailingIncompleteComment(content);
  }, [content, isStreaming]);
  // Plan proposal markers are only valid in assistant messages — never parse user messages
  // (user feedback text may quote marker strings which would falsely trigger PlanProposalCard)
  const segments = useMemo(
    () => (!isUser && hasPlanProposal(processed) ? splitPlanProposals(processed) : null),
    [processed, isUser],
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
  const { t } = useTranslation("chat");
  const isUser = message.role === "user";
  const isSystem = message.role === "system";
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
  // Lazy-load progressContent when marker "…" is present (list_messages returns lightweight)
  const [loadedProgress, setLoadedProgress] = useState<string | null>(null);
  useEffect(() => {
    if (!isUser && !isStreaming && message.progressContent === "…") {
      invoke<string | null>("get_progress_content", { messageId: message.id })
        .then((data) => { if (data) setLoadedProgress(data); })
        .catch(() => {});
    }
  }, [message.id, message.progressContent, isUser, isStreaming]);
  const effectiveProgress = (message.progressContent && message.progressContent !== "…")
    ? message.progressContent
    : loadedProgress;
  const toolSteps = useMemo(() => {
    if (isStreaming && liveSteps?.length) return liveSteps;
    if (effectiveProgress) return deserializeSteps(effectiveProgress);
    return [];
  }, [isStreaming, liveSteps, effectiveProgress]);

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
        if (await ask(t("message.delete_confirm_body"), { title: t("message.delete_confirm_title"), kind: "warning" })) onDeletePair(message.id);
      } : undefined}
    >
      <div
        className={cn(
          "relative flex items-start pl-5 transition-colors border-l-2 border-l-transparent",
          grouped ? "py-1" : "py-2",
          isCompact && "pl-4 py-1",
          "hover:border-l-primary/40 hover:bg-accent/30",
          showActions ? "pr-16" : "pr-4",
        )}
      >
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
          <div className={cn("text-foreground leading-relaxed overflow-x-auto", isCompact ? "text-xs" : "text-sm")}>
            {isSystem ? (
              <ToolResultCollapsible content={message.content} conversationId={message.conversationId} />
            ) : isUser ? (
              message.content.startsWith("### 🛠️ 도구 호출 결과") ? (
                <ToolResultCollapsible content={message.content} conversationId={message.conversationId} />
              ) : (
              <div className={cn("rounded-lg px-3 py-2 inline-block", isCompact && "line-clamp-3")} style={{ background: "var(--user-bubble)" }}>
                <MarkdownBody content={message.content} conversationId={message.conversationId} isUser />
              </div>
              )
            ) : isStreaming && !message.content ? (
              <TypingIndicator />
            ) : isStreaming ? (
              <>
                <MarkdownBody content={message.content} conversationId={message.conversationId} isStreaming />
                {/* Tool running 중에는 본문 아래에도 타이핑 인디케이터를 병행 표시 —
                    tool 결과 대기 중 "멈춘 것처럼" 보이는 UX 문제 해소. */}
                {toolSteps.some((s) => s.status === "running") && (
                  <div className="mt-1 opacity-60"><TypingIndicator /></div>
                )}
              </>
            ) : (
              <MarkdownBody content={message.content} conversationId={message.conversationId} className={cn(isCompact && "line-clamp-3")} />
            )}
          </div>
        </div>

        {/* Hover actions — absolute overlay at top-right of bubble */}
        {showActions && (
          <div className="absolute top-1 right-1">
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
          </div>
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
