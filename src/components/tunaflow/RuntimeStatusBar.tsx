import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { Activity, Loader2, Zap } from "lucide-react";
import { TraceModal } from "./TraceModal";
import type { Message } from "@/types";

function SkillsBadge() {
  const activeSkills = useChatStore((s) => s.activeSkills);
  const workflowSkills = useChatStore((s) => s.workflowSkills);
  const totalPhaseSkills = Object.values(workflowSkills).flat().length;
  const count = activeSkills.length;
  if (count === 0 && totalPhaseSkills === 0) return null;
  return (
    <>
      <span className="w-px h-3 bg-border/30" />
      <span className="flex items-center gap-0.5 text-muted-foreground/50">
        <Zap className="w-2.5 h-2.5" />
        <span>{count}s</span>
        {totalPhaseSkills > 0 && <span className="text-primary/40">+wf</span>}
      </span>
    </>
  );
}

/** Known model context window sizes (tokens). Fallback: 200K */
const MODEL_CONTEXT_LIMITS: Record<string, number> = {
  "claude-opus-4-6": 1_000_000, "claude-sonnet-4-6": 200_000, "claude-sonnet-4-5": 200_000, "claude-haiku-4-5": 200_000,
  "gpt-4o": 128_000, "gpt-4o-mini": 128_000, "gpt-4.1": 1_000_000, "gpt-4.1-mini": 1_000_000, "gpt-4.1-nano": 1_000_000,
  "o3": 200_000, "o4-mini": 200_000, "codex-mini": 1_000_000,
  "gemini-2.5-pro": 1_000_000, "gemini-2.5-flash": 1_000_000, "gemini-2.0-flash": 1_000_000,
};

function getContextLimit(model: string | null | undefined): number {
  if (!model) return 200_000;
  if (MODEL_CONTEXT_LIMITS[model]) return MODEL_CONTEXT_LIMITS[model];
  for (const [key, limit] of Object.entries(MODEL_CONTEXT_LIMITS)) {
    if (model.startsWith(key)) return limit;
  }
  return 200_000;
}

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
  const [hourlyCost, setHourlyCost] = useState(0);
  const [lastContextMode, setLastContextMode] = useState<string | null>(null);
  const [lastContextSections, setLastContextSections] = useState(0);
  const [lastSkippedLayers, setLastSkippedLayers] = useState(0);
  const [lastContextPct, setLastContextPct] = useState<number | null>(null);
  const [gitStatus, setGitStatus] = useState<{ branch: string | null; dirty: boolean; added: number; modified: number; untracked: number } | null>(null);
  const [traceOpen, setTraceOpen] = useState(false);

  const isRunning = runningThreadIds.length > 0;
  const runningEngines = [...new Set(jobs.filter((j) => j.status === "running").map((j) => j.engine))];
  const runningJobCount = jobs.filter((j) => j.status === "running").length;

  // Poll active jobs + auto-recover orphan running states
  // Grace period: track when each thread started running to avoid false orphan detection.
  // Between _startRun and DB job creation there's a gap where the thread appears orphaned.
  const [runStartTimes] = useState(() => new Map<string, number>());
  useEffect(() => {
    // Track start times for new running threads
    for (const id of runningThreadIds) {
      if (!runStartTimes.has(id)) runStartTimes.set(id, Date.now());
    }
    // Clean up finished threads
    for (const id of runStartTimes.keys()) {
      if (!runningThreadIds.includes(id)) runStartTimes.delete(id);
    }
  }, [runningThreadIds]);

  useEffect(() => {
    const poll = () => {
      invoke<AgentJob[]>("list_active_jobs").then((fetchedJobs) => {
        setJobs(fetchedJobs);
        // Orphan recovery: if no running jobs in DB but store has runningThreadIds,
        // the agent:completed/error event was missed (e.g., timeout kill during tab switch).
        // Grace period: skip threads that started less than 10s ago (DB job not yet created).
        const now = Date.now();
        const GRACE_MS = 10_000;
        const dbRunning = new Set(fetchedJobs.filter((j) => j.status === "running").map((j) => j.conversationId));
        const storeRunning = useChatStore.getState().runningThreadIds;
        const orphans = storeRunning.filter((id) => {
          if (dbRunning.has(id)) return false;
          const startTime = runStartTimes.get(id) ?? 0;
          return (now - startTime) > GRACE_MS; // only orphan after grace period
        });
        if (orphans.length > 0) {
          console.warn("[orphan-recovery] Clearing stale runningThreadIds:", orphans);
          for (const id of orphans) {
            useChatStore.getState()._endRun(id);
          }
          // Reload current conversation messages to clear "streaming" status
          const convId = useChatStore.getState().selectedConversationId;
          if (convId) {
            invoke<Message[]>("list_messages", { conversationId: convId }).then((msgs) => {
              useChatStore.setState({ messages: msgs });
            }).catch(() => {});
          }
          // Reload thread messages if drawer is open
          const threadConvId = useChatStore.getState().threadBranchConvId;
          if (threadConvId) {
            invoke<Message[]>("list_messages", { conversationId: threadConvId }).then((msgs) => {
              useChatStore.setState({ threadMessages: msgs });
            }).catch(() => {});
          }
        }
      }).catch((e) => console.debug("[jobs]", e));
    };
    poll();
    const timer = setInterval(poll, 2000);
    return () => clearInterval(timer);
  }, [selectedConversationId, runningThreadIds.length]);

  const storeMessages = useChatStore((s) => s.messages);
  const selectedProjectKey = useChatStore((s) => s.selectedProjectKey);

  // Git status — slow poll (10s)
  useEffect(() => {
    if (!selectedProjectKey) { setGitStatus(null); return; }
    let cancelled = false;
    const poll = async () => {
      try {
        const project = await invoke<{ path?: string }>("get_project", { key: selectedProjectKey });
        if (cancelled || !project?.path) return;
        const status = await invoke<{ isRepo: boolean; branch: string | null; dirty: boolean; added: number; modified: number; untracked: number }>(
          "get_git_status", { projectPath: project.path },
        );
        if (!cancelled && status.isRepo) setGitStatus(status);
        else if (!cancelled) setGitStatus(null);
      } catch { if (!cancelled) setGitStatus(null); }
    };
    poll();
    const timer = setInterval(poll, 10000);
    return () => { cancelled = true; clearInterval(timer); };
  }, [selectedProjectKey]);

  // Aggregate cost + last context mode from conversation
  useEffect(() => {
    if (!selectedConversationId) { setTotalCost(0); setLastContextMode(null); setLastContextPct(null); return; }
    invoke<any[]>("list_traces", { conversationId: selectedConversationId, traceId: null })
      .then((spans) => {
        const cost = spans.reduce((sum: number, s: any) => sum + (s.costUsd ?? 0), 0);
        setTotalCost(cost);
        // Hourly cost: totalCost / session duration in hours
        const firstRecordedAt = spans.length > 0
          ? Math.min(...spans.map((s: any) => s.recordedAt ?? Infinity))
          : 0;
        if (firstRecordedAt > 0 && cost > 0) {
          const hours = (Date.now() / 1000 - firstRecordedAt) / 3600;
          setHourlyCost(hours > 0.01 ? cost / hours : 0);
        } else {
          setHourlyCost(0);
        }
        // Find latest span with context metadata
        const withCtx = spans.find((s: any) => s.contextMode);
        if (withCtx) {
          setLastContextMode(withCtx.contextMode);
          try {
            const secs = JSON.parse(withCtx.contextSections || "[]") as string[];
            setLastContextSections(secs.filter((s: string) => !s.includes(":skipped")).length);
            setLastSkippedLayers(secs.filter((s: string) => s.includes(":skipped")).length);
          } catch { setLastContextSections(0); setLastSkippedLayers(0); }
        } else {
          setLastContextMode(null);
          setLastContextSections(0);
        }
        // Context window % — from latest span with inputTokens
        const latestWithTokens = spans.find((s: any) => s.inputTokens > 0);
        if (latestWithTokens) {
          const model = latestWithTokens.messageId
            ? storeMessages.find((m) => m.id === latestWithTokens.messageId)?.model
            : null;
          const limit = getContextLimit(model);
          setLastContextPct(Math.min((latestWithTokens.inputTokens / limit) * 100, 100));
        } else {
          setLastContextPct(null);
        }
      })
      .catch(() => { setTotalCost(0); setLastContextMode(null); setLastContextPct(null); });
  }, [selectedConversationId, runningThreadIds.length, storeMessages.length]);

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
                {(() => { const b = lastContextMode.indexOf("(") > 0 ? lastContextMode.slice(0, lastContextMode.indexOf("(")) : lastContextMode; return b === "Standard" ? "Std" : b; })()}
                {lastContextSections > 0 && <span className="text-muted-foreground/30"> · {lastContextSections}s</span>}
                {lastSkippedLayers > 0 && <span className="text-amber-500/40"> -{lastSkippedLayers}</span>}
              </span>
              {lastContextPct != null && (
                <span className={cn("font-mono text-[9px]",
                  lastContextPct >= 90 ? "text-red-400" :
                  lastContextPct >= 80 ? "text-orange-400" :
                  lastContextPct >= 60 ? "text-yellow-400" :
                  "text-muted-foreground/40"
                )}>
                  {lastContextPct >= 80 ? "\u2757" : lastContextPct >= 60 ? "\u26A0\uFE0F" : "\u{1F9CA}"}{" "}
                  {lastContextPct.toFixed(0)}%
                </span>
              )}
            </>
          )}
          <span className="w-px h-3 bg-border/30" />
          <span>
            {totalCost > 0 ? `$${totalCost < 0.01 ? totalCost.toFixed(4) : totalCost.toFixed(2)}` : "$0"}
            {hourlyCost > 0 && <span className="text-muted-foreground/30 ml-0.5">(${hourlyCost < 0.01 ? hourlyCost.toFixed(3) : hourlyCost.toFixed(2)}/h)</span>}
          </span>
          <SkillsBadge />
        </div>

        {/* Git status */}
        {gitStatus && gitStatus.branch && (
          <>
            <span className="w-px h-3 bg-border/30" />
            <span className="flex items-center gap-1 px-2 text-muted-foreground/50">
              <span>{gitStatus.branch}{gitStatus.dirty ? "*" : ""}</span>
              {(gitStatus.added > 0 || gitStatus.modified > 0 || gitStatus.untracked > 0) && (
                <span className="text-[8px] text-muted-foreground/30">
                  {gitStatus.added > 0 && `+${gitStatus.added}`}{gitStatus.added > 0 && gitStatus.modified > 0 && " "}{gitStatus.modified > 0 && `~${gitStatus.modified}`}{(gitStatus.added > 0 || gitStatus.modified > 0) && gitStatus.untracked > 0 && " "}{gitStatus.untracked > 0 && `?${gitStatus.untracked}`}
                </span>
              )}
            </span>
          </>
        )}

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
