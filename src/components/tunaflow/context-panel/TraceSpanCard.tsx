import { cn } from "@/lib/utils";
import { Clock, DollarSign, Package, AlertTriangle, Gauge } from "lucide-react";

interface TraceSpan {
  id: number;
  conversationId: string;
  traceId: string | null;
  spanId: string | null;
  parentSpanId: string | null;
  operation: string | null;
  engine: string | null;
  inputTokens: number;
  outputTokens: number;
  costUsd: number;
  durationMs: number | null;
  status: string | null;
  recordedAt: number;
  contextMode: string | null;
  contextSections: string | null;
  contextLength: number | null;
  contextHash: string | null;
  contextTruncated: number | null;
  messageId: string | null;
}

// ─── Formatting utilities (shared with TracePanel) ─────────────────────────

export function formatDuration(ms: number | null): string {
  if (ms === null) return "—";
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

export function formatCost(usd: number, engine?: string | null): string {
  if (usd === 0) {
    if (engine === "gemini" || engine === "opencode") return "N/A";
    return "$0";
  }
  if (usd < 0.01) return `$${usd.toFixed(4)}`;
  return `$${usd.toFixed(2)}`;
}

export function formatTokens(tokens: number, engine?: string | null): string {
  if (tokens === 0 && engine === "opencode") return "N/A";
  return tokens.toLocaleString();
}

export function formatTime(epoch: number): string {
  const d = new Date(epoch * 1000);
  return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
}

export function baseMode(mode: string): string {
  const idx = mode.indexOf("(");
  return (idx > 0 ? mode.slice(0, idx) : mode).toLowerCase();
}

export function contextModeColor(mode: string): string {
  const m = baseMode(mode);
  if (m === "full") return "bg-purple-500/15 text-purple-400/80";
  if (m === "standard") return "bg-blue-500/15 text-blue-400/80";
  return "bg-muted-foreground/10 text-muted-foreground/60";
}

export function contextModeAbbrev(mode: string): string {
  const m = baseMode(mode);
  if (m === "standard") return "Std";
  if (m === "full") return "Full";
  if (m === "lite") return "Lite";
  return mode.charAt(0).toUpperCase() + mode.slice(1);
}

export function calcTokPerSec(span: TraceSpan): number | null {
  if (!span.durationMs || span.durationMs <= 0 || span.outputTokens <= 0) return null;
  return span.outputTokens / (span.durationMs / 1000);
}

// ─── Context limit resolution ──────────────────────────────────────────────

const MODEL_CONTEXT_LIMITS: Record<string, number> = {
  "claude-opus-4-6": 1_000_000,
  "claude-sonnet-4-6": 200_000,
  "claude-sonnet-4-5": 200_000,
  "claude-haiku-4-5": 200_000,
  "gpt-4o": 128_000,
  "gpt-4o-mini": 128_000,
  "gpt-4.1": 1_000_000,
  "gpt-4.1-mini": 1_000_000,
  "gpt-4.1-nano": 1_000_000,
  "o3": 200_000,
  "o4-mini": 200_000,
  "codex-mini": 1_000_000,
  "gemini-2.5-pro": 1_000_000,
  "gemini-2.5-flash": 1_000_000,
  "gemini-2.0-flash": 1_000_000,
};

export function getContextLimit(model: string | null | undefined): number {
  if (!model) return 200_000;
  if (MODEL_CONTEXT_LIMITS[model]) return MODEL_CONTEXT_LIMITS[model];
  for (const [key, limit] of Object.entries(MODEL_CONTEXT_LIMITS)) {
    if (model.startsWith(key)) return limit;
  }
  return 200_000;
}

function contextPctColor(pct: number): string {
  if (pct >= 90) return "bg-red-500";
  if (pct >= 80) return "bg-orange-500";
  if (pct >= 60) return "bg-yellow-500";
  return "bg-emerald-500";
}

function contextPctTextColor(pct: number): string {
  if (pct >= 90) return "text-red-400";
  if (pct >= 80) return "text-orange-400";
  if (pct >= 60) return "text-yellow-400";
  return "text-muted-foreground/60";
}

// ─── ContextUsageBar ───────────────────────────────────────────────────────

export function ContextUsageBar({ inputTokens, model }: { inputTokens: number; model: string | null | undefined }) {
  if (inputTokens <= 0) return null;
  const limit = getContextLimit(model);
  const pct = Math.min((inputTokens / limit) * 100, 100);
  const limitLabel = limit >= 1_000_000 ? `${(limit / 1_000_000).toFixed(0)}M` : `${(limit / 1_000).toFixed(0)}K`;

  return (
    <div className="flex items-center gap-1.5 text-[8px]">
      <span className={cn("font-mono font-semibold min-w-[28px] text-right", contextPctTextColor(pct))}>
        {pct.toFixed(0)}%
      </span>
      <div className="flex-1 h-1.5 bg-accent/60 rounded-full overflow-hidden max-w-[100px]">
        <div
          className={cn("h-full rounded-full transition-all", contextPctColor(pct))}
          style={{ width: `${pct}%` }}
        />
      </div>
      <span className="text-muted-foreground/30 font-mono">{limitLabel}</span>
      {pct >= 80 && <AlertTriangle className="w-2.5 h-2.5 text-orange-400/70" />}
    </div>
  );
}

// ─── TraceSpanCard ─────────────────────────────────────────────────────────

interface TraceSpanCardProps {
  span: TraceSpan;
  model: string | null;
}

export function TraceSpanCard({ span: sp, model }: TraceSpanCardProps) {
  return (
    <div className="rounded-md border border-border/40 bg-card/50 px-2 py-1.5 text-[10px]">
      <div className="flex items-center gap-1.5 mb-0.5">
        {sp.engine && <span className="font-semibold text-primary/70">{sp.engine}</span>}
        {sp.operation && <span className="text-muted-foreground/50">{sp.operation}</span>}
        <span className="flex-1" />
        <span className={cn(
          "font-medium text-[9px]",
          sp.status === "ok" ? "text-status-approved/70" :
          sp.status === "error" ? "text-status-rejected/70" :
          "text-muted-foreground/40"
        )}>
          {sp.status || "—"}
        </span>
      </div>
      <div className="flex items-center gap-2.5 text-muted-foreground/50">
        <span className="flex items-center gap-0.5">
          <Clock className="w-2.5 h-2.5" />
          {formatDuration(sp.durationMs)}
        </span>
        <span>{formatTokens(sp.inputTokens + sp.outputTokens, sp.engine)} tok</span>
        {(() => {
          const speed = calcTokPerSec(sp);
          return speed != null ? (
            <span className="flex items-center gap-0.5 text-primary/50 font-mono">
              <Gauge className="w-2.5 h-2.5" />
              {speed.toFixed(0)} t/s
            </span>
          ) : null;
        })()}
        <span className="flex items-center gap-0.5">
          <DollarSign className="w-2.5 h-2.5" />
          {formatCost(sp.costUsd, sp.engine)}
        </span>
        <span className="ml-auto">{formatTime(sp.recordedAt)}</span>
      </div>
      {sp.contextMode && (
        <div className="mt-1.5 pt-1.5 border-t border-border/25 space-y-1">
          <div className="flex items-center gap-1.5 flex-wrap text-[9px]">
            <Package className="w-3 h-3 text-muted-foreground/40 shrink-0" />
            <span className={cn("px-1.5 py-px rounded text-[8px] font-semibold", contextModeColor(sp.contextMode))}>
              {sp.contextMode}
            </span>
            {sp.contextSections && (() => {
              try {
                const s = JSON.parse(sp.contextSections) as string[];
                const active = s.filter((sec) => !sec.includes(":skipped"));
                const skipped = s.filter((sec) => sec.includes(":skipped")).map((sec) => sec.replace(":skipped", ""));
                return (
                  <>
                    {active.map((sec) => (
                      <span key={sec} className="bg-accent/60 text-muted-foreground/60 px-1 py-px rounded text-[8px]">{sec}</span>
                    ))}
                    {skipped.map((sec) => (
                      <span key={`skip-${sec}`} className="bg-destructive/10 text-destructive/40 px-1 py-px rounded text-[8px] line-through">{sec}</span>
                    ))}
                  </>
                );
              } catch { return null; }
            })()}
          </div>
          <div className="flex items-center gap-2 text-[8px] text-muted-foreground/40 pl-[18px]">
            {sp.contextLength != null && (
              <span className="font-mono">{(sp.contextLength / 1000).toFixed(1)}k chars</span>
            )}
            {sp.contextTruncated === 1 && (
              <span className="flex items-center gap-0.5 text-amber-500/70 font-medium">
                <AlertTriangle className="w-2.5 h-2.5" />
                truncated
              </span>
            )}
          </div>
          {sp.inputTokens > 0 && (
            <div className="pl-[18px]">
              <ContextUsageBar inputTokens={sp.inputTokens} model={model} />
            </div>
          )}
          {sp.contextHash && sp.contextHash.startsWith("[") && (() => {
            try {
              const sizes = JSON.parse(sp.contextHash) as { name: string; chars: number }[];
              if (sizes.length === 0) return null;
              const sorted = [...sizes].sort((a, b) => b.chars - a.chars);
              const top = sorted.slice(0, 4);
              return (
                <div className="flex items-center gap-1.5 text-[7px] text-muted-foreground/30 pl-[18px] flex-wrap">
                  {top.map((s) => (
                    <span key={s.name} className={cn(
                      "font-mono",
                      s.chars > 8000 ? "text-amber-500/50" : ""
                    )}>
                      {s.name}:{(s.chars / 1000).toFixed(1)}k
                    </span>
                  ))}
                </div>
              );
            } catch { return null; }
          })()}
        </div>
      )}
    </div>
  );
}

export type { TraceSpan };
