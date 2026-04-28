import { cn, AGENT_DOT_COLORS, normalizeEngine } from "@/lib/utils";
import type { Message, RoundtableParticipant } from "@/types";
import { Users, Loader2 } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { useState, useEffect } from "react";
import ReactMarkdown from "react-markdown";
import { useChatStore } from "@/stores/chatStore";
import type { RtParticipantStatus } from "@/stores/slices/threadSlice";
import { markdownComponents } from "./chat/MarkdownComponents";
import { REMARK_PLUGINS } from "@/lib/markdownPlugins";

/** RT topic 렌더러 — Review RT 프롬프트는 마크다운 + HTML 주석 마커를 포함하므로
 *  raw text 로 출력하면 가독성 급락. 채팅과 동일한 마크다운 파이프라인 재사용. */
function TopicMarkdown({ text }: { text: string }) {
  return (
    <div className="prose prose-invert prose-chat prose-sm max-w-none text-[13px] leading-relaxed [&>*:first-child]:mt-0 [&>*:last-child]:mb-0">
      <ReactMarkdown remarkPlugins={REMARK_PLUGINS} components={markdownComponents}>
        {text}
      </ReactMarkdown>
    </div>
  );
}

import { groupIntoRounds, getParticipants, parsePromptSources } from "./roundtable/rtUtils";
import { RtMessageCard } from "./roundtable/RtMessageCard";

interface RoundtableViewProps {
  messages: Message[];
  onBranch?: (messageId: string) => void;
  onBranchRT?: (messageId: string) => void;
  onMemo?: (messageId: string) => void;
  onFollowup?: (engine: string, content: string) => void;
  onSaveArtifact?: (content: string) => void;
  onDelete?: (messageId: string) => void;
  conversationId?: string;
}

const RT_MODE_LABELS: Record<string, string> = {
  sequential: "Sequential",
  deliberative: "Deliberative",
};

// ─── Main View ──────────────────────────────────────────────────────────────

export function RoundtableView({ messages, conversationId, onBranch, onBranchRT, onMemo, onFollowup, onSaveArtifact, onDelete }: RoundtableViewProps) {
  const participants = getParticipants(messages);
  const rounds = groupIntoRounds(messages);
  const selectedConversationId = useChatStore((s) => s.selectedConversationId);
  const runningThreadIds = useChatStore((s) => s.runningThreadIds);
  const targetConvId = conversationId ?? selectedConversationId;
  const isRunning = !!targetConvId && runningThreadIds.includes(targetConvId);

  // Load RT config for role/blind visibility
  const [rtParticipants, setRtParticipants] = useState<RoundtableParticipant[]>([]);
  useEffect(() => {
    if (!targetConvId) return;
    invoke<string>("get_rt_config", { conversationId: targetConvId })
      .then((json) => {
        try {
          const cfg = JSON.parse(json) as { participants?: RoundtableParticipant[] };
          setRtParticipants(cfg.participants ?? []);
        } catch { setRtParticipants([]); }
      })
      .catch(() => setRtParticipants([]));
  }, [targetConvId]);

  // ─── Real-time participant telemetry (from store, scoped by conversationId) ──
  const allStatuses = useChatStore((s) => s.rtParticipantStatuses);
  const statusConvId = useChatStore((s) => s.rtStatusConversationId);
  const pStatuses = statusConvId === targetConvId ? allStatuses : new Map();

  const userMessages = messages.filter((m) => m.role === "user");
  const originalTopic = userMessages.length > 0 ? userMessages[0].content : null;
  const roundTopics: (string | null)[] = rounds.map((_, i) => i < userMessages.length ? userMessages[i].content : null);

  const firstRtMsg = messages.find((m) => m.role === "assistant" && m.engine !== "system" && m.progressContent);
  const firstSources = firstRtMsg ? parsePromptSources(firstRtMsg) : null;
  const totalRounds = firstSources?.totalRounds ?? rounds.length;
  const rtMode = firstSources?.mode ?? "sequential";

  if (rounds.length === 0) {
    if (isRunning && originalTopic) {
      // Show topic + loading state while waiting for first participant response
      return (
        <div className="px-5 py-4 max-w-3xl mx-auto w-full">
          <div className="mb-5 pb-3 border-b border-border/30 space-y-2">
            <div className="rounded-md bg-accent/30 p-2.5">
              <p className="text-[9px] font-semibold text-muted-foreground/40 uppercase tracking-widest mb-0.5">Topic</p>
              <TopicMarkdown text={originalTopic} />
            </div>
          </div>
          <div className="flex items-center justify-center gap-2 py-8 text-muted-foreground/50 text-sm">
            <Loader2 className="w-4 h-4 animate-spin" />
            <span>Waiting for participants...</span>
          </div>
          {pStatuses.size > 0 && (
            <div className="mt-2 mb-4 rounded-md border border-border/30 bg-card/40 px-3 py-2 space-y-1">
              <p className="text-[9px] font-semibold text-muted-foreground/50 uppercase tracking-widest mb-1">Participant Status</p>
              {Array.from(pStatuses.values()).map((ps) => {
                const knownEngine = normalizeEngine(ps.engine);
                const dotColor = knownEngine ? AGENT_DOT_COLORS[knownEngine] : "bg-muted-foreground/40";
                return (
                  <div key={ps.name} className="flex items-center gap-2 text-[10px]">
                    <span className={cn("w-1.5 h-1.5 rounded-full shrink-0", dotColor)} />
                    <span className="font-medium text-foreground/70 min-w-[60px]">{ps.name}</span>
                    <span className="text-muted-foreground/40">R{ps.round}</span>
                    <span className="flex items-center gap-1 text-primary/60"><Loader2 className="w-2.5 h-2.5 animate-spin" />running</span>
                  </div>
                );
              })}
            </div>
          )}
        </div>
      );
    }
    return (
      <div className="flex items-center justify-center h-full text-muted-foreground/50 text-sm">
        No roundtable messages yet
      </div>
    );
  }

  return (
    <div className="px-5 py-4 max-w-3xl mx-auto w-full">
      {/* Topic + meta */}
      <div className="mb-5 pb-3 border-b border-border/30 space-y-2">
        {originalTopic && (
          <div className="rounded-md bg-accent/30 p-2.5">
            <p className="text-[9px] font-semibold text-muted-foreground/40 uppercase tracking-widest mb-0.5">
              {rounds.length > 1 ? "Original Topic" : "Topic"}
            </p>
            <TopicMarkdown text={originalTopic} />
          </div>
        )}

        <div className="flex items-center gap-2.5 flex-wrap text-[10px] text-muted-foreground/50">
          <span className="flex items-center gap-1"><Users className="w-3 h-3" />{participants.length} participants</span>
          <span className="w-px h-3 bg-border/30" />
          <span>{totalRounds} round{totalRounds > 1 ? "s" : ""}</span>
          <span className="w-px h-3 bg-border/30" />
          <span>{RT_MODE_LABELS[rtMode] ?? rtMode}</span>
        </div>

        <div className="flex items-center gap-1.5 flex-wrap">
          {participants.map(({ name, engine }) => {
            const knownEngine = normalizeEngine(engine);
            const dotColor = knownEngine ? AGENT_DOT_COLORS[knownEngine] : "bg-muted-foreground/40";
            return (
              <span key={name} className="inline-flex items-center gap-1 text-[10px] font-medium text-foreground/60">
                <span className={cn("w-1.5 h-1.5 rounded-full", dotColor)} />{name}
              </span>
            );
          })}
        </div>
      </div>

      {/* Rounds */}
      {rounds.map((round, roundIdx) => {
        const roundParticipants = [...new Set(round.filter((m) => m.persona).map((m) => m.persona!))];
        const roundIntent = roundTopics[roundIdx];
        const showIntent = roundIdx > 0 && roundIntent && roundIntent !== originalTopic;
        const intentSummary = showIntent && roundIntent ? (roundIntent.length > 80 ? roundIntent.slice(0, 80) + "…" : roundIntent) : null;

        return (
          <div key={roundIdx} className="mb-6">
            <div className="flex items-center gap-2.5 mb-2">
              <div className="flex-1 h-px bg-border/20" />
              <span className="text-[9px] font-semibold uppercase tracking-widest text-primary/50 bg-primary/6 px-2 py-0.5 rounded">
                Round {roundIdx + 1}
              </span>
              {roundParticipants.length > 0 && <span className="text-[8px] text-muted-foreground/40">{roundParticipants.join(", ")}</span>}
              {roundIdx === 0 && rounds.length > 1 && rtMode === "deliberative" && <span className="text-[8px] text-muted-foreground/30 italic">independent</span>}
              {roundIdx > 0 && <span className="text-[8px] text-muted-foreground/30 italic">{rtMode === "sequential" ? "builds on prior" : "reflects on prior"}</span>}
              <div className="flex-1 h-px bg-border/20" />
            </div>
            {intentSummary && (
              <div className="mb-3 mx-1 rounded bg-accent/20 px-2.5 py-1.5">
                <p className="text-[9px] text-muted-foreground/50 uppercase tracking-wider mb-0.5">Intent</p>
                <p className="text-[11px] text-foreground/70 leading-relaxed">{intentSummary}</p>
              </div>
            )}
            <div>
              {round.map((msg, i) => (
                <RtMessageCard key={msg.id} message={msg} isLast={i === round.length - 1}
                  onBranch={onBranch} onBranchRT={onBranchRT} onMemo={onMemo} onFollowup={onFollowup}
                  onSaveArtifact={onSaveArtifact} onDelete={onDelete}
                  participantMeta={rtParticipants.find((rp) => rp.name === (msg.persona ?? msg.engine))} />
              ))}
            </div>
          </div>
        );
      })}

      {/* Live participant telemetry */}
      {pStatuses.size > 0 && (
        <div className="mt-2 mb-4 rounded-md border border-border/30 bg-card/40 px-3 py-2 space-y-1">
          <p className="text-[9px] font-semibold text-muted-foreground/50 uppercase tracking-widest mb-1">Participant Status</p>
          {Array.from(pStatuses.values()).map((ps) => {
            const knownEngine = normalizeEngine(ps.engine);
            const dotColor = knownEngine ? AGENT_DOT_COLORS[knownEngine] : "bg-muted-foreground/40";
            return (
              <div key={ps.name} className="flex items-center gap-2 text-[10px]">
                <span className={cn("w-1.5 h-1.5 rounded-full shrink-0", dotColor)} />
                <span className="font-medium text-foreground/70 min-w-[60px]">{ps.name}</span>
                {ps.model && <span className="text-[8px] text-foreground/35 font-mono bg-accent/30 px-0.5 rounded">{ps.model}</span>}
                <span className="text-muted-foreground/40">R{ps.round}</span>
                {ps.status === "running" ? (
                  <span className="flex items-center gap-1 text-primary/60"><Loader2 className="w-2.5 h-2.5 animate-spin" />running</span>
                ) : ps.status === "error" ? (
                  <span className="text-destructive/60">error</span>
                ) : (
                  <span className="text-status-approved/60">done</span>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
