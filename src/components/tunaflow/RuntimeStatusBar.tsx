import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { Activity, Loader2 } from "lucide-react";
import { TraceModal } from "./TraceModal";

interface AgentJob {
  id: string;
  conversationId: string;
  engine: string;
  kind: string;
  status: string;
  startedAt: number;
}

export function RuntimeStatusBar() {
  const selectedConversationId = useChatStore((s) => s.selectedConversationId);
  const runningThreadIds = useChatStore((s) => s.runningThreadIds);
  const rawqStatus = useChatStore((s) => s.rawqStatus);

  const [jobs, setJobs] = useState<AgentJob[]>([]);
  const [totalCost, setTotalCost] = useState(0);
  const [lastContextMode, setLastContextMode] = useState<string | null>(null);
  const [lastContextSections, setLastContextSections] = useState(0);
  const [traceOpen, setTraceOpen] = useState(false);

  const isRunning = runningThreadIds.length > 0;
  const runningEngines = [...new Set(jobs.filter((j) => j.status === "running").map((j) => j.engine))];
  const runningJobCount = jobs.filter((j) => j.status === "running").length;

  // Poll active jobs
  useEffect(() => {
    const poll = () => {
      invoke<AgentJob[]>("list_active_jobs").then(setJobs).catch(() => {});
    };
    poll();
    const timer = setInterval(poll, 2000);
    return () => clearInterval(timer);
  }, [selectedConversationId, runningThreadIds.length]);

  // Aggregate cost + last context mode from conversation
  useEffect(() => {
    if (!selectedConversationId) { setTotalCost(0); setLastContextMode(null); return; }
    invoke<any[]>("list_traces", { conversationId: selectedConversationId, traceId: null })
      .then((spans) => {
        const cost = spans.reduce((sum, s) => sum + (s.costUsd ?? 0), 0);
        setTotalCost(cost);
        // Find latest span with context metadata
        const withCtx = spans.find((s) => s.contextMode);
        if (withCtx) {
          setLastContextMode(withCtx.contextMode);
          try {
            setLastContextSections((JSON.parse(withCtx.contextSections || "[]") as string[]).length);
          } catch { setLastContextSections(0); }
        } else {
          setLastContextMode(null);
          setLastContextSections(0);
        }
      })
      .catch(() => { setTotalCost(0); setLastContextMode(null); });
  }, [selectedConversationId, runningThreadIds.length]);

  return (
    <>
      <div className="flex items-center h-7 shrink-0 text-[10px] text-muted-foreground/60 select-none">
        <span className="flex-1" />

        {/* Trace area — clickable, opens modal */}
        <div
          onClick={() => setTraceOpen(true)}
          className="flex items-center gap-2.5 px-3 h-full cursor-pointer hover:bg-accent/30 rounded-sm transition-colors"
        >
          <span className="flex items-center gap-1">
            {isRunning ? (
              <>
                <Loader2 className="w-3 h-3 animate-spin text-primary" />
                <span className="text-primary/70 font-medium truncate">
                  {runningEngines.join(", ") || "running"}
                </span>
              </>
            ) : (
              <>
                <Activity className="w-3 h-3 text-muted-foreground/30" />
                <span>Idle</span>
              </>
            )}
          </span>
          <span className="w-px h-3 bg-border/30" />
          <span>{runningJobCount} jobs</span>
          {lastContextMode && (
            <>
              <span className="w-px h-3 bg-border/30" />
              <span className="text-muted-foreground/50">
                {lastContextMode === "Standard" ? "Std" : lastContextMode}
                {lastContextSections > 0 && <span className="text-muted-foreground/30"> · {lastContextSections}s</span>}
              </span>
            </>
          )}
          <span className="w-px h-3 bg-border/30" />
          <span>{totalCost > 0 ? `$${totalCost < 0.01 ? totalCost.toFixed(4) : totalCost.toFixed(2)}` : "$0"}</span>
        </div>

        {/* Separator */}
        <span className="w-px h-3 bg-border/30" />

        {/* rawq status — right edge */}
        <div className="flex items-center gap-1.5 px-3 h-full">
          {rawqStatus && (
            <>
              <span className={cn("w-1.5 h-1.5 rounded-full shrink-0",
                rawqStatus.status === "ready" || rawqStatus.status === "built" ? "bg-status-approved"
                : rawqStatus.status === "indexing" ? "bg-primary animate-pulse"
                : rawqStatus.status === "unavailable" ? "bg-muted-foreground/15" : "bg-status-rejected"
              )} />
              <span className="truncate max-w-[120px]">{rawqStatus.message}</span>
            </>
          )}
        </div>
      </div>

      {traceOpen && <TraceModal onClose={() => setTraceOpen(false)} />}
    </>
  );
}
