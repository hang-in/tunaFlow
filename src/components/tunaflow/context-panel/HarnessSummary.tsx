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
  _branches: Branch[],
  artifacts: Artifact[],
): WorkflowStage[] {
  const phase = plan?.phase;

  // Phase-based progression: each phase implies all prior stages are active
  const PHASE_ORDER = ["drafting", "subtask_review", "approval", "implementation", "rework", "review", "done"];
  const phaseIdx = phase ? PHASE_ORDER.indexOf(phase) : -1;

  const hasReview = artifacts.some((a) => a.type === "review-findings");
  const hasDecision = artifacts.some((a) => a.type === "architect-decision") || phase === "done";

  return [
    { id: "plan", label: "Plan", active: !!plan },
    { id: "subtask", label: "Subtask", active: phaseIdx >= 1 },
    { id: "approved", label: "Approved", active: phaseIdx >= 2 },
    { id: "dev", label: "Dev", active: phaseIdx >= 3 },
    { id: "review", label: "Review", active: phaseIdx >= 5 || hasReview },
    { id: "decision", label: "Decision", active: phaseIdx >= 6 || hasDecision },
  ];
}

// ─── Component ───────────────────────────────────────────────────────────────

export type WorkflowStageId = "plan" | "subtask" | "approved" | "dev" | "review" | "decision";

interface HarnessSummaryProps {
  conversationId: string;
  /** Currently selected stage (tab) */
  activeStage?: WorkflowStageId;
  /** Stage click handler — used as tab navigation */
  onStageClick?: (stageId: WorkflowStageId) => void;
  /** Incremented externally to force plan data reload (e.g. on phase change) */
  refreshKey?: number;
}

export function HarnessSummary({ conversationId, activeStage, onStageClick, refreshKey }: HarnessSummaryProps) {
  const { branches, artifacts } = useChatStore();
  const runningThreadIds = useChatStore((s) => s.runningThreadIds);
  const [activePlan, setActivePlan] = useState<Plan | null>(null);
  const [subtasks, setSubtasks] = useState<PlanSubtask[]>([]);
  const [loading, setLoading] = useState(true);
  const [tick, setTick] = useState(0);

  // Reload plan data when conversation changes or agent run completes
  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    planApi.listPlansByConversation(conversationId).then(async (plans) => {
      if (cancelled) return;
      const livePlans = plans.filter((p) => p.status !== "abandoned" && p.status !== "done");
      const active = livePlans.find((p) => p.status === "active") ?? livePlans[0] ?? null;
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
  }, [conversationId, tick]);

  // Trigger reload when running threads change or phase changes externally
  useEffect(() => {
    setTick((t) => t + 1);
  }, [runningThreadIds.length, refreshKey]);

  if (loading) return null;

  // Stage chips always show (even without active plan — user can navigate)
  // Summary card only shows when there's an active plan

  const stages = deriveStages(activePlan, subtasks, branches, artifacts);

  // Derived counts (only if plan exists)
  const counts = activePlan ? {
    approved: subtasks.filter((s) => s.status === "approved").length,
    inProgress: subtasks.filter((s) => s.status === "in_progress").length,
    done: subtasks.filter((s) => s.status === "done").length,
    todo: subtasks.filter((s) => s.status === "todo").length,
  } : null;
  const linkedBranches = branches.filter((b) => b.subtaskId);
  const reviewCount = artifacts.filter((a) => a.type === "review-findings").length;
  const decisionCount = artifacts.filter((a) => a.type === "architect-decision").length;

  return (
    <div className="mb-3 space-y-2">
      {/* Stage chips — clickable as tab navigation */}
      <div className="flex items-center gap-0.5">
        {stages.map((stage, i) => {
          const isSelected = activeStage === stage.id;
          return (
            <div key={stage.id} className="flex items-center">
              {i > 0 && (
                <div className="w-3 h-px mx-0.5 bg-border/40" />
              )}
              <button
                onClick={() => onStageClick?.(stage.id as WorkflowStageId)}
                className={cn(
                  "text-[8px] font-medium px-1.5 py-0.5 rounded transition-colors",
                  isSelected
                    ? "bg-primary/20 text-primary ring-1 ring-primary/30"
                    : "text-foreground/60 hover:bg-accent/60 hover:text-foreground/80"
                )}
              >
                {stage.label}
              </button>
            </div>
          );
        })}
      </div>

      {/* Compact summary — plan count per stage as inline badges */}
      {activePlan && counts && (
      <div className="flex items-center gap-2 text-[9px] text-muted-foreground/50 px-1">
        {counts.done > 0 && (
          <span className="flex items-center gap-1 text-status-approved/60">
            <span className="w-1.5 h-1.5 rounded-full bg-status-approved/50" />
            {counts.done} done
          </span>
        )}
        {counts.inProgress > 0 && (
          <span className="flex items-center gap-1 text-primary/60">
            <span className="w-1.5 h-1.5 rounded-full bg-primary/50" />
            {counts.inProgress} active
          </span>
        )}
        {counts.todo > 0 && (
          <span>{counts.todo} todo</span>
        )}
        {reviewCount > 0 && (
          <span className="flex items-center gap-1 text-status-draft/60">
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
      )}
    </div>
  );
}
