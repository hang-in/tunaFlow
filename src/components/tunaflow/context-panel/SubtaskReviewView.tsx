import { useState, useEffect } from "react";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { Check, RotateCcw, ClipboardList, MessageSquare } from "lucide-react";
import type { Plan, PlanPhase, PlanSubtask } from "@/types";
import * as planApi from "@/lib/api/plans";
import { requestPlanRevision } from "@/lib/workflowOrchestration";

interface SubtaskReviewViewProps {
  plan: Plan;
  onPlanUpdate: (planId: string, update: Partial<Plan>) => void;
}

export function SubtaskReviewView({ plan, onPlanUpdate }: SubtaskReviewViewProps) {
  const { sendWithEngine } = useChatStore();
  const [subtasks, setSubtasks] = useState<PlanSubtask[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    setLoading(true);
    planApi.listSubtasks(plan.id)
      .then(setSubtasks)
      .catch(() => setSubtasks([]))
      .finally(() => setLoading(false));
  }, [plan.id, plan.revision]);

  const handleApprove = async () => {
    setBusy(true);
    try {
      await planApi.updatePlanPhase(plan.id, "approval");
      await planApi.createPlanEvent(plan.id, "subtask_review_completed", "user");
      onPlanUpdate(plan.id, { phase: "approval" as PlanPhase });
    } catch { /* silent */ }
    setBusy(false);
  };

  const handleRevisionRequest = async (subtaskIdx?: number) => {
    setBusy(true);
    try {
      const list = subtasks.map((s, i) => {
        const marker = subtaskIdx === i ? " ← 수정 필요" : "";
        return `${i + 1}. ${s.title}${s.details ? ` — ${s.details}` : ""}${marker}`;
      }).join("\n");

      const planContext = `## Plan: ${plan.title}\n${plan.description ?? ""}\n\n### Subtasks\n${list}`;
      const focus = subtaskIdx !== undefined
        ? `\n\nSubtask ${subtaskIdx + 1} "${subtasks[subtaskIdx].title}"의 상세 설계를 수정해주세요.`
        : "\n\n전체 Plan의 상세 설계를 검토하고 수정해주세요.";

      const prompt = [
        `[Subtask 검토 — 수정 요청] "${plan.title}"`,
        "",
        planContext,
        focus,
        "",
        `\`<!-- tunaflow:plan-proposal -->\` 형식으로 수정된 Plan을 제안하세요.`,
      ].join("\n");

      await sendWithEngine("claude", prompt);
      await planApi.createPlanEvent(plan.id, "subtask_revision_requested", "user",
        subtaskIdx !== undefined ? `subtask ${subtaskIdx + 1}` : "all");
    } catch { /* silent */ }
    setBusy(false);
  };

  if (loading) {
    return <p className="text-xs text-muted-foreground px-2">Loading...</p>;
  }

  return (
    <div className="space-y-3">
      {/* Plan header */}
      <div className="rounded-lg border border-border bg-card p-3">
        <div className="flex items-center gap-2 mb-1.5">
          <ClipboardList className="w-4 h-4 text-primary/60" />
          <span className="text-xs font-medium text-foreground">{plan.title}</span>
          {plan.revision > 0 && (
            <span className="text-[8px] font-mono text-muted-foreground/50 px-1 rounded bg-accent/50">rev.{plan.revision}</span>
          )}
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
            onRevisionRequest={() => handleRevisionRequest(i)}
            busy={busy}
          />
        ))}
      </div>

      {/* Actions */}
      <div className="flex items-center gap-2 pt-2 border-t border-border/30">
        <button
          onClick={handleApprove}
          disabled={busy}
          className="flex items-center gap-1 px-3 py-1.5 rounded-md text-xs font-medium bg-status-approved/10 text-status-approved hover:bg-status-approved/20 disabled:opacity-50 transition-colors"
        >
          <Check className="w-3.5 h-3.5" />승인 → Approved
        </button>
        <button
          onClick={() => handleRevisionRequest()}
          disabled={busy}
          className="flex items-center gap-1 px-3 py-1.5 rounded-md text-xs font-medium bg-amber-500/10 text-amber-600 hover:bg-amber-500/20 disabled:opacity-50 transition-colors"
        >
          <RotateCcw className="w-3.5 h-3.5" />{busy ? "요청 중..." : "전체 수정 요청"}
        </button>
      </div>
    </div>
  );
}

// ─── SubtaskReviewCard ──────────────────────────────────────────────────────

function SubtaskReviewCard({
  subtask,
  index,
  onRevisionRequest,
  busy,
}: {
  subtask: PlanSubtask;
  index: number;
  onRevisionRequest: () => void;
  busy: boolean;
}) {
  const hasDetails = !!subtask.details?.trim();

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
            <p className="text-[10px] text-amber-600/50 italic mt-1">상세 설계 없음</p>
          )}
          <div className="flex items-center gap-2 mt-1.5">
            <button
              onClick={onRevisionRequest}
              disabled={busy}
              className="flex items-center gap-0.5 text-[9px] text-amber-600/60 hover:text-amber-600 disabled:opacity-40 transition-colors"
            >
              <RotateCcw className="w-2.5 h-2.5" />수정 요청
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
