import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { GitBranch, Check, Loader2, Clock, RotateCcw, Plus, ClipboardList, FileText } from "lucide-react";
import type { Plan, PlanPhase, PlanSubtask, Message } from "@/types";
import * as planApi from "@/lib/api/plans";
import { scanCompletedSubtasks, hasImplComplete } from "@/lib/planProposalParser";
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
            setImplComplete(msgs.some((m) => m.role === "assistant" && hasImplComplete(m.content)));
          }
        } catch { /* branch may not exist */ }
      }
      setLoading(false);
    })();

    return () => { cancelled = true; };
  }, [plan.id, plan.implementationBranchId]);

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
    } catch { /* silent */ }
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

      // Phase transition
      await planApi.updatePlanPhase(plan.id, "review");
      await planApi.createPlanEvent(plan.id, "review_started", "user", `reviewer=${selectedProfile.label}`);

      // Create review branch (single reviewer, not RT)
      const slug = plan.title.replace(/[^\w가-힣-]/g, "-").replace(/-+/g, "-").replace(/^-|-$/g, "").toLowerCase().slice(0, 80);
      const input = { conversationId: plan.conversationId, label: `Review: ${plan.title.slice(0, 30)}`, mode: "chat" };
      const branch = await invoke<Branch>("create_branch", { input });
      const shadowConvId = await invoke<string>("open_branch_stream", { branchId: branch.id });
      await planApi.linkPlanBranch(plan.id, "review", branch.id);
      saveConversationEngine(shadowConvId, { profileId: selectedReviewerId, engine: selectedProfile.engine });

      await loadBranches(plan.conversationId);
      await openThread(branch.id);

      // Send review prompt
      const prompt = [
        `[Review] "${plan.title}"`,
        "",
        `Plan 문서: \`docs/plans/${slug}.md\``,
        `구현 결과: \`docs/plans/${slug}-result.md\``,
        `작업 지시서: \`docs/plans/${slug}-task-*.md\``,
        "",
        `Plan 문서와 작업 지시서를 기준으로 구현 결과를 검증하세요.`,
        `\`<!-- tunaflow:review-verdict -->\` 형식으로 verdict를 제출하세요.`,
      ].join("\n");

      await sendThreadMessage(prompt, selectedProfile.engine);
      onPlanUpdate(plan.id, { phase: "review" as PlanPhase, reviewBranchId: branch.id });
    } catch { /* silent */ }
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
    } catch { /* silent */ }
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

      {/* Summary + actions */}
      <div className="flex items-center gap-2 pt-2 border-t border-border/30">
        <span className="text-[10px] text-muted-foreground/50">
          {completedNums.size}/{subtasks.length} 완료
        </span>
        <span className="flex-1" />
        {implComplete && reviewMode === "idle" && (
          <button onClick={() => setReviewMode("select")} disabled={busy}
            className="flex items-center gap-1 px-3 py-1.5 rounded-md text-xs font-medium bg-status-approved/10 text-status-approved hover:bg-status-approved/20 disabled:opacity-50 transition-colors">
            <Check className="w-3.5 h-3.5" />Review 시작
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
              {busy ? "시작 중..." : "Review 시작"}
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
