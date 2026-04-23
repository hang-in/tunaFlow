import { useState, useEffect, useCallback, useRef } from "react";
import { useTranslation } from "react-i18next";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import {
  Play, Loader2, Download, RefreshCw, GitBranch,
  AlertTriangle, Info, XCircle, ChevronRight, ChevronDown, Trash2,
  CheckSquare, Square,
} from "lucide-react";
import type { InsightSession, InsightFinding, InsightCategory, InsightSeverity } from "@/types";
import * as insightApi from "@/lib/api/insight";
import { runInsightAnalysis, revalidateFindings } from "@/lib/insightOrchestration";
import { formatError } from "@/lib/errors/userFriendlyMessage";
import { toast } from "sonner";
import { CATEGORY_META, SEVERITY_META, classifyQuadrant } from "./insight/insightConstants";
import type { QuadrantKey } from "./insight/insightConstants";
import { FindingDetail } from "./insight/InsightFindingCards";
import { QuadrantSection } from "./insight/InsightQuadrant";
import { IdentityView } from "./IdentityView";

type InsightTab = "findings" | "identity";

export function InsightPanel() {
  const [activeTab, setActiveTab] = useState<InsightTab>("findings");
  return (
    <div className="flex flex-col h-full overflow-hidden">
      <InsightTabsBar active={activeTab} onChange={setActiveTab} />
      {activeTab === "findings" ? <InsightFindingsTab /> : <IdentityView />}
    </div>
  );
}

function InsightTabsBar({
  active,
  onChange,
}: {
  active: InsightTab;
  onChange: (next: InsightTab) => void;
}) {
  const tabs: Array<{ id: InsightTab; label: string }> = [
    { id: "findings", label: "Findings" },
    { id: "identity", label: "Identity" },
  ];
  return (
    <div className="flex items-center gap-0 border-b border-border/20 shrink-0">
      {tabs.map((t) => (
        <button
          key={t.id}
          onClick={() => onChange(t.id)}
          className={cn(
            "px-3 py-1.5 text-[11px] font-medium transition-colors",
            active === t.id
              ? "text-foreground border-b-2 border-primary -mb-px"
              : "text-muted-foreground hover:text-foreground",
          )}
        >
          {t.label}
        </button>
      ))}
    </div>
  );
}

function InsightFindingsTab() {
  const { t } = useTranslation("insight");
  const selectedProjectKey = useChatStore((s) => s.selectedProjectKey);
  const projects = useChatStore((s) => s.projects);
  // Running-analysis state lives in the store so a tab switch (which
  // unmounts this component — see CenterPanel conditional render) does
  // not discard the live progress log while the background Tauri
  // command is still executing.
  const running = useChatStore((s) => s.insightRunning);
  const progressLines = useChatStore((s) => s.insightProgressLines);
  const persistedActiveSessionId = useChatStore((s) => s.insightActiveSessionId);

  const [sessions, setSessions] = useState<InsightSession[]>([]);
  const [activeSession, setActiveSession] = useState<InsightSession | null>(null);
  const [findings, setFindings] = useState<InsightFinding[]>([]);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [categoryFilter, setCategoryFilter] = useState<InsightCategory | "all">("all");
  const [activeFinding, setActiveFinding] = useState<InsightFinding | null>(null);
  // Previous session accordion
  const [expandedSessionIds, setExpandedSessionIds] = useState<Set<string>>(new Set());
  const [sessionFindings, setSessionFindings] = useState<Record<string, InsightFinding[]>>({});
  const progressEndRef = useRef<HTMLDivElement>(null);

  // Load sessions. When the panel remounts (user returns to the tab
  // mid-run), prefer the session id the store is tracking so the
  // findings list stays anchored to the latest run instead of snapping
  // back to list[0].
  useEffect(() => {
    if (!selectedProjectKey) return;
    insightApi.listInsightSessions(selectedProjectKey).then((list) => {
      setSessions(list);
      const preferred = persistedActiveSessionId
        ? list.find((s) => s.id === persistedActiveSessionId)
        : null;
      if (preferred) setActiveSession(preferred);
      else if (list.length > 0) setActiveSession(list[0]);
    }).catch(console.error);
  }, [selectedProjectKey, persistedActiveSessionId]);

  // Load findings when session changes
  useEffect(() => {
    if (!activeSession) { setFindings([]); return; }
    insightApi.listInsightFindings(activeSession.id).then(setFindings).catch(console.error);
  }, [activeSession?.id]);

  // Auto-scroll progress
  useEffect(() => {
    progressEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [progressLines]);

  // Run analysis. Progress / running state routes through the store so
  // that the background Tauri command keeps pushing updates even after
  // the user switches tabs and the panel unmounts.
  const handleRunAnalysis = useCallback(async () => {
    if (!selectedProjectKey || running) return;
    const project = projects.find((p) => p.key === selectedProjectKey);
    if (!project?.path) {
      toast.error(t("toast.no_project_path"));
      return;
    }

    const store = useChatStore.getState();
    store.insightStartRun();
    setActiveFinding(null);
    try {
      const cats = categoryFilter !== "all" ? [categoryFilter] : undefined;
      const { session, findings: newFindings } = await runInsightAnalysis({
        projectKey: selectedProjectKey,
        projectPath: project.path,
        categories: cats,
        onProgress: (msg) => useChatStore.getState().insightAppendProgress(msg),
      });
      setActiveSession(session);
      setFindings(newFindings);
      setSessions((prev) => [session, ...prev.filter((s) => s.id !== session.id)]);
      useChatStore.getState().insightAppendProgress(t("toast.progress_completed", { count: newFindings.length }));
      useChatStore.getState().insightFinishRun(session.id);
      toast.success(t("toast.analysis_success", { count: newFindings.length }));
    } catch (err) {
      useChatStore.getState().insightFailRun(String(err));
      toast.error(t("toast.analysis_error", { error: formatError(err) }));
    }
  }, [selectedProjectKey, projects, categoryFilter, running, t]);

  // Toggle selection
  const handleToggle = useCallback((id: string) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  // Select all open findings
  const handleSelectAll = useCallback((openFindings: InsightFinding[]) => {
    const openIds = openFindings.map((f) => f.id);
    setSelectedIds((prev) => {
      const allSelected = openIds.every((id) => prev.has(id));
      const next = new Set(prev);
      if (allSelected) openIds.forEach((id) => next.delete(id));
      else openIds.forEach((id) => next.add(id));
      return next;
    });
  }, []);

  // Delete previous session
  const handleDeleteSession = useCallback(async (sessionId: string) => {
    try {
      await insightApi.deleteInsightSession(sessionId);
      setSessions((prev) => prev.filter((s) => s.id !== sessionId));
      setSessionFindings((prev) => { const next = { ...prev }; delete next[sessionId]; return next; });
      setExpandedSessionIds((prev) => { const next = new Set(prev); next.delete(sessionId); return next; });
      toast.success(t("toast.delete_success"));
    } catch (err) {
      toast.error(t("toast.delete_error", { error: formatError(err) }));
    }
  }, [t]);

  // Toggle accordion for previous session
  const handleToggleSession = useCallback(async (sessionId: string) => {
    setExpandedSessionIds((prev) => {
      const next = new Set(prev);
      if (next.has(sessionId)) { next.delete(sessionId); return next; }
      next.add(sessionId);
      return next;
    });
    if (!sessionFindings[sessionId]) {
      insightApi.listInsightFindings(sessionId)
        .then((list) => setSessionFindings((prev) => ({ ...prev, [sessionId]: list })))
        .catch(console.error);
    }
  }, [sessionFindings]);

  // Dismiss selected
  const handleDismiss = useCallback(async () => {
    if (selectedIds.size === 0) return;
    const ids = Array.from(selectedIds);
    await insightApi.updateInsightFindingsBatchStatus(ids, "dismissed");
    setFindings((prev) => prev.map((f) => ids.includes(f.id) ? { ...f, status: "dismissed" as const } : f));
    setSelectedIds(new Set());
  }, [selectedIds]);

  // Export findings to project files
  const handleExportToFiles = useCallback(async () => {
    if (!activeSession || !selectedProjectKey) return;
    const project = projects.find((p) => p.key === selectedProjectKey);
    if (!project?.path) { toast.error(t("toast.no_project_path")); return; }
    try {
      const count = await insightApi.exportInsightToFiles(activeSession.id, project.path);
      toast.success(t("toast.save_success", { count }));
    } catch (err) {
      toast.error(t("toast.save_error", { error: formatError(err) }));
    }
  }, [activeSession, selectedProjectKey, projects, t]);

  // Send findings to Architect via a new Review Branch (B안)
  const handleSendToArchitect = useCallback(async (targetFindings: InsightFinding[]) => {
    if (targetFindings.length === 0) return;
    const store = useChatStore.getState();
    const convId = store.selectedConversationId;
    if (!convId) { toast.error(t("toast.select_conversation_first")); return; }

    const lines = targetFindings.map((f) => {
      const location = f.filePath
        ? t("review_prompt.entry_location", {
            path: f.filePath,
            line: f.lineNumber ? `:${f.lineNumber}` : "",
          })
        : "";
      let entry = t("review_prompt.entry", {
        title: f.title,
        category: f.category,
        severity: f.severity,
        difficulty: f.fixDifficulty,
        location,
        description: f.description,
      });
      if (f.snippet) entry += `\n\`\`\`\n${f.snippet.slice(0, 300)}\n\`\`\``;
      return entry;
    });

    const prompt = t("review_prompt.body", {
      count: targetFindings.length,
      findings: lines.join("\n\n"),
    });

    try {
      // Create Architect Review Branch
      await store.createBranch(convId, undefined, t("review_prompt.branch_label", { count: targetFindings.length }), "chat");
      // Branch is now at top of list — find it
      const newBranch = useChatStore.getState().branches
        .filter((b) => b.conversationId === convId && b.mode !== "roundtable")
        .sort((a, b) => b.createdAt - a.createdAt)[0];

      if (!newBranch) { toast.error(t("toast.branch_creation_failed")); return; }

      // Handoff to Architect = findings done from Insight's perspective. Mark
      // them resolved immediately so the user's "needs action" queue stays
      // clean. The linked branch preserves the trail — user can revisit what
      // happened via that branch. Previously these went to `in_progress` with
      // no actionable UI, polluting the list indefinitely until the linked
      // branch happened to archive.
      const ids = targetFindings.map((f) => f.id);
      const resolution = `Sent to Architect via branch ${newBranch.id.slice(0, 8)}`;
      await Promise.all(
        ids.map((id) => insightApi.updateInsightFindingStatus(id, "resolved", resolution)),
      );
      insightApi.linkInsightFindingsToBranch(ids, newBranch.id)
        .catch((e) => console.debug("[insight] link branch failed:", e));
      setFindings((prev) => prev.map((f) => ids.includes(f.id) ? { ...f, status: "resolved" as const } : f));
      setSelectedIds(new Set());

      // Open branch drawer and send message
      store.openThread(newBranch.id);
      setTimeout(() => {
        store.sendThreadMessage(prompt);
      }, 300);

      toast.success(t("toast.branch_send_success", { count: targetFindings.length }));
    } catch (err) {
      toast.error(t("toast.branch_creation_error", { error: formatError(err) }));
    }
  }, [t]);

  // Revalidate open findings against current codebase. Routes the same
  // store-backed progress path as handleRunAnalysis so revalidation
  // logs also survive tab unmounts.
  const handleRevalidate = useCallback(async () => {
    if (running || !selectedProjectKey) return;
    const openCount = findings.filter((f) => f.status === "open").length;
    if (openCount === 0) { toast.info(t("toast.rerun_no_open")); return; }

    const store = useChatStore.getState();
    store.insightStartRun();
    store.insightAppendProgress(t("toast.rerun_progress", { count: openCount }));
    try {
      const results = await revalidateFindings(
        findings,
        selectedProjectKey,
        (msg) => useChatStore.getState().insightAppendProgress(msg),
      );
      const resolved = results.filter((r) => r.status === "resolved");
      const uncertain = results.filter((r) => r.status === "uncertain");

      // Update resolved findings in DB and local state
      for (const r of resolved) {
        await insightApi.updateInsightFindingStatus(r.id, "resolved", r.reason);
      }
      if (resolved.length > 0) {
        setFindings((prev) => prev.map((f) => {
          const match = resolved.find((r) => r.id === f.id);
          return match ? { ...f, status: "resolved" as const } : f;
        }));
      }

      const msg = resolved.length > 0
        ? t("toast.rerun_done_resolved", {
            count: resolved.length,
            suffix: uncertain.length > 0 ? t("toast.rerun_uncertain_suffix", { count: uncertain.length }) : "",
          })
        : t("toast.rerun_done_all_valid");
      useChatStore.getState().insightFinishRun();
      toast.success(msg);
    } catch (err) {
      useChatStore.getState().insightFailRun(String(err));
      toast.error(t("toast.rerun_error", { error: formatError(err) }));
    }
  }, [running, findings, selectedProjectKey, t]);

  // Auto fix — disabled, pending meta-agent (see docs/ideas/onboardingMetaAgentIdea.md §8)
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  const _handleAutoFix = useCallback(() => {}, []);

  // Filter findings
  const filtered = categoryFilter === "all"
    ? findings
    : findings.filter((f) => f.category === categoryFilter);

  const openFindings = filtered.filter((f) => f.status === "open" || f.status === "selected");
  const allSelected = openFindings.length > 0 && openFindings.every((f) => selectedIds.has(f.id));

  // Group by quadrant
  const quadrants: Record<QuadrantKey, InsightFinding[]> = {
    "quick-wins": [],
    "strategic": [],
    "fill-ins": [],
    "deprioritize": [],
  };
  for (const f of filtered) {
    quadrants[classifyQuadrant(f)].push(f);
  }

  // Right panel mode
  const rightPanel: "progress" | "finding" | "none" =
    running || progressLines.length > 0
      ? (activeFinding ? "finding" : "progress")
      : activeFinding ? "finding" : "none";

  if (!selectedProjectKey) {
    return <div className="p-4 text-center text-muted-foreground/50 text-xs">{t("panel.empty_project")}</div>;
  }

  return (
    <div className="flex flex-col flex-1 min-h-0 overflow-hidden">
      {/* Toolbar */}
      <div className="flex items-center gap-1.5 px-3 py-2 border-b border-border/20 shrink-0">
        <button
          onClick={handleRunAnalysis}
          disabled={running}
          className={cn(
            "flex items-center gap-1 text-[10px] px-2 py-1 rounded font-medium transition-colors",
            running
              ? "bg-muted text-muted-foreground cursor-not-allowed"
              : "bg-accent text-accent-foreground hover:bg-accent/80",
          )}
        >
          {running ? <Loader2 className="w-3 h-3 animate-spin" /> : <Play className="w-3 h-3" />}
          {running ? t("action.run_analysis_busy") : t("action.run_analysis")}
        </button>

        {activeSession && findings.length > 0 && (
          <button
            onClick={handleRevalidate}
            disabled={running}
            className="flex items-center gap-0.5 text-[10px] px-1.5 py-0.5 rounded text-prose-muted hover:text-foreground hover:bg-muted/30 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
            title={t("action.rerun_tooltip")}
          >
            <RefreshCw className="w-3 h-3" />
            {t("action.rerun_button")}
          </button>
        )}

        {activeSession && findings.length > 0 && (
          <button
            onClick={handleExportToFiles}
            className="flex items-center gap-0.5 text-[10px] px-1.5 py-0.5 rounded text-prose-muted hover:text-foreground hover:bg-muted/30 transition-colors"
            title={t("action.save_tooltip")}
          >
            <Download className="w-3 h-3" />
            {t("action.save_button")}
          </button>
        )}

        <span className="w-px h-3 bg-border/30 mx-0.5" />

        <select
          value={categoryFilter}
          onChange={(e) => setCategoryFilter(e.target.value as InsightCategory | "all")}
          className="text-[10px] bg-transparent border border-border/30 rounded px-1.5 py-0.5 text-foreground"
        >
          <option value="all">{t("panel.all_categories")}</option>
          {Object.keys(CATEGORY_META).map((k) => (
            <option key={k} value={k}>{t(`category.${k}` as "category.stability")}</option>
          ))}
        </select>

        {activeSession && (
          <span className="text-[9px] text-muted-foreground/35 ml-auto">
            {new Date(activeSession.createdAt * 1000).toLocaleString()}
          </span>
        )}
      </div>

      {/* Summary strip — always visible when findings exist */}
      {findings.length > 0 && (() => {
        const open = findings.filter((f) => f.status === "open").length;
        const resolved = findings.filter((f) => f.status === "resolved").length;
        const inProgress = findings.filter((f) => f.status === "in_progress").length;
        const total = findings.length;
        const bySeverity = { critical: 0, major: 0, minor: 0, info: 0 };
        for (const f of findings.filter((f) => f.status === "open")) {
          bySeverity[f.severity] = (bySeverity[f.severity] || 0) + 1;
        }
        return (
          <div className="flex items-center gap-3 px-3 py-1.5 border-b border-border/10 shrink-0 text-[9px] bg-card/20">
            {bySeverity.critical > 0 && (
              <span className="flex items-center gap-0.5 text-red-500/80">
                <XCircle className="w-2.5 h-2.5" />{bySeverity.critical}
              </span>
            )}
            {bySeverity.major > 0 && (
              <span className="flex items-center gap-0.5 text-orange-500/80">
                <AlertTriangle className="w-2.5 h-2.5" />{bySeverity.major}
              </span>
            )}
            {bySeverity.minor > 0 && (
              <span className="flex items-center gap-0.5 text-yellow-500/70">
                <Info className="w-2.5 h-2.5" />{bySeverity.minor}
              </span>
            )}
            {bySeverity.info > 0 && (
              <span className="flex items-center gap-0.5 text-blue-400/70">
                <Info className="w-2.5 h-2.5" />{bySeverity.info}
              </span>
            )}
            <span className="w-px h-2.5 bg-border/30" />
            <span className="text-prose-disabled">
              {open > 0 && <span className="text-foreground/50">{open} open</span>}
              {inProgress > 0 && <span className="ml-1.5 text-primary/50">{t("panel.progress_in_progress", { count: inProgress })}</span>}
              {resolved > 0 && <span className="ml-1.5 text-status-approved/60">{t("panel.progress_resolved", { resolved, total })}</span>}
            </span>
            {resolved > 0 && (
              <div className="ml-auto w-20 h-1 bg-muted/40 rounded-full overflow-hidden">
                <div
                  className="h-full bg-status-approved/50 rounded-full transition-all"
                  style={{ width: `${Math.round((resolved / total) * 100)}%` }}
                />
              </div>
            )}
          </div>
        );
      })()}

      {/* Content — master-detail layout */}
      <div className="flex-1 flex min-h-0">
        {/* Left: findings list */}
        <div className={cn(
          "overflow-y-auto p-3 space-y-3 border-r border-border/20",
          rightPanel !== "none" ? "w-[42%] shrink-0" : "flex-1",
        )}>
          {findings.length > 0 ? (
            <>
              {/* Select all bar */}
              {openFindings.length > 0 && (
                <div className="flex items-center gap-2">
                  <button
                    onClick={() => handleSelectAll(openFindings)}
                    className="flex items-center gap-1 text-[10px] text-prose-muted hover:text-foreground transition-colors"
                  >
                    {allSelected
                      ? <CheckSquare className="w-3 h-3 text-accent" />
                      : <Square className="w-3 h-3 text-muted-foreground/40" />}
                    {t("action.select_all")}
                  </button>
                  {selectedIds.size > 0 && (
                    <>
                      <span className="text-[10px] text-prose-disabled">{t("action.selected_count", { count: selectedIds.size })}</span>
                      <button
                        onClick={() => {
                          const selected = findings.filter((f) => selectedIds.has(f.id));
                          handleSendToArchitect(selected);
                        }}
                        className="flex items-center gap-0.5 text-[10px] text-primary hover:text-primary/80 px-1.5 py-0.5 rounded border border-primary/30"
                      >
                        <GitBranch className="w-2.5 h-2.5" />
                        {t("action.architect_review_button")}
                      </button>
                      <button
                        onClick={handleDismiss}
                        className="text-[10px] text-prose-faint hover:text-foreground px-1.5 py-0.5 rounded border border-border/30"
                      >
                        {t("action.dismiss_button")}
                      </button>
                    </>
                  )}
                </div>
              )}

              <QuadrantSection quadrant="quick-wins" findings={quadrants["quick-wins"]} selectedIds={selectedIds} activeFindingId={activeFinding?.id ?? null} onToggle={handleToggle} onSelect={setActiveFinding} />
              <QuadrantSection quadrant="strategic" findings={quadrants["strategic"]} selectedIds={selectedIds} activeFindingId={activeFinding?.id ?? null} onToggle={handleToggle} onSelect={setActiveFinding} />
              <QuadrantSection quadrant="fill-ins" findings={quadrants["fill-ins"]} selectedIds={selectedIds} activeFindingId={activeFinding?.id ?? null} onToggle={handleToggle} onSelect={setActiveFinding} />
              <QuadrantSection quadrant="deprioritize" findings={quadrants["deprioritize"]} selectedIds={selectedIds} activeFindingId={activeFinding?.id ?? null} onToggle={handleToggle} onSelect={setActiveFinding} />
            </>
          ) : activeSession ? (
            <div className="text-center text-prose-faint text-tf-sm py-8">
              {activeSession.status === "completed" ? t("panel.no_findings") : activeSession.summary || t("panel.session_loading")}
            </div>
          ) : (
            <div className="text-center text-prose-faint text-tf-sm py-8">
              <p>{t("panel.no_analysis_yet")}</p>
              <p className="mt-1">{t("panel.no_analysis_hint")}</p>
            </div>
          )}

          {/* Previous sessions — accordion */}
          {sessions.length > 1 && (
            <div className="pt-3 border-t border-border/20 space-y-1">
              <p className="text-[10px] font-semibold text-prose-disabled uppercase tracking-wider mb-2">{t("panel.previous_analysis")}</p>
              {sessions.slice(1).map((s) => {
                const expanded = expandedSessionIds.has(s.id);
                const sFindings = sessionFindings[s.id];
                return (
                  <div key={s.id} className="rounded-md border border-border/20 bg-card/20 overflow-hidden">
                    {/* Session header */}
                    <div className="flex items-center gap-1 px-2 py-1.5 hover:bg-muted/20 transition-colors">
                      <button
                        onClick={() => handleToggleSession(s.id)}
                        className="flex items-center gap-1 flex-1 min-w-0 text-left"
                      >
                        {expanded
                          ? <ChevronDown className="w-3 h-3 text-prose-disabled shrink-0" />
                          : <ChevronRight className="w-3 h-3 text-prose-disabled shrink-0" />}
                        <span className="text-[10px] text-prose-muted truncate">
                          {new Date(s.createdAt * 1000).toLocaleDateString()} {new Date(s.createdAt * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}
                        </span>
                        {s.summary && (
                          <span className="text-[9px] text-prose-disabled truncate ml-1">{s.summary}</span>
                        )}
                      </button>
                      <button
                        onClick={() => handleDeleteSession(s.id)}
                        className="shrink-0 p-0.5 rounded text-prose-disabled hover:text-destructive hover:bg-destructive/10 transition-colors"
                        title={t("action.delete_tooltip")}
                      >
                        <Trash2 className="w-3 h-3" />
                      </button>
                    </div>
                    {/* Expanded finding list */}
                    {expanded && (
                      <div className="border-t border-border/10 px-2 py-1.5 space-y-1">
                        {sFindings === undefined ? (
                          <div className="flex items-center gap-1 text-[9px] text-prose-disabled py-1">
                            <Loader2 className="w-2.5 h-2.5 animate-spin" /> {t("panel.loading")}
                          </div>
                        ) : sFindings.length === 0 ? (
                          <p className="text-[9px] text-prose-faint py-1">{t("panel.no_findings")}</p>
                        ) : (
                          sFindings.map((f) => (
                            <button
                              key={f.id}
                              onClick={() => setActiveFinding(f)}
                              className={cn(
                                "w-full text-left flex items-center gap-1.5 px-1.5 py-1 rounded text-[9px] hover:bg-muted/30 transition-colors",
                                activeFinding?.id === f.id ? "bg-primary/10 text-foreground/80" : "text-prose-muted",
                              )}
                            >
                              <span className={cn("shrink-0", CATEGORY_META[f.category]?.color)}>
                                {CATEGORY_META[f.category]?.icon}
                              </span>
                              <span className={cn("text-[8px] px-0.5 py-px rounded shrink-0", SEVERITY_META[f.severity]?.cls)}>
                                {f.severity}
                              </span>
                              <span className="truncate">{f.title}</span>
                            </button>
                          ))
                        )}
                      </div>
                    )}
                  </div>
                );
              })}
            </div>
          )}
        </div>

        {/* Right: progress log OR finding detail */}
        {rightPanel !== "none" && (
          <div className="flex-1 min-w-0 relative flex flex-col">
            <button
              onClick={() => setActiveFinding(null)}
              className="absolute top-2 right-2 z-10 text-prose-disabled hover:text-foreground p-1 rounded hover:bg-muted/30"
            >
              <XCircle className="w-3.5 h-3.5" />
            </button>
            {rightPanel === "finding" && activeFinding ? (
              <FindingDetail finding={activeFinding} onSendToArchitect={(f) => handleSendToArchitect([f])} />
            ) : (
              /* Progress log — streaming style */
              <div className="flex-1 overflow-y-auto p-3">
                <p className="text-[9px] font-semibold uppercase tracking-wider text-prose-disabled mb-2">
                  {running ? t("panel.running_log") : t("panel.last_log")}
                </p>
                <div className="space-y-0.5 font-mono">
                  {progressLines.map((line, i) => (
                    <div key={i} className={cn(
                      "text-[10px] leading-relaxed",
                      line.startsWith("✓") ? "text-status-approved/80" :
                      line.startsWith("✗") ? "text-destructive/80" :
                      line.includes(t("log_hint.failure")) || line.includes(t("log_hint.empty")) ? "text-prose-disabled" :
                      "text-prose-muted",
                    )}>
                      {line}
                    </div>
                  ))}
                  {running && (
                    <div className="flex items-center gap-1 text-[10px] text-primary/60 animate-pulse mt-1">
                      <Loader2 className="w-2.5 h-2.5 animate-spin" /> {t("panel.processing")}
                    </div>
                  )}
                  <div ref={progressEndRef} />
                </div>
              </div>
            )}
          </div>
        )}

      </div>
    </div>
  );
}
