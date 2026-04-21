import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { usePtyStore } from "@/stores/ptyStore";
import { Activity, Loader2, Zap, Terminal, Settings, Moon, Sun } from "lucide-react";
import { SettingsPanel } from "./SettingsPanel";
import { TraceModal } from "./TraceModal";
import type { Message } from "@/types";
import { getSetting, setSetting } from "@/lib/appStore";

function ThemeToggleButton() {
  const [themeMode, setThemeMode] = useState<"dark" | "light">("dark");
  useEffect(() => {
    getSetting<string>("themeMode", "dark").then((m) => setThemeMode(m === "light" ? "light" : "dark"));
  }, []);
  const toggle = () => {
    const next = themeMode === "dark" ? "light" : "dark";
    setThemeMode(next);
    setSetting("themeMode", next);
    document.documentElement.classList.toggle("light", next === "light");
  };
  return (
    <button
      onClick={toggle}
      title={themeMode === "dark" ? "Switch to light mode" : "Switch to dark mode"}
      className="flex items-center px-2 h-full text-muted-foreground/50 hover:text-muted-foreground transition-colors"
    >
      {themeMode === "dark" ? <Moon className="w-3.5 h-3.5" /> : <Sun className="w-3.5 h-3.5" />}
    </button>
  );
}

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
  const [rateLimit, setRateLimit] = useState<{ fiveHourPct: number | null; weeklyPct: number | null; source: string; stale: boolean } | null>(null);
  const [traceOpen, setTraceOpen] = useState(false);
  const terminalOpen = usePtyStore((s) => s.terminalOpen);
  const toggleTerminal = usePtyStore((s) => s.toggleTerminal);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [settingsInitialSection, setSettingsInitialSection] = useState<string | undefined>(undefined);
  const hasPtySession = usePtyStore((s) => s.sessions.size > 0);

  // 외부 컴포넌트(역할 게이트, CommandPalette)가 Settings 를 여는 단일 이벤트.
  useEffect(() => {
    const handler = (e: Event) => {
      const detail = (e as CustomEvent<{ section?: string }>).detail;
      setSettingsInitialSection(detail?.section);
      setSettingsOpen(true);
    };
    window.addEventListener("tunaflow:open-settings", handler);
    return () => window.removeEventListener("tunaflow:open-settings", handler);
  }, []);

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
        // the agent:completed/error event was missed (e.g., timeout kill during
        // tab switch).
        //
        // Grace period: skip threads that started recently. The previous 10s
        // window caused FALSE POSITIVES whenever post-completion hooks
        // (vector indexing, memory compression) held the write lock for
        // longer than that — `prepare_engine_run` couldn't insert the
        // agent_jobs row in time, polling saw it as orphan, and the
        // mid-stream `list_messages` swap below clobbered the in-flight
        // streaming state. See task #82 + sample(1) finding on 2026-04-21.
        // 45s is comfortably above observed lock-hold durations while still
        // catching truly dead threads.
        const now = Date.now();
        const GRACE_MS = 45_000;
        const dbRunning = new Set(fetchedJobs.filter((j) => j.status === "running").map((j) => j.conversationId));
        const storeRunning = useChatStore.getState().runningThreadIds;
        // PTY sessions don't create agent_jobs — exclude them from orphan detection
        const ptyCapturing = usePtyStore.getState().isCapturing;
        const ptyActiveIds = new Set<string>();
        if (ptyCapturing) {
          for (const id of storeRunning) {
            if (!dbRunning.has(id)) ptyActiveIds.add(id);
          }
        }

        const orphans = storeRunning.filter((id) => {
          if (dbRunning.has(id)) return false;
          if (ptyActiveIds.has(id)) return false; // PTY active — not orphan
          const startTime = runStartTimes.get(id) ?? 0;
          return (now - startTime) > GRACE_MS;
        });
        if (orphans.length > 0) {
          console.warn("[orphan-recovery] Clearing stale runningThreadIds:", orphans);
          for (const id of orphans) {
            useChatStore.getState()._endRun(id, { silent: true });
            // `markConversationStale` nudges the next selectConversation()
            // to re-pull messages cleanly. The previous hot-swap via
            // setState({ messages }) could race with legitimate ongoing
            // streams whose agent_jobs row was simply delayed.
            useChatStore.getState().markConversationStale(id);
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

  // Rate limit — slow poll (60s), reads cached files only (no API calls)
  useEffect(() => {
    let cancelled = false;
    const poll = async () => {
      try {
        const info = await invoke<{ fiveHourPct: number | null; weeklyPct: number | null; source: string; stale: boolean } | null>("get_rate_limit_info");
        if (!cancelled) setRateLimit(info && !info.stale ? info : null);
      } catch { if (!cancelled) setRateLimit(null); }
    };
    poll();
    const timer = setInterval(poll, 60000);
    return () => { cancelled = true; clearInterval(timer); };
  }, []);

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
      <div className="flex items-center h-7 shrink-0 text-tf-xs text-prose-muted select-none">
        {/* Settings + theme toggle — far left of footer */}
        <button
          onClick={() => setSettingsOpen(true)}
          className="flex items-center px-2.5 h-full text-muted-foreground/50 hover:text-muted-foreground transition-colors"
          title="Settings"
        >
          <Settings className="w-3.5 h-3.5" />
        </button>
        <ThemeToggleButton />
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
                <span className={cn("font-mono text-tf-micro",
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
                <span className="text-tf-micro text-prose-disabled">
                  {gitStatus.added > 0 && `+${gitStatus.added}`}{gitStatus.added > 0 && gitStatus.modified > 0 && " "}{gitStatus.modified > 0 && `~${gitStatus.modified}`}{(gitStatus.added > 0 || gitStatus.modified > 0) && gitStatus.untracked > 0 && " "}{gitStatus.untracked > 0 && `?${gitStatus.untracked}`}
                </span>
              )}
            </span>
          </>
        )}

        {/* Rate limit */}
        {rateLimit && (
          <>
            <span className="w-px h-3 bg-border/30" />
            <span className="flex items-center gap-1.5 px-2 text-muted-foreground/40 text-tf-micro">
              {rateLimit.fiveHourPct != null && (
                <span className={cn(
                  "font-mono",
                  rateLimit.fiveHourPct >= 90 ? "text-red-400/70" :
                  rateLimit.fiveHourPct >= 70 ? "text-amber-400/70" : ""
                )}>
                  5h:{rateLimit.fiveHourPct.toFixed(0)}%
                </span>
              )}
              {rateLimit.weeklyPct != null && (
                <span className={cn(
                  "font-mono",
                  rateLimit.weeklyPct >= 90 ? "text-red-400/70" :
                  rateLimit.weeklyPct >= 70 ? "text-amber-400/70" : ""
                )}>
                  7d:{rateLimit.weeklyPct.toFixed(0)}%
                </span>
              )}
            </span>
          </>
        )}

        {/* Separator */}
        <span className="w-px h-3 bg-border/30" />

        {/* rawq status */}
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

        {/* PTY terminal toggle — right edge (항상 표시) */}
        <>
          <span className="w-px h-3 bg-border/30" />
          <button
            onClick={toggleTerminal}
            className={cn(
              "flex items-center gap-1 px-2 h-full transition-colors",
              terminalOpen ? "text-primary" : "text-muted-foreground/50 hover:text-muted-foreground"
            )}
            title="PTY Terminal"
          >
            <Terminal className="w-3 h-3" />
            {hasPtySession && (
              <span className={cn("w-1 h-1 rounded-full", terminalOpen ? "bg-primary" : "bg-muted-foreground/40")} />
            )}
          </button>
        </>
      </div>

      {traceOpen && <TraceModal onClose={() => setTraceOpen(false)} />}
      {settingsOpen && <SettingsPanel onClose={() => setSettingsOpen(false)} initialSection={settingsInitialSection} />}
    </>
  );
}
