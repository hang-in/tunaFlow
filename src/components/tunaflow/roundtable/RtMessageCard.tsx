import { useState } from "react";
import { cn, formatTimestamp, normalizeEngine } from "@/lib/utils";
import { copyToClipboard } from "@/lib/clipboard";
import { AgentAvatar } from "../AgentAvatar";
import { markdownComponents } from "../chat/MarkdownComponents";
import { RtReferenceBadge } from "./RtReferenceBadge";
import { parsePromptSources } from "./rtUtils";
import type { Message, RoundtableParticipant } from "@/types";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Copy, Users, GitBranch, StickyNote, Forward, FileText, ShieldCheck, Trash2 } from "lucide-react";

export interface RtMessageCardProps {
  message: Message;
  isLast: boolean;
  onBranch?: (messageId: string) => void;
  onBranchRT?: (messageId: string) => void;
  onMemo?: (messageId: string) => void;
  onFollowup?: (engine: string, content: string) => void;
  onSaveArtifact?: (content: string) => void;
  onDelete?: (messageId: string) => void;
  participantMeta?: RoundtableParticipant;
}

export function RtMessageCard({ message, isLast, onBranch, onBranchRT, onMemo, onFollowup, onSaveArtifact, onDelete, participantMeta }: RtMessageCardProps) {
  const [hovered, setHovered] = useState(false);
  const name = message.persona ?? message.engine ?? "Agent";
  const engine = message.engine ?? "";
  const knownEngine = normalizeEngine(engine);
  const content = message.content;
  const sources = parsePromptSources(message);

  return (
    <div className="relative">
      {/* Card */}
      <div
        className={cn(
          "min-w-0 mb-2 rounded-md bg-card/60 border border-border/30 p-3 transition-colors relative overflow-hidden",
          hovered && "border-border/50"
        )}
        onMouseEnter={() => setHovered(true)}
        onMouseLeave={() => setHovered(false)}
      >
        {/* Header: avatar inline + engine · model · role (matches MessageMeta pattern) */}
        <div className="flex items-baseline gap-1.5 mb-1 flex-wrap">
          <AgentAvatar engine={engine} size="xs" />
          <span className={cn("text-tf-caption font-medium", knownEngine ? `text-agent-${knownEngine}` : "text-prose-base")}>
            {name}
          </span>
          {message.model && (
            <span className="text-prose-disabled font-mono text-tf-micro">
              {message.model}
            </span>
          )}
          {participantMeta?.role && (
            <span className="text-tf-micro text-primary/50 bg-primary/8 px-1 py-px rounded font-medium">
              {participantMeta.role}
            </span>
          )}
          {participantMeta?.blind && (
            <span className="text-tf-micro text-amber-500/60 bg-amber-500/8 px-1 py-px rounded font-medium inline-flex items-center gap-0.5">
              <ShieldCheck className="w-2.5 h-2.5" />blind
            </span>
          )}
          <span className="text-prose-disabled font-mono text-tf-xs">
            {formatTimestamp(message.timestamp)}
          </span>
          {message.durationMs != null && message.durationMs > 0 && (
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
          {sources && <RtReferenceBadge sources={sources} />}
        </div>

        {/* Body — same react-markdown pipeline as main chat */}
        <div className="prose prose-sm prose-invert max-w-none text-[13px] text-foreground/90 leading-relaxed [&>*:first-child]:mt-0 [&>*:last-child]:mb-0">
          <ReactMarkdown remarkPlugins={[[remarkGfm, { singleTilde: false }]]} components={markdownComponents}>
            {content}
          </ReactMarkdown>
        </div>

        {/* Actions */}
        <div className={cn(
          "absolute right-2 top-2 flex items-center gap-0.5 transition-opacity",
          hovered ? "opacity-100" : "opacity-0 pointer-events-none"
        )}>
          {onBranch && (
            <button onClick={() => onBranch(message.id)} title="Branch"
              className="p-1 rounded text-muted-foreground/40 hover:text-foreground hover:bg-accent transition-colors">
              <GitBranch className="w-3 h-3" />
            </button>
          )}
          {onBranchRT && (
            <button onClick={() => onBranchRT(message.id)} title="Roundtable"
              className="p-1 rounded text-muted-foreground/40 hover:text-agent-gemini hover:bg-agent-gemini/10 transition-colors">
              <Users className="w-3 h-3" />
            </button>
          )}
          {onMemo && (
            <button onClick={() => onMemo(message.id)} title="Memo"
              className="p-1 rounded text-muted-foreground/40 hover:text-foreground hover:bg-accent transition-colors">
              <StickyNote className="w-3 h-3" />
            </button>
          )}
          {onSaveArtifact && (
            <button onClick={() => onSaveArtifact(message.content)} title="Save as Artifact"
              className="p-1 rounded text-muted-foreground/40 hover:text-primary hover:bg-primary/10 transition-colors">
              <FileText className="w-3 h-3" />
            </button>
          )}
          {onFollowup && (
            <button onClick={() => onFollowup(message.engine ?? "claude", message.content)} title="Forward"
              className="p-1 rounded text-muted-foreground/40 hover:text-foreground hover:bg-accent transition-colors">
              <Forward className="w-3 h-3" />
            </button>
          )}
          <button onClick={() => copyToClipboard(message.content)} title="Copy"
            className="p-1 rounded text-muted-foreground/40 hover:text-foreground hover:bg-accent transition-colors">
            <Copy className="w-3 h-3" />
          </button>
          {onDelete && (
            <button onClick={() => onDelete(message.id)} title="Delete"
              className="p-1 rounded text-muted-foreground/40 hover:text-destructive hover:bg-destructive/10 transition-colors">
              <Trash2 className="w-3 h-3" />
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
