import { useState, useEffect } from "react";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { ClipboardList, GitBranch, FileSearch, Gavel } from "lucide-react";
import type { Plan, PlanSubtask, Artifact, Branch } from "@/types";
import * as planApi from "@/lib/api/plans";

// ─── Workflow stages derived from data ───────────────────────────────────────

interface WorkflowStage {
  id: string;
  label: string;
  active: boolean;
}

function deriveStages(
  plan: Plan | null,
  subtasks: PlanSubtask[],
  branches: Branch[],
  artifacts: Artifact[],
): WorkflowStage[] {
  const hasApproved = subtasks.some((s) => s.status === "approved");
  const hasInProgress = subtasks.some((s) => s.status === "in_progress");
  const hasDone = subtasks.some((s) => s.status === "done");
  const linkedBranches = branches.filter((b) => b.subtaskId);
  const hasReview = artifacts.some((a) => a.type === "review-findings");
  const hasDecision = artifacts.some((a) => a.type === "architect-decision");

  return [
    { id: "plan", label: "Plan", active: !!plan },
    { id: "approved", label: "Approved", active: hasApproved || hasInProgress || hasDone },
    { id: "dev", label: "Dev", active: linkedBranches.length > 0 || hasInProgress },
    { id: "review", label: "Review", active: hasReview },
    { id: "decision", label: "Decision", active: hasDecision },
  ];
}

// ─── Component ───────────────────────────────────────────────────────────────

export type WorkflowStageId = "plan" | "approved" | "dev" | "review" | "decision";

interface HarnessSummaryProps {
  conversationId: string;
  /** Currently selected stage (tab) */
  activeStage?: WorkflowStageId;
  /** Stage click handler — used as tab navigation */
  onStageClick?: (stageId: WorkflowStageId) => void;
}

export function HarnessSummary({ conversationId, activeStage, onStageClick }: HarnessSummaryProps) {
  const { branches, artifacts } = useChatStore();
  const [activePlan, setActivePlan] = useState<Plan | null>(null);
  const [subtasks, setSubtasks] = useState<PlanSubtask[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    planApi.listPlansByConversation(conversationId).then(async (plans) => {
      if (cancelled) return;
      const active = plans.find((p) => p.status === "active") ?? plans[0] ?? null;
      setActivePlan(active);
      if (active) {
        const tasks = await planApi.listSubtasks(active.id);
        if (!cancelled) setSubtasks(tasks);
      } else {
        setSubtasks([]);
      }
      setLoading(false);
    }).catch(() => {
      if (!cancelled) { setActivePlan(null); setSubtasks([]); setLoading(false); }
    });
    return () => { cancelled = true; };
  }, [conversationId]);

  if (loading || !activePlan) return null;

  // Derived counts
  const counts = {
    approved: subtasks.filter((s) => s.status === "approved").length,
    inProgress: subtasks.filter((s) => s.status === "in_progress").length,
    done: subtasks.filter((s) => s.status === "done").length,
    todo: subtasks.filter((s) => s.status === "todo").length,
  };
  const linkedBranches = branches.filter((b) => b.subtaskId);
  const reviewCount = artifacts.filter((a) => a.type === "review-findings").length;
  const decisionCount = artifacts.filter((a) => a.type === "architect-decision").length;

  const stages = deriveStages(activePlan, subtasks, branches, artifacts);
  // Current stage = last active stage
  const currentIdx = stages.reduce((last, s, i) => (s.active ? i : last), -1);

  return (
    <div className="mb-3 space-y-2">
      {/* Stage chips — clickable as tab navigation */}
      <div className="flex items-center gap-0.5">
        {stages.map((stage, i) => {
          const isSelected = activeStage === stage.id;
          return (
            <div key={stage.id} className="flex items-center">
              {i > 0 && (
                <div className={cn(
                  "w-3 h-px mx-0.5",
                  i <= currentIdx ? "bg-primary/30" : "bg-border/40"
                )} />
              )}
              <button
                onClick={() => onStageClick?.(stage.id as WorkflowStageId)}
                className={cn(
                  "text-[8px] font-medium px-1.5 py-0.5 rounded transition-colors",
                  isSelected
                    ? "bg-primary/20 text-primary ring-1 ring-primary/30"
                    : stage.active
                      ? i === currentIdx
                        ? "bg-primary/15 text-primary hover:bg-primary/25"
                        : "bg-accent text-foreground/70 hover:bg-accent/80"
                      : "bg-transparent text-muted-foreground/40 hover:text-muted-foreground/60"
                )}
              >
                {stage.label}
              </button>
            </div>
          );
        })}
      </div>

      {/* Compact summary card */}
      <div className="rounded-md bg-card/50 border border-border/30 px-2.5 py-2 space-y-1.5">
        {/* Plan title */}
        <div className="flex items-center gap-1.5">
          <ClipboardList className="w-3 h-3 text-primary/60 shrink-0" />
          <span className="text-[11px] font-medium text-foreground truncate">{activePlan.title}</span>
        </div>

        {/* Subtask distribution */}
        <div className="flex items-center gap-2 text-[9px]">
          {counts.done > 0 && (
            <span className="flex items-center gap-1 text-status-approved/70">
              <span className="w-1.5 h-1.5 rounded-full bg-status-approved/50" />
              {counts.done} done
            </span>
          )}
          {counts.inProgress > 0 && (
            <span className="flex items-center gap-1 text-primary/70">
              <span className="w-1.5 h-1.5 rounded-full bg-primary/50" />
              {counts.inProgress} active
            </span>
          )}
          {counts.approved > 0 && (
            <span className="flex items-center gap-1 text-agent-gemini/70">
              <span className="w-1.5 h-1.5 rounded-full bg-agent-gemini/50" />
              {counts.approved} approved
            </span>
          )}
          {counts.todo > 0 && (
            <span className="text-muted-foreground/50">{counts.todo} todo</span>
          )}
        </div>

        {/* Linked branches + harness artifacts */}
        <div className="flex items-center gap-2.5 text-[9px] text-muted-foreground/60">
          {linkedBranches.length > 0 && (
            <span className="flex items-center gap-1">
              <GitBranch className="w-2.5 h-2.5" />
              {linkedBranches.length} dev branch{linkedBranches.length > 1 ? "es" : ""}
            </span>
          )}
          {reviewCount > 0 && (
            <span className="flex items-center gap-1 text-status-draft/70">
              <FileSearch className="w-2.5 h-2.5" />
              {reviewCount} review
            </span>
          )}
          {decisionCount > 0 && (
            <span className="flex items-center gap-1 text-primary/60">
              <Gavel className="w-2.5 h-2.5" />
              {decisionCount} decision
            </span>
          )}
        </div>
      </div>
    </div>
  );
}
