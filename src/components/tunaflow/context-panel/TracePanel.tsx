import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { Activity, Clock, Cpu, DollarSign, RefreshCw, Briefcase, ChevronDown, ChevronRight, Zap, Package, AlertTriangle, Brain } from "lucide-react";

interface AgentJob {
  id: string;
  conversationId: string;
  messageId: string | null;
  engine: string;
  kind: string;
  status: string;
  error: string | null;
  startedAt: number;
  updatedAt: number;
}

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
}

function formatDuration(ms: number | null): string {
  if (ms === null) return "—";
  if (ms < 1000) return `${ms}ms`;
  return `${(ms / 1000).toFixed(1)}s`;
}

function formatCost(usd: number, engine?: string | null): string {
  if (usd === 0) {
    if (engine === "gemini" || engine === "opencode") return "N/A";
    return "$0";
  }
  if (usd < 0.01) return `$${usd.toFixed(4)}`;
  return `$${usd.toFixed(2)}`;
}

function formatTokens(tokens: number, engine?: string | null): string {
  if (tokens === 0 && engine === "opencode") return "N/A";
  return tokens.toLocaleString();
}

function formatTime(epoch: number): string {
  const d = new Date(epoch * 1000);
  return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
}

/** Extract base mode name from trace format, e.g. "Standard(auto:standard(baseline))" → "standard" */
function baseMode(mode: string): string {
  const idx = mode.indexOf("(");
  return (idx > 0 ? mode.slice(0, idx) : mode).toLowerCase();
}

function contextModeColor(mode: string): string {
  const m = baseMode(mode);
  if (m === "full") return "bg-purple-500/15 text-purple-400/80";
  if (m === "standard") return "bg-blue-500/15 text-blue-400/80";
  return "bg-muted-foreground/10 text-muted-foreground/60"; // Lite
}

function contextModeAbbrev(mode: string): string {
  const m = baseMode(mode);
  if (m === "standard") return "Std";
  if (m === "full") return "Full";
  if (m === "lite") return "Lite";
  return mode.charAt(0).toUpperCase() + mode.slice(1);
}

function formatElapsed(startedAt: number): string {
  const elapsedMs = Date.now() - startedAt;
  if (elapsedMs < 0) return "0s";
  const elapsed = Math.floor(elapsedMs / 1000);
  if (elapsed < 60) return `${elapsed}s`;
  return `${Math.floor(elapsed / 60)}m ${elapsed % 60}s`;
}

export function TracePanel() {
  const selectedConversationId = useChatStore((s) => s.selectedConversationId);
  const activeBranchId = useChatStore((s) => s.activeBranchId);
  const runningThreadIds = useChatStore((s) => s.runningThreadIds);
  const messageQueue = useChatStore((s) => s.messageQueue);
  const rawqStatus = useChatStore((s) => s.rawqStatus);
  const activeSkills = useChatStore((s) => s.activeSkills);

  const [spans, setSpans] = useState<TraceSpan[]>([]);
  const [jobs, setJobs] = useState<AgentJob[]>([]);
  const [loading, setLoading] = useState(false);
  const [historyOpen, setHistoryOpen] = useState(false);
  const [tick, setTick] = useState(0);
  const [memoryStatus, setMemoryStatus] = useState<{
    state: string; sourceCount: number | null; messageCount: number;
    createdAt: number | null; updatedAt: number | null;
    newMessagesSince: number; summaryLength: number | null;
    topicCount: number; provenance: string | null; modelUsed: string | null;
  } | null>(null);

  const convId = activeBranchId
    ? `branch:${activeBranchId}`
    : selectedConversationId;

  const loadTraces = async () => {
    if (!convId) return;
    setLoading(true);
    try {
      const data = await invoke<TraceSpan[]>("list_traces", { conversationId: convId, traceId: null });
      setSpans(data);
    } catch { setSpans([]); }
    finally { setLoading(false); }
  };

  const loadJobs = async () => {
    try {
      const data = await invoke<AgentJob[]>("list_active_jobs");
      setJobs(data);
    } catch { setJobs([]); }
  };

  const loadMemoryStatus = async () => {
    if (!convId) return;
    try {
      const s = await invoke<typeof memoryStatus>("get_conversation_memory_status", { conversationId: convId });
      setMemoryStatus(s);
    } catch { setMemoryStatus(null); }
  };

  useEffect(() => {
    loadTraces();
    loadJobs();
    loadMemoryStatus();
  }, [convId]);

  // Auto-refresh jobs while running + tick for elapsed counter
  const threadRunning = convId ? runningThreadIds.includes(convId) : false;
  useEffect(() => {
    if (!threadRunning) return;
    const interval = setInterval(() => { loadJobs(); setTick((t) => t + 1); }, 1000);
    return () => clearInterval(interval);
  }, [threadRunning]);

  const queuedCount = convId
    ? messageQueue.filter((q) => q.threadId === convId).length
    : 0;

  // Aggregate stats — total + per-engine
  const totalInputTokens = spans.reduce((s, sp) => s + sp.inputTokens, 0);
  const totalOutputTokens = spans.reduce((s, sp) => s + sp.outputTokens, 0);
  const totalCost = spans.reduce((s, sp) => s + sp.costUsd, 0);

  const engineAggregates = (() => {
    const map = new Map<string, { input: number; output: number; cost: number; count: number }>();
    for (const sp of spans) {
      const eng = sp.engine || "unknown";
      const prev = map.get(eng) || { input: 0, output: 0, cost: 0, count: 0 };
      map.set(eng, {
        input: prev.input + sp.inputTokens,
        output: prev.output + sp.outputTokens,
        cost: prev.cost + sp.costUsd,
        count: prev.count + 1,
      });
    }
    return [...map.entries()].sort((a, b) => b[1].count - a[1].count);
  })();

  if (!convId) {
    return <p className="text-xs text-muted-foreground px-2">No conversation selected.</p>;
  }

  return (
    <div className="space-y-3">
      {/* ═══ RUNTIME STATUS (primary section) ═══ */}

      {/* Live indicator */}
      <div className="flex items-center gap-2">
        <span className={cn(
          "w-2.5 h-2.5 rounded-full shrink-0",
          threadRunning ? "bg-primary animate-pulse" : "bg-muted-foreground/20"
        )} />
        <span className={cn("text-[12px] font-semibold", threadRunning ? "text-primary" : "text-muted-foreground/60")}>
          {threadRunning ? "Running" : "Idle"}
        </span>
        {queuedCount > 0 && (
          <span className="text-[9px] text-muted-foreground bg-accent px-1.5 py-0.5 rounded-full">
            {queuedCount} queued
          </span>
        )}
      </div>

      {/* Active jobs — prominent cards */}
      {jobs.length > 0 && (
        <div className="space-y-1.5">
          {jobs.map((j) => (
            <div key={j.id} className="rounded-lg border border-primary/25 bg-primary/5 px-3 py-2">
              <div className="flex items-center gap-2">
                <span className="w-2 h-2 rounded-full bg-primary animate-pulse shrink-0" />
                <span className="text-[11px] font-semibold text-foreground">{j.engine}</span>
                <span className="text-[10px] text-muted-foreground/50">{j.kind}</span>
                <span className="ml-auto text-[10px] font-mono text-primary/60">{/* tick={tick} triggers re-render */}{formatElapsed(j.startedAt)}</span>
              </div>
              {j.error && <p className="text-[9px] text-destructive/60 mt-1 truncate">{j.error}</p>}
            </div>
          ))}
        </div>
      )}

      {/* Runtime context — rawq + skills */}
      <div className="space-y-1 px-0.5">
        {rawqStatus && (
          <div className="flex items-center gap-1.5 text-[10px] text-muted-foreground/60">
            <Cpu className="w-3 h-3 shrink-0" />
            <span>rawq</span>
            <span className={cn(
              "px-1 py-px rounded text-[8px] font-medium",
              rawqStatus.status === "ready" ? "bg-status-approved/10 text-status-approved/70" :
              rawqStatus.status === "error" ? "bg-destructive/10 text-destructive/70" :
              "bg-accent text-muted-foreground/60"
            )}>
              {rawqStatus.status}
            </span>
            {rawqStatus.files != null && (
              <span className="text-muted-foreground/40">{rawqStatus.files} files</span>
            )}
          </div>
        )}
        {activeSkills.length > 0 && (
          <div className="flex items-center gap-1.5 text-[10px] text-muted-foreground/60">
            <Zap className="w-3 h-3 shrink-0" />
            <span>{activeSkills.length} skill{activeSkills.length > 1 ? "s" : ""} active</span>
          </div>
        )}
        {memoryStatus && (
          <div className="flex items-center gap-1.5 text-[10px] text-muted-foreground/60 flex-wrap">
            <Brain className="w-3 h-3 shrink-0" />
            <span>memory</span>
            <span className={cn(
              "px-1 py-px rounded text-[8px] font-medium",
              memoryStatus.state === "fresh" ? "bg-status-approved/10 text-status-approved/70" :
              memoryStatus.state === "stale" ? "bg-amber-500/10 text-amber-500/70" :
              "bg-accent text-muted-foreground/60"
            )}>
              {memoryStatus.state}
            </span>
            {memoryStatus.topicCount > 0 && (
              <span className="text-muted-foreground/40">{memoryStatus.topicCount} topic{memoryStatus.topicCount > 1 ? "s" : ""}</span>
            )}
            {memoryStatus.sourceCount != null && (
              <span className="text-muted-foreground/40">{memoryStatus.sourceCount}/{memoryStatus.messageCount} msgs</span>
            )}
            {memoryStatus.state === "stale" && memoryStatus.newMessagesSince > 0 && (
              <span className="text-amber-500/40">+{memoryStatus.newMessagesSince} new</span>
            )}
            {memoryStatus.provenance && memoryStatus.provenance !== "auto" && (
              <span className="px-1 py-px rounded text-[8px] bg-accent text-muted-foreground/50">{memoryStatus.provenance}</span>
            )}
            {(memoryStatus.state === "stale" || memoryStatus.state === "not_generated") && (
              <button
                className="px-1.5 py-px rounded text-[8px] font-medium bg-accent hover:bg-accent/80 text-muted-foreground/70 transition-colors"
                onClick={async () => {
                  if (!convId) return;
                  try {
                    await invoke("force_recompress_memory", { conversationId: convId });
                    loadMemoryStatus();
                  } catch (e) { console.error("[TracePanel] recompress failed", e); }
                }}
              >
                {memoryStatus.state === "not_generated" ? "compress" : "recompress"}
              </button>
            )}
          </div>
        )}
      </div>

      {/* Aggregate stats — total */}
      {spans.length > 0 && (
        <div className="space-y-1.5">
          <div className="grid grid-cols-3 gap-1.5">
            <div className="rounded-md bg-accent/50 px-2 py-1.5 text-center">
              <p className="text-[8px] text-muted-foreground/60 uppercase">Input</p>
              <p className="text-[11px] font-semibold text-foreground">{totalInputTokens.toLocaleString()}</p>
            </div>
            <div className="rounded-md bg-accent/50 px-2 py-1.5 text-center">
              <p className="text-[8px] text-muted-foreground/60 uppercase">Output</p>
              <p className="text-[11px] font-semibold text-foreground">{totalOutputTokens.toLocaleString()}</p>
            </div>
            <div className="rounded-md bg-accent/50 px-2 py-1.5 text-center">
              <p className="text-[8px] text-muted-foreground/60 uppercase">Cost</p>
              <p className="text-[11px] font-semibold text-foreground">{formatCost(totalCost, null)}</p>
            </div>
          </div>

          {/* Per-engine breakdown */}
          {engineAggregates.length > 1 && (
            <div className="space-y-0.5">
              {engineAggregates.map(([eng, agg]) => (
                <div key={eng} className="flex items-center gap-2 px-1 text-[9px] text-muted-foreground/50">
                  <span className="font-medium text-foreground/60 w-14 truncate">{eng}</span>
                  <span className="flex-1 text-right font-mono">{formatTokens(agg.input + agg.output, eng)} tok</span>
                  <span className="w-12 text-right font-mono">{formatCost(agg.cost, eng)}</span>
                  <span className="w-6 text-right text-muted-foreground/30">{agg.count}×</span>
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {/* Last context summary */}
      {(() => {
        const latest = spans.find((sp) => sp.contextMode);
        if (!latest) return null;
        const allSections = (() => {
          try { return JSON.parse(latest.contextSections || "[]") as string[]; } catch { return [] as string[]; }
        })();
        const activeCount = allSections.filter((s) => !s.includes(":skipped")).length;
        const skippedCount = allSections.filter((s) => s.includes(":skipped")).length;
        return (
          <button
            onClick={() => { if (!historyOpen) setHistoryOpen(true); }}
            className="flex items-center gap-1.5 w-full text-[9px] text-muted-foreground/50 hover:text-muted-foreground/70 transition-colors px-0.5"
          >
            <Package className="w-3 h-3 shrink-0" />
            <span className={cn("px-1 py-px rounded text-[8px] font-semibold", contextModeColor(latest.contextMode!))}>
              {contextModeAbbrev(latest.contextMode!)}
            </span>
            <span>· {activeCount} active{skippedCount > 0 ? `, ${skippedCount} skipped` : ""}</span>
            {latest.contextLength != null && (
              <span className="font-mono">· {(latest.contextLength / 1000).toFixed(1)}k</span>
            )}
            {latest.contextTruncated === 1 && (
              <span className="text-amber-500/60">· truncated</span>
            )}
          </button>
        );
      })()}

      {/* ═══ TRACE HISTORY (collapsible) ═══ */}
      <div className="border-t border-border/20 pt-2">
        <button
          onClick={() => { setHistoryOpen(!historyOpen); if (!historyOpen) loadTraces(); }}
          className="flex items-center gap-1.5 w-full text-left hover:bg-accent/30 rounded px-1 py-0.5 transition-colors"
        >
          {historyOpen
            ? <ChevronDown className="w-3 h-3 text-muted-foreground/50" />
            : <ChevronRight className="w-3 h-3 text-muted-foreground/50" />
          }
          <span className="text-[10px] font-semibold text-muted-foreground/50 uppercase tracking-wider flex-1">
            Trace History
          </span>
          <span className="text-[9px] text-muted-foreground/30">{spans.length}</span>
          <button
            onClick={(e) => { e.stopPropagation(); loadTraces(); loadJobs(); }}
            disabled={loading}
            className="p-0.5 text-muted-foreground/40 hover:text-muted-foreground transition-colors"
            title="Refresh"
          >
            <RefreshCw className={cn("w-3 h-3", loading && "animate-spin")} />
          </button>
        </button>

        {historyOpen && (
          <div className="mt-1.5 space-y-1">
            {loading && spans.length === 0 && (
              <p className="text-[10px] text-muted-foreground px-1">Loading...</p>
            )}

            {!loading && spans.length === 0 && (
              <div className="text-center py-3">
                <Activity className="w-4 h-4 text-muted-foreground/30 mx-auto mb-1" />
                <p className="text-[10px] text-muted-foreground/50">No trace data yet.</p>
              </div>
            )}

            {spans.map((sp) => (
              <div
                key={sp.id}
                className="rounded-md border border-border/40 bg-card/50 px-2 py-1.5 text-[10px]"
              >
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
                    {/* Section budget breakdown — from contextHash */}
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
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
