import { useState, useEffect } from "react";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { Check, RotateCcw, ClipboardList, FileText, ArrowLeft } from "lucide-react";
import type { Plan, PlanPhase, PlanSubtask } from "@/types";
import * as planApi from "@/lib/api/plans";
import { PlanDocumentModal } from "./PlanDocumentModal";

interface SubtaskReviewViewProps {
  plan: Plan;
  onPlanUpdate: (planId: string, update: Partial<Plan>) => void;
}

export function SubtaskReviewView({ plan, onPlanUpdate }: SubtaskReviewViewProps) {
  const { sendWithEngine } = useChatStore();
  const [subtasks, setSubtasks] = useState<PlanSubtask[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [showDoc, setShowDoc] = useState(false);

  useEffect(() => {
    setLoading(true);
    planApi.listSubtasks(plan.id)
      .then(setSubtasks)
      .catch(() => setSubtasks([]))
      .finally(() => setLoading(false));
  }, [plan.id, plan.revision]);

  // Only allow actions if plan is actually in subtask_review phase
  const isActionable = plan.phase === "subtask_review";

  const handleApprove = async () => {
    if (!isActionable) return;
    setBusy(true);
    try {
      await planApi.updatePlanPhase(plan.id, "approval");
      await planApi.createPlanEvent(plan.id, "subtask_review_completed", "user");
      onPlanUpdate(plan.id, { phase: "approval" as PlanPhase });
    } catch { /* silent */ }
    setBusy(false);
  };

  const handleBackToPlan = async () => {
    if (!isActionable) return;
    setBusy(true);
    try {
      await planApi.updatePlanPhase(plan.id, "drafting");
      await planApi.createPlanEvent(plan.id, "reverted_to_drafting", "user");
      onPlanUpdate(plan.id, { phase: "drafting" as PlanPhase });
    } catch { /* silent */ }
    setBusy(false);
  };

  const handleRevisionRequest = async (subtaskIdx: number, opinion: string) => {
    if (!isActionable) return;
    setBusy(true);
    try {
      const list = subtasks.map((s, i) => {
        return `${i + 1}. ${s.title}${s.details ? ` — ${s.details}` : ""}`;
      }).join("\n");

      const planContext = `## Plan: ${plan.title}\n${plan.description ?? ""}\n\n### Subtasks\n${list}`;
      const prompt = [
        `[Subtask 검토 — 수정 요청] "${plan.title}" Subtask ${subtaskIdx + 1}`,
        "",
        `### 검토 의견`,
        opinion,
        "",
        planContext,
        "",
        `위 검토 의견을 반영하여 수정된 Plan을 \`<!-- tunaflow:plan-proposal -->\` 형식으로 제안하세요.`,
      ].join("\n");

      await sendWithEngine("claude", prompt);
      await planApi.createPlanEvent(plan.id, "subtask_revision_requested", "user",
        `subtask ${subtaskIdx + 1}: ${opinion.slice(0, 100)}`);
    } catch { /* silent */ }
    setBusy(false);
  };

  if (loading) {
    return <p className="text-xs text-muted-foreground px-2">Loading...</p>;
  }

  return (
    <div className="space-y-3">
      {/* Plan header + document view button */}
      <div className="rounded-lg border border-border bg-card p-3">
        <div className="flex items-center gap-2 mb-1.5">
          <ClipboardList className="w-4 h-4 text-primary/60" />
          <span className="text-xs font-medium text-foreground flex-1">{plan.title}</span>
          {plan.revision > 0 && (
            <span className="text-[8px] font-mono text-muted-foreground/50 px-1 rounded bg-accent/50">rev.{plan.revision}</span>
          )}
          <button onClick={() => setShowDoc(true)} className="flex items-center gap-1 text-[9px] text-primary/60 hover:text-primary transition-colors">
            <FileText className="w-3 h-3" />문서 보기
          </button>
        </div>
        {plan.description && (
          <p className="text-[11px] text-muted-foreground leading-snug mb-1">{plan.description}</p>
        )}
        {plan.expectedOutcome && (
          <p className="text-[10px] text-muted-foreground/60 italic">Goal: {plan.expectedOutcome}</p>
        )}
      </div>

      {/* Subtask review list */}
      <div className="space-y-2">
        {subtasks.map((st, i) => (
          <SubtaskReviewCard
            key={st.id}
            subtask={st}
            index={i}
            onRevisionRequest={(opinion) => handleRevisionRequest(i, opinion)}
            busy={busy}
            actionable={isActionable}
          />
        ))}
      </div>

      {/* Actions — only when actionable */}
      {isActionable && (
        <div className="flex items-center gap-2 pt-2 border-t border-border/30">
          <button onClick={handleApprove} disabled={busy}
            className="flex items-center gap-1 px-3 py-1.5 rounded-md text-xs font-medium bg-status-approved/10 text-status-approved hover:bg-status-approved/20 disabled:opacity-50 transition-colors">
            <Check className="w-3.5 h-3.5" />승인 → Approved
          </button>
          <button onClick={handleBackToPlan} disabled={busy}
            className="flex items-center gap-1 px-3 py-1.5 rounded-md text-xs font-medium bg-accent text-muted-foreground hover:text-foreground disabled:opacity-50 transition-colors">
            <ArrowLeft className="w-3.5 h-3.5" />Plan 수정
          </button>
        </div>
      )}

      {/* Read-only notice when viewing from another stage */}
      {!isActionable && (
        <p className="text-[10px] text-muted-foreground/40 italic pt-2 border-t border-border/30">
          읽기 전용 — 현재 phase: {plan.phase}
        </p>
      )}

      {/* Document modal */}
      {showDoc && <PlanDocumentModal plan={plan} onClose={() => setShowDoc(false)} />}
    </div>
  );
}

// ─── SubtaskReviewCard ──────────────────────────────────────────────────────

function SubtaskReviewCard({
  subtask,
  index,
  onRevisionRequest,
  busy,
  actionable,
}: {
  subtask: PlanSubtask;
  index: number;
  onRevisionRequest: (opinion: string) => void;
  busy: boolean;
  actionable: boolean;
}) {
  const [opinionMode, setOpinionMode] = useState(false);
  const [opinion, setOpinion] = useState("");
  const hasDetails = !!subtask.details?.trim();

  const handleSubmit = () => {
    if (!opinion.trim()) return;
    onRevisionRequest(opinion.trim());
    setOpinion("");
    setOpinionMode(false);
  };

  return (
    <div className={cn(
      "rounded-md border p-2.5",
      hasDetails ? "border-border bg-card" : "border-amber-500/20 bg-amber-500/5",
    )}>
      <div className="flex items-start gap-2">
        <span className="text-[10px] text-muted-foreground/50 font-mono shrink-0 mt-0.5 w-4 text-right">{index + 1}.</span>
        <div className="flex-1 min-w-0">
          <p className="text-[11px] font-medium text-foreground leading-snug">{subtask.title}</p>
          {hasDetails ? (
            <p className="text-[10px] text-muted-foreground leading-snug mt-1 whitespace-pre-wrap">{subtask.details}</p>
          ) : (
            <p className="text-[10px] text-amber-600/50 italic mt-1">상세 설계 미작성</p>
          )}

          {/* Opinion input for revision request */}
          {opinionMode && (
            <div className="mt-2 space-y-1.5">
              <textarea
                value={opinion}
                onChange={(e) => setOpinion(e.target.value)}
                placeholder="검토 의견을 작성하세요 (왜 수정이 필요한지)..."
                rows={2}
                className="w-full bg-input rounded-md px-2 py-1.5 text-[10px] outline-none text-foreground placeholder:text-muted-foreground border border-border focus:border-ring/50 resize-none"
                autoFocus
              />
              <div className="flex gap-1.5">
                <button onClick={handleSubmit} disabled={!opinion.trim() || busy}
                  className="px-2 py-0.5 rounded text-[9px] font-medium bg-amber-500/10 text-amber-600 hover:bg-amber-500/20 disabled:opacity-40 transition-colors">
                  수정 요청 전송
                </button>
                <button onClick={() => { setOpinionMode(false); setOpinion(""); }}
                  className="px-2 py-0.5 rounded text-[9px] text-muted-foreground hover:text-foreground transition-colors">
                  취소
                </button>
              </div>
            </div>
          )}

          {/* Action button — only when actionable and not in opinion mode */}
          {actionable && !opinionMode && (
            <div className="flex items-center gap-2 mt-1.5">
              <button onClick={() => setOpinionMode(true)} disabled={busy}
                className="flex items-center gap-0.5 text-[9px] text-amber-600/60 hover:text-amber-600 disabled:opacity-40 transition-colors">
                <RotateCcw className="w-2.5 h-2.5" />수정 요청
              </button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
