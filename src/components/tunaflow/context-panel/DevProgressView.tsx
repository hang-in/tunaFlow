import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { GitBranch, Check, Loader2, Clock, RotateCcw, Plus, ClipboardList, FileText } from "lucide-react";
import type { Plan, PlanPhase, PlanSubtask, Message } from "@/types";
import * as planApi from "@/lib/api/plans";
import { scanCompletedSubtasks, hasImplComplete, hasReviewVerdict, extractReviewVerdict } from "@/lib/planProposalParser";
import { runProjectTests, type TestRunResult } from "@/lib/api/testRunner";
import type { ParsedReviewVerdict } from "@/lib/planProposalParser";
import { syncResultReport } from "@/lib/workflowOrchestration";
import type { Branch } from "@/types";
import { PlanDocumentModal } from "./PlanDocumentModal";

interface DevProgressViewProps {
  plan: Plan;
  onPlanUpdate: (planId: string, update: Partial<Plan>) => void;
}

export function DevProgressView({ plan, onPlanUpdate }: DevProgressViewProps) {
  const { openThread, sendThreadMessage, loadBranches, saveConversationEngine } = useChatStore();
  const profiles = useChatStore((s) => s.agentProfiles);
  const [subtasks, setSubtasks] = useState<PlanSubtask[]>([]);
  const [completedNums, setCompletedNums] = useState<Set<number>>(new Set());
  const [implComplete, setImplComplete] = useState(false);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [showDoc, setShowDoc] = useState(false);
  const [testResult, setTestResult] = useState<TestRunResult | null>(null);
  const [testRunning, setTestRunning] = useState(false);
  const [reviewVerdict, setReviewVerdict] = useState<ParsedReviewVerdict | null>(null);
  const [reviewMode, setReviewMode] = useState<"idle" | "select">("idle");
  const [selectedReviewerId, setSelectedReviewerId] = useState(() => {
    // Default to a profile with "review" in label, or first profile
    const reviewer = profiles.find((p) => p.label.toLowerCase().includes("review"));
    return reviewer?.id ?? profiles[0]?.id ?? "";
  });

  useEffect(() => {
    let cancelled = false;
    setLoading(true);

    (async () => {
      const sts = await planApi.listSubtasks(plan.id).catch(() => [] as PlanSubtask[]);
      if (cancelled) return;
      setSubtasks(sts);

      // Scan branch messages for subtask-done + impl-complete markers
      if (plan.implementationBranchId) {
        try {
          const shadowConvId = `branch:${plan.implementationBranchId}`;
          const msgs = await invoke<Message[]>("list_messages", { conversationId: shadowConvId });
          if (!cancelled) {
            setCompletedNums(scanCompletedSubtasks(msgs));
            const complete = msgs.some((m) => m.role === "assistant" && hasImplComplete(m.content));
            setImplComplete(complete);

            // Auto-run tests when implementation is complete
            if (complete && !cancelled) {
              try {
                const projectKey = useChatStore.getState().selectedProjectKey;
                if (projectKey) {
                  const project = await invoke("get_project", { key: projectKey }) as { path?: string };
                  if (project?.path) {
                    setTestRunning(true);
                    const result = await runProjectTests(project.path);
                    if (!cancelled) {
                      setTestResult(result);
                      setTestRunning(false);
                    }
                  }
                }
              } catch (e) {
                console.warn("[tunaflow] test run failed:", e);
                setTestRunning(false);
              }
            }
          }
        } catch { /* branch may not exist */ }
      }
      // Scan review branch for verdict (rework phase)
      if (plan.reviewBranchId && (plan.phase === "rework" || plan.phase === "review")) {
        try {
          const reviewShadow = `branch:${plan.reviewBranchId}`;
          const reviewMsgs = await invoke<Message[]>("list_messages", { conversationId: reviewShadow });
          for (const msg of reviewMsgs) {
            if (msg.role === "assistant" && hasReviewVerdict(msg.content)) {
              const v = extractReviewVerdict(msg.content);
              if (v && !cancelled) setReviewVerdict(v);
              break;
            }
          }
        } catch (e) { console.warn("[tunaflow]", e); }
      }

      setLoading(false);
    })();

    return () => { cancelled = true; };
  }, [plan.id, plan.implementationBranchId, plan.phase]);

  const handleOpenBranch = () => {
    if (plan.implementationBranchId) openThread(plan.implementationBranchId);
  };

  const handleRerunSubtask = async (subtask: PlanSubtask, index: number) => {
    if (!plan.implementationBranchId) return;
    setBusy(true);
    try {
      await openThread(plan.implementationBranchId);
      const prompt = `Subtask ${index + 1} "${subtask.title}"을(를) 다시 구현해주세요.\n\n상세 설계:\n${subtask.details ?? "(없음)"}\n\n완료 후 \`<!-- tunaflow:subtask-done:${index + 1} -->\`을 포함하세요.`;
      const shadowConvId = `branch:${plan.implementationBranchId}`;
      const saved = useChatStore.getState().getConversationEngine(shadowConvId);
      await sendThreadMessage(prompt, saved?.engine ?? "claude");
    } catch (e) { console.warn("[tunaflow]", e); }
    setBusy(false);
  };

  const handleStartReview = async () => {
    if (!plan.implementationBranchId) return;
    const selectedProfile = profiles.find((p) => p.id === selectedReviewerId);
    if (!selectedProfile) return;
    setBusy(true);
    try {
      // Generate result report
      const implShadow = `branch:${plan.implementationBranchId}`;
      const msgs = await invoke<Message[]>("list_messages", { conversationId: implShadow });
      await syncResultReport(plan.id, msgs, plan.developerEngine ?? undefined);

      // Archive previous review branch if exists
      if (plan.reviewBranchId) {
        await invoke("archive_branch", { id: plan.reviewBranchId }).catch(() => {});
      }

      // Phase transition
      const isRework = plan.phase === "rework" || !!reviewVerdict;
      const roundLabel = isRework ? `Re-review` : `Review`;
      await planApi.updatePlanPhase(plan.id, "review");
      await planApi.createPlanEvent(plan.id, "review_started", "user",
        `reviewer=${selectedProfile.label}${isRework ? " (rework)" : ""}`);

      // Create new review branch
      const slug = plan.title.replace(/[^\w가-힣-]/g, "-").replace(/-+/g, "-").replace(/^-|-$/g, "").toLowerCase().slice(0, 80);
      const input = { conversationId: plan.conversationId, label: `${roundLabel}: ${plan.title.slice(0, 25)}`, mode: "chat" };
      const branch = await invoke<Branch>("create_branch", { input });
      const shadowConvId = await invoke<string>("open_branch_stream", { branchId: branch.id });
      await planApi.linkPlanBranch(plan.id, "review", branch.id);
      saveConversationEngine(shadowConvId, { profileId: selectedReviewerId, engine: selectedProfile.engine });

      await loadBranches(plan.conversationId);
      await openThread(branch.id);

      // Build structured review prompt
      const prevFindingsBlock = reviewVerdict && reviewVerdict.findings.length > 0
        ? [
            `│`,
            `│ 이전 Review Findings (수정 확인 필요):`,
            ...reviewVerdict.findings.map((f, i) => `│ □ ${i + 1}. ${f.slice(0, 150)}`),
            `│`,
            `│ 위 사항이 수정되었는지 반드시 확인하세요.`,
          ]
        : [];

      const prompt = [
        `┌─ ${roundLabel} 요청 ────────────────────────┐`,
        `│`,
        `│ Plan: "${plan.title}"`,
        `│`,
        `│ 검증 문서:`,
        `│ • Plan: docs/plans/${slug}.md`,
        `│ • 결과: docs/plans/${slug}-result.md`,
        `│ • 지시서: docs/plans/${slug}-task-*.md`,
        ...prevFindingsBlock,
        `│`,
        `│ Plan 문서와 작업 지시서를 기준으로`,
        `│ 구현 결과를 검증하세요.`,
        `│`,
        `│ 완료 조건:`,
        `│ <!-- tunaflow:review-verdict --> 제출`,
        `└──────────────────────────────────────────────┘`,
      ].join("\n");

      await sendThreadMessage(prompt, selectedProfile.engine);
      onPlanUpdate(plan.id, { phase: "review" as PlanPhase, reviewBranchId: branch.id });
    } catch (e) { console.warn("[tunaflow]", e); }
    setBusy(false);
    setReviewMode("idle");
  };

  const handleCreateSubPlan = async (subtask: PlanSubtask, index: number) => {
    // Send to main chat Architect — request a sub-plan for this subtask
    const { sendWithEngine } = useChatStore.getState();
    setBusy(true);
    try {
      const prompt = [
        `[Sub-plan 요청] Plan "${plan.title}" → Subtask ${index + 1} "${subtask.title}"`,
        "",
        `이 subtask의 구현이 복잡하여 별도 Plan이 필요합니다.`,
        subtask.details ? `\n### 기존 상세 설계\n${subtask.details}` : "",
        "",
        `이 subtask를 위한 별도 Plan을 \`<!-- tunaflow:plan-proposal -->\` 형식으로 제안하세요.`,
        `부모 Plan: ${plan.title}`,
      ].filter(Boolean).join("\n");
      await sendWithEngine("claude", prompt);
    } catch (e) { console.warn("[tunaflow]", e); }
    setBusy(false);
  };

  if (loading) {
    return <p className="text-xs text-muted-foreground px-2">Loading...</p>;
  }

  return (
    <div className="space-y-3">
      {/* Plan header + branch link */}
      <div className="rounded-lg border border-border bg-card p-3">
        <div className="flex items-center gap-2">
          <ClipboardList className="w-4 h-4 text-primary/60" />
          <span className="text-xs font-medium text-foreground flex-1">{plan.title}</span>
          <button onClick={() => setShowDoc(true)} className="flex items-center gap-1 text-[9px] text-muted-foreground/50 hover:text-primary/60 transition-colors">
            <FileText className="w-3 h-3" />문서
          </button>
          {plan.implementationBranchId && (
            <button onClick={handleOpenBranch} className="flex items-center gap-1 text-[9px] text-primary/60 hover:text-primary transition-colors">
              <GitBranch className="w-3 h-3" />Branch 열기
            </button>
          )}
        </div>
      </div>

      {/* Subtask progress */}
      <div className="space-y-1.5">
        {subtasks.map((st, i) => {
          const num = i + 1;
          const isDone = completedNums.has(num);
          const isNext = !isDone && !completedNums.has(num) && (i === 0 || completedNums.has(i));

          return (
            <div key={st.id} className={cn(
              "rounded-md border p-2.5 flex items-start gap-2",
              isDone ? "border-status-approved/30 bg-status-approved/5" :
              isNext ? "border-primary/30 bg-primary/5" :
              "border-border bg-card"
            )}>
              {/* Status icon */}
              <div className="shrink-0 mt-0.5">
                {isDone ? <Check className="w-3.5 h-3.5 text-status-approved" /> :
                 isNext ? <Loader2 className="w-3.5 h-3.5 text-primary animate-spin" /> :
                 <Clock className="w-3.5 h-3.5 text-muted-foreground/30" />}
              </div>

              {/* Content */}
              <div className="flex-1 min-w-0">
                <p className={cn("text-[11px] font-medium leading-snug",
                  isDone ? "text-status-approved/80" : "text-foreground"
                )}>
                  {num}. {st.title}
                </p>
                {st.details && (
                  <p className="text-[10px] text-muted-foreground/60 leading-snug mt-0.5 line-clamp-2">{st.details}</p>
                )}

                {/* Actions for failed/done subtasks */}
                <div className="flex items-center gap-2 mt-1.5">
                  {isDone && (
                    <span className="text-[9px] text-status-approved/60">완료</span>
                  )}
                  {!isDone && (
                    <button onClick={() => handleRerunSubtask(st, i)} disabled={busy}
                      className="flex items-center gap-0.5 text-[9px] text-primary/60 hover:text-primary disabled:opacity-40 transition-colors">
                      <RotateCcw className="w-2.5 h-2.5" />재수행
                    </button>
                  )}
                  <button onClick={() => handleCreateSubPlan(st, i)} disabled={busy}
                    className="flex items-center gap-0.5 text-[9px] text-muted-foreground/50 hover:text-foreground disabled:opacity-40 transition-colors">
                    <Plus className="w-2.5 h-2.5" />Sub-plan
                  </button>
                </div>
              </div>
            </div>
          );
        })}
      </div>

      {/* Rework notice */}
      {plan.phase === "rework" && (
        <div className="rounded-md border border-status-rejected/30 bg-status-rejected/5 p-2.5 text-[10px] text-status-rejected space-y-2">
          <p className="font-medium">Rework 필요 — Review에서 다음 사항이 지적되었습니다.</p>
          {reviewVerdict && reviewVerdict.findings.length > 0 && (
            <ul className="space-y-0.5 text-[9px] text-foreground/60 pl-2">
              {reviewVerdict.findings.map((f, i) => <li key={i}>- {f.slice(0, 200)}</li>)}
            </ul>
          )}
          {reviewVerdict && reviewVerdict.recommendations.length > 0 && (
            <div className="text-[9px] text-muted-foreground/50">
              Recommendations: {reviewVerdict.recommendations.map((r) => r.slice(0, 100)).join("; ")}
            </div>
          )}
          <div className="flex items-center gap-2">
            <button
              onClick={async () => {
                if (!plan.implementationBranchId) return;
                setBusy(true);
                try {
                  await planApi.updatePlanPhase(plan.id, "implementation");
                  await planApi.createPlanEvent(plan.id, "rework_requested", "user");
                  await openThread(plan.implementationBranchId);
                  const findingItems = reviewVerdict?.findings.map((f, i) => {
                    // Extract file path from finding if present
                    const fileMatch = f.match(/([a-zA-Z0-9_./-]+\.[a-zA-Z]+(?:#L\d+)?)/);
                    const file = fileMatch ? fileMatch[1] : "";
                    const summary = f.slice(0, 150);
                    return file
                      ? `□ ${i + 1}. ${summary}\n  파일: ${file}`
                      : `□ ${i + 1}. ${summary}`;
                  }) ?? [];
                  const recItems = reviewVerdict?.recommendations.map((r) => `• ${r.slice(0, 100)}`) ?? [];

                  const reworkPrompt = [
                    `┌─ Rework ──────────────────────────────┐`,
                    `│`,
                    `│ 수정 항목 (${findingItems.length}건):`,
                    `│`,
                    ...findingItems.map((f) => `│ ${f}`),
                    `│`,
                    ...(recItems.length > 0 ? [`│ Recommendations:`, ...recItems.map((r) => `│ ${r}`), `│`] : []),
                    `│ 완료 조건: 위 항목 모두 해결 후`,
                    `│ <!-- tunaflow:impl-complete --> 포함`,
                    `└──────────────────────────────────────┘`,
                  ].join("\n");
                  const shadowConvId = `branch:${plan.implementationBranchId}`;
                  const saved = useChatStore.getState().getConversationEngine(shadowConvId);
                  await sendThreadMessage(reworkPrompt, saved?.engine ?? "claude");
                  onPlanUpdate(plan.id, { phase: "implementation" as PlanPhase });
                } catch (e) { console.warn("[tunaflow]", e); }
                setBusy(false);
              }}
              disabled={busy}
              className="px-2.5 py-1 rounded-md text-[10px] font-medium bg-primary/10 text-primary hover:bg-primary/20 disabled:opacity-50 transition-colors"
            >
              {busy ? "전달 중..." : "Developer에게 전달 + Rework"}
            </button>
            <button
              onClick={async () => {
                setBusy(true);
                try {
                  await planApi.updatePlanPhase(plan.id, "subtask_review");
                  await planApi.createPlanEvent(plan.id, "reverted_to_subtask_review", "user", "Design change needed from rework");
                  onPlanUpdate(plan.id, { phase: "subtask_review" as PlanPhase });
                } catch (e) { console.warn("[tunaflow]", e); }
                setBusy(false);
              }}
              disabled={busy}
              className="px-2.5 py-1 rounded-md text-[10px] font-medium bg-accent text-muted-foreground hover:text-foreground disabled:opacity-50 transition-colors"
            >
              설계 변경 → Subtask
            </button>
          </div>
        </div>
      )}

      {/* Test results — informational, does not block Review */}
      {testRunning && (
        <div className="rounded-md border border-primary/20 bg-primary/5 p-2.5 text-[10px] text-primary flex items-center gap-2">
          <Loader2 className="w-3.5 h-3.5 animate-spin" />테스트 실행 중...
        </div>
      )}
      {testResult && !testRunning && (
        <div className={cn(
          "rounded-md border p-2.5 text-[10px] space-y-1",
          testResult.success
            ? "border-status-approved/30 bg-status-approved/5 text-status-approved"
            : "border-status-rejected/30 bg-status-rejected/5 text-status-rejected"
        )}>
          <div className="font-medium">{testResult.testType} 테스트: {testResult.success ? "PASS" : "FAIL"}</div>
          <div className="flex gap-3 text-[9px]">
            <span>통과: {testResult.passed}</span>
            <span>실패: {testResult.failed}</span>
            {testResult.skipped > 0 && <span>건너뜀: {testResult.skipped}</span>}
            <span>{testResult.durationMs}ms</span>
          </div>
          {!testResult.success && testResult.output && (
            <details className="mt-1">
              <summary className="text-[9px] cursor-pointer text-muted-foreground/60 hover:text-foreground">출력 보기</summary>
              <pre className="text-[8px] mt-1 max-h-32 overflow-auto bg-card/50 rounded p-1.5 whitespace-pre-wrap">
                {testResult.output.slice(0, 2000)}
              </pre>
            </details>
          )}
        </div>
      )}

      {/* Re-review: show previous findings summary before review button */}
      {implComplete && plan.phase !== "rework" && reviewVerdict && (
        <div className="rounded-md border border-amber-500/20 bg-amber-500/5 p-2.5 text-[10px] space-y-1.5">
          <div className="font-medium text-amber-600">이전 Review Findings (Re-review #{(plan.versionMinor || 0) + 1})</div>
          <ul className="space-y-0.5 text-[9px] text-foreground/60 pl-2">
            {reviewVerdict.findings.slice(0, 5).map((f, i) => (
              <li key={i}>□ {f.slice(0, 150)}</li>
            ))}
          </ul>
          <p className="text-[9px] text-muted-foreground/50">위 사항이 수정되었는지 중심으로 재검증됩니다.</p>
        </div>
      )}

      {/* Summary + actions */}
      <div className="flex items-center gap-2 pt-2 border-t border-border/30">
        <span className="text-[10px] text-muted-foreground/50">
          {completedNums.size}/{subtasks.length} 완료
        </span>
        <span className="flex-1" />
        {implComplete && plan.phase !== "rework" && reviewMode === "idle" && (
          <button onClick={() => setReviewMode("select")} disabled={busy}
            className={cn(
              "flex items-center gap-1 px-3 py-1.5 rounded-md text-xs font-medium disabled:opacity-50 transition-colors",
              reviewVerdict
                ? "bg-amber-500/10 text-amber-600 hover:bg-amber-500/20"
                : "bg-status-approved/10 text-status-approved hover:bg-status-approved/20"
            )}>
            <Check className="w-3.5 h-3.5" />{reviewVerdict ? "Re-review 시작" : "Review 시작"}
          </button>
        )}
        {implComplete && reviewMode === "select" && (
          <div className="flex items-center gap-2">
            <select value={selectedReviewerId} onChange={(e) => setSelectedReviewerId(e.target.value)}
              className="text-[10px] bg-input border border-border rounded px-1.5 py-0.5 outline-none">
              {profiles.map((p) => <option key={p.id} value={p.id}>{p.label} ({p.engine})</option>)}
            </select>
            <button onClick={handleStartReview} disabled={busy}
              className="px-2.5 py-1 rounded-md text-[10px] font-medium bg-status-approved/10 text-status-approved hover:bg-status-approved/20 disabled:opacity-50 transition-colors">
              {busy ? "시작 중..." : reviewVerdict ? "Re-review 시작" : "Review 시작"}
            </button>
            <button onClick={() => setReviewMode("idle")}
              className="text-[10px] text-muted-foreground hover:text-foreground transition-colors">취소</button>
          </div>
        )}
      </div>
      {showDoc && <PlanDocumentModal plan={plan} onClose={() => setShowDoc(false)} />}
    </div>
  );
}
