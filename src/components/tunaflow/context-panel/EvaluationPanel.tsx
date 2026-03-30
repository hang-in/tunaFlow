import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { FlaskConical, ChevronRight, ChevronDown, Clock, CheckCircle2, XCircle, RefreshCw } from "lucide-react";
import { AgentAvatar } from "../AgentAvatar";

interface EvalRun {
  id: string;
  conversationId: string;
  title: string;
  prompt: string;
  mode: string | null;
  participants: string | null;
  rounds: number | null;
  status: string;
  createdAt: number;
}

interface EvalResult {
  id: string;
  evalRunId: string;
  agentName: string;
  engine: string;
  round: number;
  content: string;
  inputTokens: number | null;
  outputTokens: number | null;
  costUsd: number | null;
  durationMs: number | null;
  createdAt: number;
}

const STATUS_CONFIG: Record<string, { icon: React.ReactNode; cls: string }> = {
  running: { icon: <Clock className="w-2.5 h-2.5 animate-spin" />, cls: "text-primary/70 bg-primary/10" },
  done: { icon: <CheckCircle2 className="w-2.5 h-2.5" />, cls: "text-status-approved/70 bg-status-approved/10" },
  failed: { icon: <XCircle className="w-2.5 h-2.5" />, cls: "text-status-rejected/70 bg-status-rejected/10" },
};

export function EvaluationPanel() {
  const selectedConversationId = useChatStore((s) => s.selectedConversationId);
  const [runs, setRuns] = useState<EvalRun[]>([]);
  const [selectedRunId, setSelectedRunId] = useState<string | null>(null);
  const [results, setResults] = useState<EvalResult[]>([]);
  const [loading, setLoading] = useState(false);

  // Load runs for current conversation
  useEffect(() => {
    if (!selectedConversationId) { setRuns([]); return; }
    setLoading(true);
    invoke<EvalRun[]>("list_eval_runs", { conversationId: selectedConversationId })
      .then((r) => { setRuns(r); if (r.length > 0 && !selectedRunId) setSelectedRunId(r[0].id); })
      .catch(() => setRuns([]))
      .finally(() => setLoading(false));
  }, [selectedConversationId]);

  // Load results when run selected
  useEffect(() => {
    if (!selectedRunId) { setResults([]); return; }
    invoke<EvalResult[]>("list_eval_results", { evalRunId: selectedRunId })
      .then(setResults)
      .catch(() => setResults([]));
  }, [selectedRunId]);

  const selectedRun = runs.find((r) => r.id === selectedRunId);

  const refresh = () => {
    if (!selectedConversationId) return;
    invoke<EvalRun[]>("list_eval_runs", { conversationId: selectedConversationId })
      .then(setRuns).catch(() => {});
    if (selectedRunId) {
      invoke<EvalResult[]>("list_eval_results", { evalRunId: selectedRunId })
        .then(setResults).catch(() => {});
    }
  };

  // Group results by round
  const roundGroups = results.reduce<Record<number, EvalResult[]>>((acc, r) => {
    (acc[r.round] ??= []).push(r);
    return acc;
  }, {});

  return (
    <div>
      <div className="flex items-center gap-2 mb-4">
        <h2 className="text-[14px] font-[550] text-foreground flex-1">Evaluation</h2>
        <button onClick={refresh}
          className="p-1.5 rounded-md text-muted-foreground/40 hover:text-foreground hover:bg-accent transition-colors">
          <RefreshCw className="w-3.5 h-3.5" />
        </button>
      </div>

      {runs.length === 0 ? (
        <div className="text-center py-8">
          <FlaskConical className="w-6 h-6 text-muted-foreground/20 mx-auto mb-2" />
          <p className="text-[12px] text-muted-foreground/40">No evaluation runs yet</p>
          <p className="text-[11px] text-muted-foreground/30 mt-1">Evaluation runs are created from Roundtable discussions</p>
        </div>
      ) : (
        <div className="flex gap-4 min-h-[300px]">
          {/* Run list */}
          <div className="w-[200px] shrink-0 space-y-1">
            {runs.map((run) => {
              const status = STATUS_CONFIG[run.status] ?? STATUS_CONFIG.done;
              return (
                <button key={run.id} onClick={() => setSelectedRunId(run.id)}
                  className={cn("w-full text-left px-3 py-2 rounded-lg transition-colors",
                    selectedRunId === run.id ? "bg-background text-foreground" : "text-muted-foreground hover:bg-background/50")}>
                  <div className="flex items-center gap-1.5">
                    <span className="text-[12px] font-medium truncate flex-1">{run.title}</span>
                    <span className={cn("inline-flex items-center gap-0.5 text-[9px] px-1 py-0.5 rounded", status.cls)}>
                      {status.icon}
                    </span>
                  </div>
                  <div className="text-[10px] text-muted-foreground/40 mt-0.5">
                    {run.mode ?? "sequential"} · {run.rounds ?? 1}R
                  </div>
                </button>
              );
            })}
          </div>

          {/* Run detail */}
          {selectedRun ? (
            <div className="flex-1 min-w-0 space-y-3">
              {/* Run header */}
              <div className="rounded-lg border border-border/30 bg-background/50 p-3">
                <h3 className="text-[13px] font-medium text-foreground">{selectedRun.title}</h3>
                <p className="text-[11px] text-muted-foreground/60 mt-1 line-clamp-2">{selectedRun.prompt}</p>
                <div className="flex items-center gap-3 mt-2 text-[10px] text-muted-foreground/40">
                  <span>{selectedRun.mode ?? "sequential"}</span>
                  <span>{selectedRun.rounds ?? 1} rounds</span>
                  <span>{new Date(selectedRun.createdAt).toLocaleString()}</span>
                </div>
              </div>

              {/* Results by round */}
              {Object.entries(roundGroups).sort(([a], [b]) => Number(a) - Number(b)).map(([round, roundResults]) => (
                <div key={round}>
                  <h4 className="text-[10px] font-semibold text-muted-foreground/50 uppercase tracking-widest mb-2">
                    Round {round}
                  </h4>
                  <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
                    {roundResults.map((result) => (
                      <div key={result.id} className="rounded-lg border border-border/30 bg-background/50 p-3">
                        <div className="flex items-center gap-2 mb-1.5">
                          <AgentAvatar engine={result.engine} size="xs" />
                          <span className="text-[12px] font-medium text-foreground">{result.agentName}</span>
                          <span className="text-[9px] text-muted-foreground/40 font-mono">{result.engine}</span>
                        </div>
                        <p className="text-[11px] text-foreground/80 leading-relaxed line-clamp-6 whitespace-pre-wrap">
                          {result.content}
                        </p>
                        <div className="flex items-center gap-3 mt-2 text-[9px] text-muted-foreground/30">
                          {result.inputTokens != null && <span>{result.inputTokens.toLocaleString()} in</span>}
                          {result.outputTokens != null && <span>{result.outputTokens.toLocaleString()} out</span>}
                          {result.costUsd != null && <span>${result.costUsd < 0.01 ? result.costUsd.toFixed(4) : result.costUsd.toFixed(2)}</span>}
                          {result.durationMs != null && <span>{(result.durationMs / 1000).toFixed(1)}s</span>}
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
              ))}

              {results.length === 0 && (
                <p className="text-[12px] text-muted-foreground/40 text-center py-4">No results yet</p>
              )}
            </div>
          ) : (
            <div className="flex-1 flex items-center justify-center text-muted-foreground/30 text-[13px]">
              Select an evaluation run
            </div>
          )}
        </div>
      )}
    </div>
  );
}
