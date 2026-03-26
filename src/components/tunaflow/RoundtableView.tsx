import { cn, AGENT_DOT_COLORS, formatTimestamp, normalizeEngine } from "@/lib/utils";
import { AgentAvatar } from "./AgentAvatar";
import type { Message } from "@/types";
import { Copy, Users } from "lucide-react";
import { useState } from "react";

interface RoundtableViewProps {
  messages: Message[];
  onBranch?: (messageId: string) => void;
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

export function RoundtableView({ messages }: RoundtableViewProps) {
  const participants = getParticipants(messages);
  const rounds = groupIntoRounds(messages);

  const userMessages = messages.filter((m) => m.role === "user");
  const topic = userMessages.length > 0 ? userMessages[userMessages.length - 1].content : null;

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
        {topic && (
          <div className="rounded-md bg-accent/30 p-2.5">
            <p className="text-[9px] font-semibold text-muted-foreground/40 uppercase tracking-widest mb-0.5">Topic</p>
            <p className="text-[13px] text-foreground/90 leading-relaxed">{topic}</p>
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

        return (
          <div key={roundIdx} className="mb-6">
            {/* Round divider */}
            <div className="flex items-center gap-2.5 mb-4">
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

            {/* Messages */}
            <div>
              {round.map((msg, i) => (
                <RoundtableMessage key={msg.id} message={msg} isLast={i === round.length - 1} />
              ))}
            </div>
          </div>
        );
      })}
    </div>
  );
}
