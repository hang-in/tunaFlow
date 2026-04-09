import { useState, useEffect, useCallback } from "react";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import {
  Zap, Shield, Box, Gauge, Lock, Trash2,
  Play, ChevronDown, ChevronRight, CheckSquare, Square,
  AlertTriangle, Info, XCircle, CheckCircle2, Clock, Loader2,
  Download, ArrowUpRight,
} from "lucide-react";
import type { InsightSession, InsightFinding, InsightCategory, InsightSeverity } from "@/types";
import * as insightApi from "@/lib/api/insight";
import { runInsightAnalysis, autoFixQuickWins } from "@/lib/insightOrchestration";
import { toast } from "sonner";

// ── Constants ────────────────────────────────────────────────

const CATEGORY_META: Record<InsightCategory, { label: string; icon: React.ReactNode; color: string }> = {
  stability: { label: "안정성", icon: <Zap className="w-3 h-3" />, color: "text-yellow-500" },
  test: { label: "테스트", icon: <CheckSquare className="w-3 h-3" />, color: "text-blue-500" },
  architecture: { label: "아키텍처", icon: <Box className="w-3 h-3" />, color: "text-purple-500" },
  performance: { label: "성능", icon: <Gauge className="w-3 h-3" />, color: "text-orange-500" },
  security: { label: "보안", icon: <Lock className="w-3 h-3" />, color: "text-red-500" },
  debt: { label: "기술 부채", icon: <Trash2 className="w-3 h-3" />, color: "text-gray-500" },
};

const SEVERITY_META: Record<InsightSeverity, { icon: React.ReactNode; cls: string }> = {
  critical: { icon: <XCircle className="w-3 h-3" />, cls: "text-red-500 bg-red-500/10" },
  major: { icon: <AlertTriangle className="w-3 h-3" />, cls: "text-orange-500 bg-orange-500/10" },
  minor: { icon: <Info className="w-3 h-3" />, cls: "text-yellow-500 bg-yellow-500/10" },
  info: { icon: <Info className="w-3 h-3" />, cls: "text-blue-400 bg-blue-400/10" },
};

type QuadrantKey = "quick-wins" | "strategic" | "fill-ins" | "deprioritize";

function classifyQuadrant(f: InsightFinding): QuadrantKey {
  const isHighImpact = f.severity === "critical" || f.severity === "major";
  if (f.fixDifficulty === "auto") return isHighImpact ? "quick-wins" : "fill-ins";
  if (f.fixDifficulty === "guided") return isHighImpact ? "strategic" : "fill-ins";
  return "deprioritize"; // manual
}

// ── Summary bar ──────────────────────────────────────────────

function SummaryBar({ findings }: { findings: InsightFinding[] }) {
  const severityCounts = { critical: 0, major: 0, minor: 0, info: 0 };
  const difficultyCounts = { auto: 0, guided: 0, manual: 0 };
  let resolved = 0;

  for (const f of findings) {
    severityCounts[f.severity] = (severityCounts[f.severity] || 0) + 1;
    difficultyCounts[f.fixDifficulty] = (difficultyCounts[f.fixDifficulty] || 0) + 1;
    if (f.status === "resolved") resolved++;
  }

  return (
    <div className="rounded-md border border-border/30 bg-card/40 p-2 text-[10px] space-y-1">
      <div className="flex gap-3 flex-wrap">
        {Object.entries(severityCounts).map(([k, v]) => v > 0 && (
          <span key={k} className={cn("flex items-center gap-0.5", SEVERITY_META[k as InsightSeverity]?.cls)}>
            {SEVERITY_META[k as InsightSeverity]?.icon} {k}: {v}
          </span>
        ))}
      </div>
      <div className="flex gap-3 text-muted-foreground/60">
        <span>Auto: {difficultyCounts.auto}</span>
        <span>Guided: {difficultyCounts.guided}</span>
        <span>Manual: {difficultyCounts.manual}</span>
        <span className="text-status-approved/60">Resolved: {resolved}</span>
      </div>
    </div>
  );
}

// ── Finding card ─────────────────────────────────────────────

function FindingRow({
  finding,
  checked,
  active,
  onToggle,
  onSelect,
}: {
  finding: InsightFinding;
  checked: boolean;
  active: boolean;
  onToggle: (id: string) => void;
  onSelect: (f: InsightFinding) => void;
}) {
  const catMeta = CATEGORY_META[finding.category];
  const sevMeta = SEVERITY_META[finding.severity];

  return (
    <div
      onClick={() => onSelect(finding)}
      className={cn(
        "rounded-md border px-2 py-1.5 cursor-pointer transition-colors flex items-center gap-1.5",
        active ? "border-primary/40 bg-primary/5" :
        finding.status === "resolved" ? "border-status-approved/20 bg-status-approved/5 opacity-60" :
        finding.status === "dismissed" ? "border-border/10 opacity-40" :
        "border-border/30 bg-card/60 hover:border-border/50",
      )}
    >
      {(finding.status === "open" || finding.status === "selected") && (
        <button
          onClick={(e) => { e.stopPropagation(); onToggle(finding.id); }}
          className="shrink-0"
        >
          {checked ? <CheckSquare className="w-3 h-3 text-accent" /> : <Square className="w-3 h-3 text-muted-foreground/40" />}
        </button>
      )}
      {finding.status === "resolved" && <CheckCircle2 className="w-3 h-3 text-status-approved/60 shrink-0" />}
      {finding.status === "in_progress" && <Clock className="w-3 h-3 text-yellow-500/60 shrink-0" />}

      <span className={cn("shrink-0", catMeta?.color)}>{catMeta?.icon}</span>
      <span className={cn("text-[9px] px-1 py-0.5 rounded shrink-0", sevMeta?.cls)}>{finding.severity}</span>
      <span className="text-tf-sm font-medium text-foreground truncate flex-1">{finding.title}</span>
      {finding.filePath && (
        <span className="text-tf-micro text-prose-disabled font-mono truncate max-w-[120px] shrink-0">
          {finding.filePath.split("/").pop()}
        </span>
      )}
    </div>
  );
}

// ── Detail panel ─────────────────────────────────────────────

function FindingDetail({ finding, onPromoteToPlan }: { finding: InsightFinding; onPromoteToPlan?: (f: InsightFinding) => void }) {
  const catMeta = CATEGORY_META[finding.category];
  const sevMeta = SEVERITY_META[finding.severity];

  return (
    <div className="p-4 space-y-4 overflow-y-auto h-full">
      {/* Header */}
      <div>
        <div className="flex items-center gap-2 mb-2">
          <span className={cn("shrink-0", catMeta?.color)}>{catMeta?.icon}</span>
          <span className={cn("text-tf-xs px-1.5 py-0.5 rounded inline-flex items-center gap-0.5", sevMeta?.cls)}>
            {sevMeta?.icon} {finding.severity}
          </span>
          <span className="text-tf-xs text-prose-faint px-1.5 py-0.5 rounded bg-muted/40">
            {finding.fixDifficulty}
          </span>
          <span className="text-tf-xs text-prose-disabled">
            {finding.status}
          </span>
        </div>
        <h3 className="text-tf-base font-semibold text-foreground">{finding.title}</h3>
      </div>

      {/* File path */}
      {finding.filePath && (
        <div className="text-tf-sm text-prose-muted font-mono bg-muted/20 rounded px-2 py-1">
          {finding.filePath}{finding.lineNumber ? `:${finding.lineNumber}` : ""}
        </div>
      )}

      {/* Description */}
      <div className="text-tf-sm text-prose-base leading-relaxed whitespace-pre-wrap">
        {finding.description}
      </div>

      {/* Code snippet */}
      {finding.snippet && (
        <div>
          <p className="text-tf-xs text-prose-faint mb-1">코드</p>
          <pre className="text-tf-sm bg-[#1e1e1e] text-[#d4d4d4] rounded-md p-3 overflow-x-auto font-mono leading-relaxed">
            {finding.snippet}
          </pre>
        </div>
      )}

      {/* Resolution (resolved/done findings) */}
      {finding.resolution && (
        <div>
          <p className="text-tf-xs text-status-approved/70 mb-1 font-medium">해결 방법</p>
          <div className="text-tf-sm text-prose-base bg-status-approved/5 border border-status-approved/20 rounded-md p-3 whitespace-pre-wrap">
            {finding.resolution}
          </div>
        </div>
      )}

      {/* Meta */}
      {finding.estimatedFiles && finding.estimatedFiles > 1 && (
        <p className="text-tf-xs text-prose-disabled">
          예상 수정 파일: {finding.estimatedFiles}개
        </p>
      )}

      {/* Actions */}
      {(finding.status === "open" || finding.status === "selected") && onPromoteToPlan && (
        <button
          onClick={() => onPromoteToPlan(finding)}
          className="flex items-center gap-1 text-tf-xs px-2 py-1 rounded font-medium bg-primary/10 text-primary hover:bg-primary/20 transition-colors"
        >
          <ArrowUpRight className="w-3 h-3" />
          Plan으로 승격
        </button>
      )}
    </div>
  );
}

// ── Quadrant section ─────────────────────────────────────────

const QUADRANT_META: Record<QuadrantKey, { label: string; desc: string }> = {
  "quick-wins": { label: "Quick Wins", desc: "auto + high impact" },
  "strategic": { label: "Strategic", desc: "guided + high impact" },
  "fill-ins": { label: "Fill-ins", desc: "low impact" },
  "deprioritize": { label: "Deprioritize", desc: "manual" },
};

function QuadrantSection({
  quadrant,
  findings,
  selectedIds,
  activeFindingId,
  onToggle,
  onSelect,
  onAutoFix,
}: {
  quadrant: QuadrantKey;
  findings: InsightFinding[];
  selectedIds: Set<string>;
  activeFindingId: string | null;
  onToggle: (id: string) => void;
  onSelect: (f: InsightFinding) => void;
  onAutoFix?: () => void;
}) {
  const [collapsed, setCollapsed] = useState(quadrant === "deprioritize");
  const meta = QUADRANT_META[quadrant];
  const open = findings.filter((f) => f.status !== "resolved" && f.status !== "dismissed");

  if (findings.length === 0) return null;

  return (
    <div className="space-y-1">
      <div className="flex items-center">
        <button
          onClick={() => setCollapsed(!collapsed)}
          className="flex items-center gap-1 text-tf-xs font-medium text-prose-muted hover:text-foreground flex-1"
        >
          {collapsed ? <ChevronRight className="w-3 h-3" /> : <ChevronDown className="w-3 h-3" />}
          {meta.label}
          <span className="text-tf-micro text-prose-disabled">— {meta.desc} ({open.length})</span>
        </button>
        {quadrant === "quick-wins" && open.length > 0 && onAutoFix && (
          <button
            onClick={onAutoFix}
            className="text-tf-micro px-1.5 py-0.5 rounded bg-accent/20 text-accent hover:bg-accent/30 transition-colors"
          >
            Run All
          </button>
        )}
      </div>
      {!collapsed && (
        <div className="space-y-0.5">
          {findings.map((f) => (
            <FindingRow
              key={f.id}
              finding={f}
              checked={selectedIds.has(f.id)}
              active={f.id === activeFindingId}
              onToggle={onToggle}
              onSelect={onSelect}
            />
          ))}
        </div>
      )}
    </div>
  );
}

// ── Main panel ───────────────────────────────────────────────

export function InsightPanel() {
  const selectedProjectKey = useChatStore((s) => s.selectedProjectKey);
  const projects = useChatStore((s) => s.projects);

  const [sessions, setSessions] = useState<InsightSession[]>([]);
  const [activeSession, setActiveSession] = useState<InsightSession | null>(null);
  const [findings, setFindings] = useState<InsightFinding[]>([]);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [running, setRunning] = useState(false);
  const [progress, setProgress] = useState<string>("");
  const [categoryFilter, setCategoryFilter] = useState<InsightCategory | "all">("all");
  const [activeFinding, setActiveFinding] = useState<InsightFinding | null>(null);

  // Load sessions
  useEffect(() => {
    if (!selectedProjectKey) return;
    insightApi.listInsightSessions(selectedProjectKey).then((list) => {
      setSessions(list);
      if (list.length > 0) setActiveSession(list[0]);
    }).catch(console.error);
  }, [selectedProjectKey]);

  // Load findings when session changes
  useEffect(() => {
    if (!activeSession) { setFindings([]); return; }
    insightApi.listInsightFindings(activeSession.id).then(setFindings).catch(console.error);
  }, [activeSession?.id]);

  // Run analysis
  const handleRunAnalysis = useCallback(async () => {
    if (!selectedProjectKey || running) return;
    const project = projects.find((p) => p.key === selectedProjectKey);
    if (!project?.path) {
      toast.error("프로젝트 경로 없음");
      return;
    }

    setRunning(true);
    setProgress("시작...");
    try {
      const cats = categoryFilter !== "all" ? [categoryFilter] : undefined;
      const { session, findings: newFindings } = await runInsightAnalysis({
        projectKey: selectedProjectKey,
        projectPath: project.path,
        categories: cats,
        onProgress: setProgress,
      });
      setActiveSession(session);
      setFindings(newFindings);
      setSessions((prev) => [session, ...prev.filter((s) => s.id !== session.id)]);
      toast.success(`분석 완료: ${newFindings.length}건 발견`);
    } catch (err) {
      toast.error(`분석 실패: ${err}`);
    } finally {
      setRunning(false);
      setProgress("");
    }
  }, [selectedProjectKey, projects, categoryFilter, running]);

  // Toggle selection
  const handleToggle = useCallback((id: string) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

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
    if (!project?.path) { toast.error("프로젝트 경로 없음"); return; }
    try {
      const count = await insightApi.exportInsightToFiles(activeSession.id, project.path);
      toast.success(`${count}개 파일 저장 완료 (docs/insight/)`);
    } catch (err) {
      toast.error(`파일 저장 실패: ${err}`);
    }
  }, [activeSession, selectedProjectKey, projects]);

  // Promote findings to Architect plan
  const handlePromoteToPlan = useCallback(async (targetFindings: InsightFinding[]) => {
    if (targetFindings.length === 0) return;
    const lines = targetFindings.map((f) => {
      let entry = `### ${f.title}\n- **카테고리**: ${f.category}\n- **심각도**: ${f.severity}`;
      if (f.filePath) entry += `\n- **위치**: ${f.filePath}${f.lineNumber ? `:${f.lineNumber}` : ""}`;
      entry += `\n- **설명**: ${f.description}`;
      return entry;
    });
    const prompt = `## Insight 분석 결과 — Plan 요청\n\n다음 findings를 해결하는 plan을 제안해주세요.\n\n${lines.join("\n\n")}\n\n각 finding의 원본 파일을 확인하고 구체적인 subtask로 분해해주세요.`;

    // Mark findings as in_progress
    const ids = targetFindings.map((f) => f.id);
    await insightApi.updateInsightFindingsBatchStatus(ids, "in_progress");
    setFindings((prev) => prev.map((f) => ids.includes(f.id) ? { ...f, status: "in_progress" as const } : f));
    setSelectedIds(new Set());

    // Switch to Chat tab and send to Architect
    window.dispatchEvent(new CustomEvent("tunaflow:switch-tab", { detail: "chat" }));
    useChatStore.getState().sendWithEngine("claude", prompt);
    toast.success(`${targetFindings.length}건 Plan 승격 → Architect에게 전달`);
  }, []);

  // Auto fix quick wins
  const handleAutoFix = useCallback(async () => {
    if (running) return;
    const project = projects.find((p) => p.key === selectedProjectKey);
    if (!project?.path) return;

    setRunning(true);
    setProgress("Auto Fix 시작...");
    try {
      const { fixed, failed } = await autoFixQuickWins(
        findings,
        selectedProjectKey!,
        project.path,
        setProgress,
      );
      toast.success(`Auto Fix 완료: ${fixed}건 수정, ${failed}건 실패`);
      // Reload findings
      if (activeSession) {
        const updated = await insightApi.listInsightFindings(activeSession.id);
        setFindings(updated);
      }
    } catch (err) {
      toast.error(`Auto Fix 실패: ${err}`);
    } finally {
      setRunning(false);
      setProgress("");
    }
  }, [running, findings, selectedProjectKey, projects, activeSession]);

  // Filter findings
  const filtered = categoryFilter === "all"
    ? findings
    : findings.filter((f) => f.category === categoryFilter);

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

  if (!selectedProjectKey) {
    return <div className="p-4 text-center text-muted-foreground/50 text-xs">프로젝트를 선택하세요</div>;
  }

  return (
    <div className="flex flex-col h-full overflow-hidden">
      {/* Toolbar */}
      <div className="flex items-center gap-2 px-3 py-2 border-b border-border/20 shrink-0">
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
          {running ? "분석 중..." : "분석 실행"}
        </button>

        {activeSession && findings.length > 0 && (
          <button
            onClick={handleExportToFiles}
            className="flex items-center gap-0.5 text-[10px] px-1.5 py-0.5 rounded text-prose-muted hover:text-foreground hover:bg-muted/30 transition-colors"
            title="docs/insight/ 에 파일 저장"
          >
            <Download className="w-3 h-3" />
            파일 저장
          </button>
        )}
        <select
          value={categoryFilter}
          onChange={(e) => setCategoryFilter(e.target.value as InsightCategory | "all")}
          className="text-[10px] bg-transparent border border-border/30 rounded px-1.5 py-0.5 text-foreground"
        >
          <option value="all">전체 카테고리</option>
          {Object.entries(CATEGORY_META).map(([k, v]) => (
            <option key={k} value={k}>{v.label}</option>
          ))}
        </select>

        {activeSession && (
          <span className="text-[9px] text-muted-foreground/40 ml-auto">
            {new Date(activeSession.createdAt * 1000).toLocaleString()}
          </span>
        )}
      </div>

      {/* Progress */}
      {running && progress && (
        <div className="px-3 py-1.5 text-[10px] text-accent bg-accent/5 border-b border-border/10">
          {progress}
        </div>
      )}

      {/* Content — master-detail layout */}
      <div className="flex-1 flex min-h-0">
        {/* Left: findings list */}
        <div className={cn(
          "overflow-y-auto p-3 space-y-3 border-r border-border/20",
          activeFinding ? "w-[40%] shrink-0" : "flex-1",
        )}>
          {findings.length > 0 ? (
            <>
              <SummaryBar findings={filtered} />

              <QuadrantSection quadrant="quick-wins" findings={quadrants["quick-wins"]} selectedIds={selectedIds} activeFindingId={activeFinding?.id ?? null} onToggle={handleToggle} onSelect={setActiveFinding} onAutoFix={handleAutoFix} />
              <QuadrantSection quadrant="strategic" findings={quadrants["strategic"]} selectedIds={selectedIds} activeFindingId={activeFinding?.id ?? null} onToggle={handleToggle} onSelect={setActiveFinding} />
              <QuadrantSection quadrant="fill-ins" findings={quadrants["fill-ins"]} selectedIds={selectedIds} activeFindingId={activeFinding?.id ?? null} onToggle={handleToggle} onSelect={setActiveFinding} />
              <QuadrantSection quadrant="deprioritize" findings={quadrants["deprioritize"]} selectedIds={selectedIds} activeFindingId={activeFinding?.id ?? null} onToggle={handleToggle} onSelect={setActiveFinding} />

              {selectedIds.size > 0 && (
                <div className="flex items-center gap-2 pt-2 border-t border-border/20">
                  <span className="text-tf-xs text-prose-muted">{selectedIds.size}개 선택</span>
                  <button
                    onClick={() => {
                      const selected = findings.filter((f) => selectedIds.has(f.id));
                      handlePromoteToPlan(selected);
                    }}
                    className="text-tf-xs text-primary hover:text-primary/80 px-2 py-0.5 rounded border border-primary/30 flex items-center gap-0.5"
                  >
                    <ArrowUpRight className="w-2.5 h-2.5" />
                    Plan 승격
                  </button>
                  <button
                    onClick={handleDismiss}
                    className="text-tf-xs text-prose-faint hover:text-foreground px-2 py-0.5 rounded border border-border/30"
                  >
                    무시
                  </button>
                </div>
              )}
            </>
          ) : activeSession ? (
            <div className="text-center text-prose-faint text-tf-sm py-8">
              {activeSession.status === "completed" ? "발견 사항 없음" : activeSession.summary || "세션 로드 중..."}
            </div>
          ) : (
            <div className="text-center text-prose-faint text-tf-sm py-8">
              <p>아직 분석을 실행하지 않았습니다.</p>
              <p className="mt-1">"분석 실행" 버튼으로 프로젝트 품질을 분석하세요.</p>
            </div>
          )}

          {/* Session history */}
          {sessions.length > 1 && (
            <div className="pt-3 border-t border-border/20">
              <p className="text-tf-micro text-prose-disabled mb-1">이전 분석</p>
              {sessions.slice(1, 5).map((s) => (
                <button
                  key={s.id}
                  onClick={() => setActiveSession(s)}
                  className={cn(
                    "block w-full text-left text-tf-micro px-2 py-1 rounded hover:bg-muted/30 transition-colors",
                    s.id === activeSession?.id ? "bg-muted/40" : "",
                  )}
                >
                  <span className="text-prose-disabled">
                    {new Date(s.createdAt * 1000).toLocaleString()}
                  </span>
                  {s.summary && <span className="ml-2 text-prose-faint">{s.summary}</span>}
                </button>
              ))}
            </div>
          )}
        </div>

        {/* Right: detail panel */}
        {activeFinding && (
          <div className="flex-1 min-w-0 relative">
            <button
              onClick={() => setActiveFinding(null)}
              className="absolute top-2 right-2 z-10 text-prose-disabled hover:text-foreground p-1 rounded hover:bg-muted/30"
            >
              <XCircle className="w-4 h-4" />
            </button>
            <FindingDetail finding={activeFinding} onPromoteToPlan={(f) => handlePromoteToPlan([f])} />
          </div>
        )}
      </div>
    </div>
  );
}
