/**
 * QualityDashboard — ContextPack 품질 관측 대시보드 (M scope, s37).
 *
 * `trace_log` 에 이미 누적된 필드(mode/sections/length/truncated/cache tokens)를
 * 집계해 사용자가 "지금 ContextPack 가 어떻게 작동하는가" 를 한 화면에서
 * 보도록 한다. 회귀 회귀 테스트(골든셋)는 별도 작업(Phase 6) 이므로 여기서는
 * 관측에 집중.
 *
 * 지표:
 *  - **Mode 분포** — Lite/Standard/Full 비율 (auto mode 튜닝이 잘 됐는지)
 *  - **Truncation rate** — budget cap 에 걸린 비율 (높으면 budget 올리거나
 *    섹션 cap 조정 필요 신호)
 *  - **Hit rate** — retrieval 이 ≥1 chunk 를 실제로 가져온 쿼리 비율
 *    (0% 가까우면 retrieval 자체가 유명무실)
 *  - **Section size Pareto** — 평균적으로 어느 섹션이 토큰을 가장 많이
 *    먹는지 (tuning 타깃 식별)
 *  - **Cache hit rate** — (PR #50 이후 값이 실제로 채워지면 의미) prompt cache
 *    적중률. 지금은 placeholder — 실측 값 파이프라인 후속 PR.
 */
import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useChatStore } from "@/stores/chatStore";
import { cn } from "@/lib/utils";
import { RefreshCw, AlertTriangle, TrendingUp, Package } from "lucide-react";
import type { TraceSpan } from "./TraceSpanCard";

interface Stats {
  total: number;
  modeDist: Record<string, number>;
  truncated: number;
  hitRate: number;
  avgLen: number;
  sectionAvg: Array<{ name: string; avgChars: number }>;
  cacheReadTotal: number;
  cacheCreateTotal: number;
  inputTotal: number;
}

function contextModeShort(mode: string | null): string {
  if (!mode) return "unknown";
  // Mode strings look like "Standard(auto:standard(baseline))"  or "Lite", etc.
  const m = mode.match(/^(Lite|Standard|Full)/);
  return m ? m[1].toLowerCase() : mode.slice(0, 10);
}

function computeStats(spans: TraceSpan[]): Stats {
  const total = spans.length;
  const modeDist: Record<string, number> = {};
  let truncated = 0;
  let withRetrieval = 0;
  let lenSum = 0;
  let cacheReadTotal = 0;
  let cacheCreateTotal = 0;
  let inputTotal = 0;
  const sectionChars: Record<string, { sum: number; count: number }> = {};

  for (const sp of spans) {
    const mode = contextModeShort(sp.contextMode);
    modeDist[mode] = (modeDist[mode] || 0) + 1;
    if (sp.contextTruncated === 1) truncated++;
    if (sp.contextLength != null) lenSum += sp.contextLength;
    inputTotal += sp.inputTokens;
    // Retrieval hit — section list contains "retrieval" or "rawq" marker.
    if (sp.contextSections) {
      try {
        const s = JSON.parse(sp.contextSections) as string[];
        if (s.some((x) => x === "retrieval" || x === "rawq")) withRetrieval++;
      } catch { /* ignore */ }
    }
    // Cache tokens — populated starting from PR #50+. May be null on legacy rows.
    const cr = (sp as TraceSpan & { cacheReadTokens?: number | null }).cacheReadTokens;
    const cc = (sp as TraceSpan & { cacheCreationTokens?: number | null }).cacheCreationTokens;
    if (typeof cr === "number") cacheReadTotal += cr;
    if (typeof cc === "number") cacheCreateTotal += cc;
    // Section sizes — stored in `contextHash` field (see prompt_assembly.rs)
    if (sp.contextHash && sp.contextHash.startsWith("[")) {
      try {
        const sizes = JSON.parse(sp.contextHash) as { name: string; chars: number }[];
        for (const { name, chars } of sizes) {
          if (chars <= 0) continue;
          const cur = sectionChars[name] || { sum: 0, count: 0 };
          cur.sum += chars;
          cur.count += 1;
          sectionChars[name] = cur;
        }
      } catch { /* ignore */ }
    }
  }

  const sectionAvg = Object.entries(sectionChars)
    .map(([name, { sum, count }]) => ({ name, avgChars: Math.round(sum / count) }))
    .sort((a, b) => b.avgChars - a.avgChars);

  return {
    total,
    modeDist,
    truncated,
    hitRate: total > 0 ? withRetrieval / total : 0,
    avgLen: total > 0 ? Math.round(lenSum / total) : 0,
    sectionAvg,
    cacheReadTotal,
    cacheCreateTotal,
    inputTotal,
  };
}

const MODE_COLORS: Record<string, string> = {
  lite: "bg-muted-foreground/40",
  standard: "bg-blue-500/60",
  full: "bg-fuchsia-500/60",
  unknown: "bg-muted-foreground/20",
};

function StatCard({ label, value, hint, tone }: { label: string; value: string; hint?: string; tone?: "ok" | "warn" | "alert" }) {
  const toneCls = tone === "alert" ? "text-destructive"
    : tone === "warn" ? "text-amber-500"
    : "text-foreground";
  return (
    <div className="rounded-md border border-border/40 bg-card/50 px-3 py-2">
      <div className="text-[9px] uppercase tracking-wider text-muted-foreground/50">{label}</div>
      <div className={cn("text-[16px] font-semibold mt-0.5", toneCls)}>{value}</div>
      {hint && <div className="text-[9px] text-muted-foreground/40 mt-0.5">{hint}</div>}
    </div>
  );
}

export function QualityDashboard() {
  const selectedConversationId = useChatStore((s) => s.selectedConversationId);
  const selectedProjectKey = useChatStore((s) => s.selectedProjectKey);
  const [spans, setSpans] = useState<TraceSpan[]>([]);
  const [loading, setLoading] = useState(false);
  const [scope, setScope] = useState<"conversation" | "project">("conversation");

  const reload = async () => {
    setLoading(true);
    try {
      if (scope === "conversation" && selectedConversationId) {
        const rows = await invoke<TraceSpan[]>("list_traces", { conversationId: selectedConversationId, traceId: null });
        setSpans(rows);
      } else if (scope === "project" && selectedProjectKey) {
        // No per-project list_traces command yet — aggregate by iterating
        // conversations is expensive. For now fall back to current conv; a
        // dedicated backend command can be added as a follow-up (it's
        // essentially `SELECT ... FROM trace_log JOIN conversations WHERE project_key = ?`).
        const rows = selectedConversationId
          ? await invoke<TraceSpan[]>("list_traces", { conversationId: selectedConversationId, traceId: null })
          : [];
        setSpans(rows);
      } else {
        setSpans([]);
      }
    } catch (e) {
      console.debug("[quality]", e);
      setSpans([]);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => { reload(); /* eslint-disable-next-line react-hooks/exhaustive-deps */ }, [selectedConversationId, scope]);

  const stats = useMemo(() => computeStats(spans), [spans]);

  if (!selectedConversationId) {
    return <p className="text-xs text-muted-foreground px-2">No conversation selected.</p>;
  }

  const truncRate = stats.total > 0 ? stats.truncated / stats.total : 0;
  const cacheHitRate = stats.inputTotal > 0
    ? stats.cacheReadTotal / stats.inputTotal
    : 0;

  return (
    <div className="space-y-3">
      {/* Header */}
      <div className="flex items-center gap-2">
        <TrendingUp className="w-3.5 h-3.5 text-muted-foreground/50" />
        <span className="text-[11px] font-semibold text-foreground">Quality</span>
        <span className="text-[9px] text-muted-foreground/40">· {stats.total} spans</span>
        <button
          onClick={reload}
          disabled={loading}
          className="ml-auto p-0.5 text-muted-foreground/40 hover:text-muted-foreground transition-colors"
          title="Refresh"
        >
          <RefreshCw className={cn("w-3 h-3", loading && "animate-spin")} />
        </button>
      </div>

      {stats.total === 0 ? (
        <p className="text-[10px] text-muted-foreground/50 px-0.5">
          {loading ? "Loading..." : "No trace data yet."}
        </p>
      ) : (
        <>
          {/* Stat cards row */}
          <div className="grid grid-cols-2 gap-1.5">
            <StatCard
              label="Truncation"
              value={`${(truncRate * 100).toFixed(0)}%`}
              hint={`${stats.truncated} / ${stats.total} hit budget cap`}
              tone={truncRate > 0.3 ? "alert" : truncRate > 0.1 ? "warn" : "ok"}
            />
            <StatCard
              label="Retrieval hit"
              value={`${(stats.hitRate * 100).toFixed(0)}%`}
              hint={`queries returning ≥1 chunk`}
              tone={stats.hitRate < 0.2 ? "warn" : "ok"}
            />
            <StatCard
              label="Avg ContextPack"
              value={`${(stats.avgLen / 1000).toFixed(1)}k`}
              hint={`chars per send`}
            />
            <StatCard
              label="Cache read"
              value={stats.inputTotal > 0 ? `${(cacheHitRate * 100).toFixed(0)}%` : "—"}
              hint={stats.cacheReadTotal > 0
                ? `${stats.cacheReadTotal.toLocaleString()} tokens cached`
                : `needs provider population`}
              tone={stats.cacheReadTotal === 0 ? undefined : cacheHitRate > 0.5 ? "ok" : "warn"}
            />
          </div>

          {/* Mode distribution */}
          <div className="rounded-md border border-border/40 bg-card/50 px-3 py-2 space-y-1.5">
            <div className="flex items-center gap-1.5 text-[9px] uppercase tracking-wider text-muted-foreground/50">
              <Package className="w-3 h-3" />
              ContextPack mode
            </div>
            <div className="flex h-2 rounded overflow-hidden">
              {Object.entries(stats.modeDist).map(([mode, count]) => (
                <div
                  key={mode}
                  className={MODE_COLORS[mode] ?? "bg-muted-foreground/30"}
                  style={{ width: `${(count / stats.total) * 100}%` }}
                  title={`${mode}: ${count} (${((count / stats.total) * 100).toFixed(0)}%)`}
                />
              ))}
            </div>
            <div className="flex items-center gap-2 text-[9px] text-muted-foreground/60">
              {Object.entries(stats.modeDist).map(([mode, count]) => (
                <span key={mode} className="flex items-center gap-1">
                  <span className={cn("w-2 h-2 rounded-sm", MODE_COLORS[mode] ?? "bg-muted-foreground/30")} />
                  <span className="capitalize">{mode}</span>
                  <span className="text-muted-foreground/40 font-mono">{count}</span>
                </span>
              ))}
            </div>
          </div>

          {/* Section pareto */}
          {stats.sectionAvg.length > 0 && (
            <div className="rounded-md border border-border/40 bg-card/50 px-3 py-2 space-y-1.5">
              <div className="flex items-center gap-1.5 text-[9px] uppercase tracking-wider text-muted-foreground/50">
                Section avg size (per send)
              </div>
              {(() => {
                const max = stats.sectionAvg[0]?.avgChars ?? 1;
                return stats.sectionAvg.slice(0, 8).map((s) => (
                  <div key={s.name} className="flex items-center gap-2 text-[10px]">
                    <span className="text-muted-foreground/70 font-mono w-20 truncate">{s.name}</span>
                    <div className="flex-1 h-1.5 bg-muted/30 rounded overflow-hidden">
                      <div
                        className={cn("h-full rounded",
                          s.avgChars > 8000 ? "bg-amber-500/60" : "bg-primary/50"
                        )}
                        style={{ width: `${(s.avgChars / max) * 100}%` }}
                      />
                    </div>
                    <span className="text-muted-foreground/50 font-mono w-14 text-right">
                      {(s.avgChars / 1000).toFixed(1)}k
                    </span>
                  </div>
                ));
              })()}
            </div>
          )}

          {truncRate > 0.3 && (
            <div className="rounded-md border border-destructive/30 bg-destructive/5 px-3 py-2 text-[10px] flex items-start gap-1.5">
              <AlertTriangle className="w-3 h-3 text-destructive shrink-0 mt-0.5" />
              <span className="text-destructive/80">
                Truncation rate 30% 초과 — ContextPack 의 budget cap 조정 또는
                섹션별 max_chars 재조정 검토.
              </span>
            </div>
          )}
        </>
      )}

      {/* Scope switcher — future-proofed (project scope currently mirrors conversation until backend command lands) */}
      <div className="flex gap-1 text-[9px] text-muted-foreground/40">
        <button
          onClick={() => setScope("conversation")}
          className={cn(
            "px-1.5 py-0.5 rounded transition-colors",
            scope === "conversation" ? "bg-accent text-foreground" : "hover:text-muted-foreground",
          )}
        >
          Conversation
        </button>
        <button
          onClick={() => setScope("project")}
          disabled
          title="Project scope — per-project trace aggregation lands in a follow-up"
          className={cn(
            "px-1.5 py-0.5 rounded transition-colors opacity-40 cursor-not-allowed",
            scope === "project" ? "bg-accent text-foreground" : "",
          )}
        >
          Project
        </button>
      </div>
    </div>
  );
}
