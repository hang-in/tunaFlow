import { cn, AGENT_DOT_COLORS, formatTimestamp, normalizeEngine } from "@/lib/utils";
import { AgentAvatar } from "./AgentAvatar";
import type { Message } from "@/types";
import { Copy, Users, Loader2 } from "lucide-react";
import { useState, useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { useChatStore } from "@/stores/chatStore";

interface ParticipantStatus {
  name: string;
  engine: string;
  model: string | null;
  round: number;
  status: "running" | "done" | "error";
  updatedAt: number;
}

interface RoundtableViewProps {
  messages: Message[];
  onBranch?: (messageId: string) => void;
  /** Override conversationId for thread/drawer context */
  conversationId?: string;
}

// ─── Prompt source metadata ──────────────────────────────────────────────────

interface PromptSources {
  round: number;
  totalRounds: number;
  mode: string;
  priorRoundRefs: string[];
  currentRoundRefs: string[];
}

function parsePromptSources(msg: Message): PromptSources | null {
  if (!msg.progressContent) return null;
  try { return JSON.parse(msg.progressContent) as PromptSources; } catch { return null; }
}

function ReferenceBadge({ sources }: { sources: PromptSources }) {
  const hasPrior = sources.priorRoundRefs.length > 0;
  const hasCurrent = sources.currentRoundRefs.length > 0;

  if (!hasPrior && !hasCurrent) {
    return <span className="text-[8px] font-medium px-1 py-0.5 rounded bg-muted text-muted-foreground/50">Independent</span>;
  }

  const refs: string[] = [];
  if (hasPrior) {
    refs.push(sources.priorRoundRefs.length <= 2
      ? sources.priorRoundRefs.map((n) => `← ${n}`).join(", ")
      : `← Round ${sources.round - 1}`);
  }
  if (hasCurrent) {
    refs.push(...sources.currentRoundRefs.map((n) => `← ${n}`));
  }

  return (
    <span className="text-[8px] font-medium text-primary/50 bg-primary/5 px-1 py-0.5 rounded">
      {refs.join(" · ")}
    </span>
  );
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

function parseMarkdown(text: string): React.ReactNode {
  const parts = text.split(/(\*\*[^*]+\*\*)/g);
  return parts.map((part, i) => {
    if (part.startsWith("**") && part.endsWith("**")) {
      return <strong key={i} className="font-semibold text-foreground">{part.slice(2, -2)}</strong>;
    }
    return <span key={i}>{part}</span>;
  });
}

function groupIntoRounds(messages: Message[]): Message[][] {
  const assistantMsgs = messages.filter((m) => m.role === "assistant");
  const hasSystemHeaders = assistantMsgs.some(
    (m) => m.engine === "system" && /^---\s*Round\s+\d+/.test(m.content)
  );

  if (hasSystemHeaders) {
    const rounds: Message[][] = [];
    let currentRound: Message[] = [];
    for (const msg of assistantMsgs) {
      if (msg.engine === "system" && /^---\s*Round\s+\d+/.test(msg.content)) {
        if (currentRound.length > 0) rounds.push(currentRound);
        currentRound = [];
      } else {
        currentRound.push(msg);
      }
    }
    if (currentRound.length > 0) rounds.push(currentRound);
    return rounds;
  }

  const rounds: Message[][] = [];
  let currentRound: Message[] = [];
  const seenPersonas = new Set<string>();
  for (const msg of assistantMsgs) {
    if (msg.engine === "system") continue;
    const persona = msg.persona ?? msg.engine ?? "agent";
    if (seenPersonas.has(persona) && currentRound.length > 0) {
      rounds.push(currentRound);
      currentRound = [msg];
      seenPersonas.clear();
      seenPersonas.add(persona);
    } else {
      currentRound.push(msg);
      seenPersonas.add(persona);
    }
  }
  if (currentRound.length > 0) rounds.push(currentRound);
  return rounds;
}

function getParticipants(messages: Message[]): { name: string; engine: string }[] {
  const seen = new Map<string, string>();
  for (const msg of messages) {
    if (msg.role !== "assistant" || msg.engine === "system") continue;
    const name = msg.persona ?? msg.engine ?? "Agent";
    const engine = msg.engine ?? "claude";
    if (!seen.has(name)) seen.set(name, engine);
  }
  return Array.from(seen.entries()).map(([name, engine]) => ({ name, engine }));
}

// ─── RT Message Card ────────────────────────────────────────────────────────

function RoundtableMessage({ message, isLast }: { message: Message; isLast: boolean }) {
  const [hovered, setHovered] = useState(false);
  const name = message.persona ?? message.engine ?? "Agent";
  const engine = message.engine ?? "";
  const knownEngine = normalizeEngine(engine);
  const dotColor = knownEngine ? AGENT_DOT_COLORS[knownEngine] : "bg-muted-foreground/40";
  const paragraphs = message.content.split("\n\n").filter(Boolean);
  const sources = parsePromptSources(message);

  return (
    <div className="relative flex gap-3">
      {/* Timeline line */}
      {!isLast && (
        <div className="absolute left-[11px] top-7 bottom-0 w-px bg-border/30" />
      )}
      {/* Avatar */}
      <div className="relative z-10 shrink-0">
        <AgentAvatar engine={engine} size="md" />
      </div>
      {/* Card */}
      <div
        className={cn(
          "flex-1 mb-3 rounded-md bg-card/60 border border-border/30 p-3 transition-colors relative",
          hovered && "border-border/50"
        )}
        onMouseEnter={() => setHovered(true)}
        onMouseLeave={() => setHovered(false)}
      >
        {/* Header */}
        <div className="flex items-center gap-1.5 mb-1.5 flex-wrap">
          <span className="inline-flex items-center gap-1">
            <span className={cn("w-1.5 h-1.5 rounded-full", dotColor)} />
            <span className="text-[10px] font-medium text-foreground/80">{name}</span>
          </span>
          {message.model && (
            <span className="text-[8px] text-foreground/40 font-mono bg-accent/40 px-1 py-0.5 rounded">
              {message.model}
            </span>
          )}
          <span className="text-[9px] text-muted-foreground/40 font-mono">
            {formatTimestamp(message.timestamp)}
          </span>
          {sources && <ReferenceBadge sources={sources} />}
        </div>

        {/* Body */}
        <div className="text-[13px] text-foreground/90 leading-relaxed space-y-1.5">
          {paragraphs.map((para, i) => {
            if (para.startsWith("- ")) {
              const items = para.split("\n").filter(Boolean);
              return (
                <ul key={i} className="space-y-0.5 ml-1">
                  {items.map((item, j) => (
                    <li key={j} className="flex gap-1.5">
                      <span className="text-muted-foreground/40 mt-0.5 shrink-0 text-[10px]">•</span>
                      <span>{parseMarkdown(item.replace(/^- /, ""))}</span>
                    </li>
                  ))}
                </ul>
              );
            }
            return <p key={i}>{parseMarkdown(para)}</p>;
          })}
        </div>

        {/* Copy */}
        <div className={cn(
          "absolute right-2 top-2 transition-opacity",
          hovered ? "opacity-100" : "opacity-0 pointer-events-none"
        )}>
          <button
            onClick={() => navigator.clipboard.writeText(message.content)}
            className="p-1 rounded text-muted-foreground/40 hover:text-foreground hover:bg-accent transition-colors"
          >
            <Copy className="w-3 h-3" />
          </button>
        </div>
      </div>
    </div>
  );
}

// ─── Main View ───────────────────────────────────────────────────────────────

const RT_MODE_LABELS: Record<string, string> = {
  sequential: "Sequential",
  deliberative: "Deliberative",
};

export function RoundtableView({ messages, conversationId }: RoundtableViewProps) {
  const participants = getParticipants(messages);
  const rounds = groupIntoRounds(messages);
  const selectedConversationId = useChatStore((s) => s.selectedConversationId);
  const runningThreadIds = useChatStore((s) => s.runningThreadIds);
  const targetConvId = conversationId ?? selectedConversationId;
  const isRunning = !!targetConvId && runningThreadIds.includes(targetConvId);

  // ─── Real-time participant telemetry ─────────────────────────────
  const [pStatuses, setPStatuses] = useState<Map<string, ParticipantStatus>>(new Map());
  const statusesRef = useRef(pStatuses);
  statusesRef.current = pStatuses;

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | null = null;

    listen<{ conversationId: string; name: string; engine: string; model?: string; round: number; status: string }>(
      "roundtable:participant_status",
      (event) => {
        if (cancelled) return;
        const { conversationId, name, engine, model, round, status } = event.payload;
        if (conversationId !== targetConvId) return;
        setPStatuses((prev) => {
          const next = new Map(prev);
          next.set(name, { name, engine, model: model ?? null, round, status: status as ParticipantStatus["status"], updatedAt: Date.now() });
          return next;
        });
      },
    ).then((fn) => { unlisten = fn; });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [targetConvId]);

  // Clear statuses when RT finishes (no more running)
  useEffect(() => {
    if (!isRunning && pStatuses.size > 0) {
      // Keep for 2s after completion so user sees final state, then clear
      const timer = setTimeout(() => setPStatuses(new Map()), 2000);
      return () => clearTimeout(timer);
    }
  }, [isRunning]);

  const userMessages = messages.filter((m) => m.role === "user");
  const originalTopic = userMessages.length > 0 ? userMessages[0].content : null;

  const roundTopics: (string | null)[] = rounds.map((_, i) =>
    i < userMessages.length ? userMessages[i].content : null
  );

  const firstRtMsg = messages.find((m) => m.role === "assistant" && m.engine !== "system" && m.progressContent);
  const firstSources = firstRtMsg ? parsePromptSources(firstRtMsg) : null;
  const totalRounds = firstSources?.totalRounds ?? rounds.length;
  const rtMode = firstSources?.mode ?? "sequential";

  if (rounds.length === 0) {
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
            <p className="text-[13px] text-foreground/90 leading-relaxed">{originalTopic}</p>
          </div>
        )}

        <div className="flex items-center gap-2.5 flex-wrap text-[10px] text-muted-foreground/50">
          <span className="flex items-center gap-1">
            <Users className="w-3 h-3" />
            {participants.length} participants
          </span>
          <span className="w-px h-3 bg-border/30" />
          <span>{totalRounds} round{totalRounds > 1 ? "s" : ""}</span>
          <span className="w-px h-3 bg-border/30" />
          <span>{RT_MODE_LABELS[rtMode] ?? rtMode}</span>
        </div>

        {/* Participant dots */}
        <div className="flex items-center gap-1.5 flex-wrap">
          {participants.map(({ name, engine }) => {
            const knownEngine = normalizeEngine(engine);
            const dotColor = knownEngine ? AGENT_DOT_COLORS[knownEngine] : "bg-muted-foreground/40";
            return (
              <span key={name} className="inline-flex items-center gap-1 text-[10px] font-medium text-foreground/60">
                <span className={cn("w-1.5 h-1.5 rounded-full", dotColor)} />
                {name}
              </span>
            );
          })}
        </div>
      </div>

      {/* Rounds */}
      {rounds.map((round, roundIdx) => {
        const roundParticipants = [...new Set(
          round.filter((m) => m.persona).map((m) => m.persona!)
        )];
        const roundIntent = roundTopics[roundIdx];
        // For Round 1 don't repeat topic (already shown in header)
        const showIntent = roundIdx > 0 && roundIntent && roundIntent !== originalTopic;
        const intentSummary = showIntent
          ? (roundIntent!.length > 80 ? roundIntent!.slice(0, 80) + "…" : roundIntent!)
          : null;

        return (
          <div key={roundIdx} className="mb-6">
            {/* Round divider */}
            <div className="flex items-center gap-2.5 mb-2">
              <div className="flex-1 h-px bg-border/20" />
              <span className="text-[9px] font-semibold uppercase tracking-widest text-primary/50 bg-primary/6 px-2 py-0.5 rounded">
                Round {roundIdx + 1}
              </span>
              {roundParticipants.length > 0 && (
                <span className="text-[8px] text-muted-foreground/40">
                  {roundParticipants.join(", ")}
                </span>
              )}
              {roundIdx === 0 && rounds.length > 1 && rtMode === "deliberative" && (
                <span className="text-[8px] text-muted-foreground/30 italic">independent</span>
              )}
              {roundIdx > 0 && (
                <span className="text-[8px] text-muted-foreground/30 italic">
                  {rtMode === "sequential" ? "builds on prior" : "reflects on prior"}
                </span>
              )}
              <div className="flex-1 h-px bg-border/20" />
            </div>
            {/* Round intent — shown for follow-up rounds with a different prompt */}
            {intentSummary && (
              <div className="mb-3 mx-1 rounded bg-accent/20 px-2.5 py-1.5">
                <p className="text-[9px] text-muted-foreground/50 uppercase tracking-wider mb-0.5">Intent</p>
                <p className="text-[11px] text-foreground/70 leading-relaxed">{intentSummary}</p>
              </div>
            )}

            {/* Messages */}
            <div>
              {round.map((msg, i) => (
                <RoundtableMessage key={msg.id} message={msg} isLast={i === round.length - 1} />
              ))}
            </div>
          </div>
        );
      })}

      {/* ─── Live participant telemetry ─── */}
      {pStatuses.size > 0 && (
        <div className="mt-2 mb-4 rounded-md border border-border/30 bg-card/40 px-3 py-2 space-y-1">
          <p className="text-[9px] font-semibold text-muted-foreground/50 uppercase tracking-widest mb-1">
            Participant Status
          </p>
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
                  <span className="flex items-center gap-1 text-primary/60">
                    <Loader2 className="w-2.5 h-2.5 animate-spin" />
                    running
                  </span>
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
