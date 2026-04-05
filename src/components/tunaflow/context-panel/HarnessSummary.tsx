import { useState, useEffect } from "react";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { ClipboardList } from "lucide-react";
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

export type WorkflowStageId = "all" | "plan" | "subtask" | "approved" | "dev" | "review" | "decision";

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
  const [allPlans, setAllPlans] = useState<Plan[]>([]);
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
      setAllPlans(plans);
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
      if (!cancelled) { setAllPlans([]); setActivePlan(null); setSubtasks([]); setLoading(false); }
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

  // Per-stage plan counts
  const livePlans = allPlans.filter((p) => p.status !== "abandoned");
  const stageCounts: Record<string, number> = {
    plan: livePlans.filter((p) => p.phase === "drafting").length,
    subtask: livePlans.filter((p) => p.phase === "subtask_review").length,
    approved: livePlans.filter((p) => p.phase === "approval").length,
    dev: livePlans.filter((p) => p.phase === "implementation" || p.phase === "rework").length,
    review: livePlans.filter((p) => p.phase === "review").length,
    decision: livePlans.filter((p) => p.phase === "done").length,
  };
  const abandonedCount = allPlans.filter((p) => p.status === "abandoned").length;

  return (
    <div className="mb-3 space-y-2">
      {/* Stage chips — clickable as tab navigation */}
      <div className="flex items-center gap-0.5">
        {/* "All" chip — shows total plan count */}
        <button
          onClick={() => onStageClick?.("all" as WorkflowStageId)}
          className={cn(
            "text-[8px] font-medium px-1.5 py-0.5 rounded transition-colors flex items-center gap-1",
            activeStage === "all"
              ? "bg-primary/20 text-primary ring-1 ring-primary/30"
              : "text-foreground/60 hover:bg-accent/60 hover:text-foreground/80"
          )}
        >
          All
          {allPlans.length > 0 && (
            <span className={cn("min-w-[14px] h-3.5 flex items-center justify-center rounded-full text-[7px] font-semibold",
              activeStage === "all" ? "bg-primary/30 text-primary" : "bg-accent text-foreground/50"
            )}>
              {allPlans.length}
            </span>
          )}
        </button>
        <div className="w-3 h-px mx-0.5 bg-border/40" />
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
                  "text-[8px] font-medium px-1.5 py-0.5 rounded transition-colors flex items-center gap-1",
                  isSelected
                    ? "bg-primary/20 text-primary ring-1 ring-primary/30"
                    : "text-foreground/60 hover:bg-accent/60 hover:text-foreground/80"
                )}
              >
                {stage.label}
                {stage.id === "decision"
                  ? (stageCounts.decision + abandonedCount > 0 && (
                      <span className="text-[7px] text-muted-foreground/40">({stageCounts.decision + abandonedCount})</span>
                    ))
                  : (stageCounts[stage.id] > 0 && (
                      <span className={cn("min-w-[14px] h-3.5 flex items-center justify-center rounded-full text-[7px] font-semibold",
                        isSelected ? "bg-primary/30 text-primary" : "bg-accent text-foreground/50"
                      )}>
                        {stageCounts[stage.id]}
                      </span>
                    ))
                }
              </button>
            </div>
          );
        })}
      </div>

      {/* Subtask summary for active plan */}
      {activePlan && subtasks.length > 0 && (() => {
        const doneCount = subtasks.filter((s) => s.status === "done").length;
        return doneCount > 0 ? (
          <p className="text-[9px] text-muted-foreground/40 px-1">
            {activePlan.title}: {doneCount}/{subtasks.length} subtask 완료
          </p>
        ) : null;
      })()}
    </div>
  );
}
