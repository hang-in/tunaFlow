import { useState, useEffect, useRef } from "react";
import { useChatStore } from "@/stores/chatStore";
import { ClipboardList } from "lucide-react";
import type { Plan, PlanPhase, PlanStatus } from "@/types";
import * as planApi from "@/lib/api/plans";
import { SubtaskReviewView } from "./SubtaskReviewView";
import { DevProgressView } from "./DevProgressView";
import { PlanCard } from "./plans/PlanCard";

// ─── PlansPanel (main export) ────────────────────────────────────────────────

/** Phase filter mapping for stage IDs */
const STAGE_PHASE_MAP: Record<string, { phases: PlanPhase[]; includeAbandoned?: boolean; empty: string }> = {
  plan:     { phases: ["drafting"],                 empty: "Chat 탭에서 Architect와 대화하여 Plan을 생성하세요." },
  subtask:  { phases: ["subtask_review"],           empty: "Subtask 검토 중인 Plan이 없습니다." },
  approved: { phases: ["approval"],                 empty: "Dev 대기 중인 Plan이 없습니다." },
  dev:      { phases: ["implementation", "rework"], empty: "구현 중인 Plan이 없습니다." },
  review:   { phases: ["review"],                   empty: "리뷰 중인 Plan이 없습니다." },
  decision: { phases: ["done"], includeAbandoned: true, empty: "완료된 Plan이 없습니다." },
};

interface PlansPanelProps {
  /** Active stage from HarnessSummary — filters plans by phase */
  activeStage?: string;
  /** Callback when a plan's phase changes — parent can update stage */
  onPhaseChanged?: (planId: string, newPhase: PlanPhase) => void;
  /** Switch to Chat tab — used after sending prompts to Architect */
  onSwitchToChat?: () => void;
}

export function PlansPanel({ activeStage, onPhaseChanged, onSwitchToChat }: PlansPanelProps) {
  const { selectedConversationId, activeBranchId, parentConversationId } = useChatStore();
  const [plans, setPlans] = useState<Plan[]>([]);
  const containerRef = useRef<HTMLDivElement>(null);

  const canonicalConvId = activeBranchId && parentConversationId
    ? parentConversationId
    : selectedConversationId;

  const loadPlans = () => {
    if (!canonicalConvId) return;
    planApi.listPlansByConversation(canonicalConvId)
      .then(setPlans)
      .catch(() => setPlans([]));
  };

  useEffect(() => { loadPlans(); }, [canonicalConvId]);

  // Reload when visible
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const observer = new IntersectionObserver(
      ([entry]) => { if (entry.isIntersecting) loadPlans(); },
      { threshold: 0.1 },
    );
    observer.observe(el);
    return () => observer.disconnect();
  }, [canonicalConvId]);

  const handlePlanStatus = async (planId: string, status: PlanStatus) => {
    try {
      await planApi.updatePlanStatus(planId, status);
      setPlans((prev) => prev.map((p) => (p.id === planId ? { ...p, status } : p)));
    } catch (e) {
      console.error("[PlansPanel] plan status update failed:", e);
    }
  };

  const handlePlanUpdated = (planId: string, update: Partial<Plan>) => {
    setPlans((prev) => prev.map((p) => (p.id === planId ? { ...p, ...update } : p)));
    if (update.phase && onPhaseChanged) onPhaseChanged(planId, update.phase);
  };

  if (!canonicalConvId) {
    return <p className="text-xs text-muted-foreground px-2">No conversation selected.</p>;
  }

  // Filter by active stage
  const stageCfg = activeStage ? STAGE_PHASE_MAP[activeStage] : null;
  const filteredPlans = stageCfg
    ? plans.filter((p) => stageCfg.phases.includes(p.phase) || (stageCfg.includeAbandoned && p.status === "abandoned"))
    : plans;
  const emptyMessage = stageCfg?.empty ?? "No plans yet.";

  return (
    <div ref={containerRef} className="space-y-2">
      {filteredPlans.length === 0 && (
        <div className="text-center py-4">
          <ClipboardList className="w-5 h-5 text-muted-foreground/40 mx-auto mb-2" />
          <p className="text-xs text-muted-foreground">{emptyMessage}</p>
        </div>
      )}

      {activeStage === "subtask" ? (() => {
        // Separate: new plans (never implemented) vs plans that went through Dev→Review cycle
        // Plans with implementationBranchId have been through at least one implementation round
        const normalPlans = filteredPlans.filter((p) => !p.implementationBranchId);
        const escalatedPlans = filteredPlans.filter((p) => !!p.implementationBranchId);
        return (
          <>
            {normalPlans.length > 0 && (
              <div className="space-y-2">
                {normalPlans.length > 0 && escalatedPlans.length > 0 && (
                  <p className="text-[9px] font-medium text-muted-foreground/40 uppercase tracking-wider px-1">검토 대기</p>
                )}
                {normalPlans.map((plan) => (
                  <SubtaskReviewView key={plan.id} plan={plan} onPlanUpdate={handlePlanUpdated} onSwitchToChat={onSwitchToChat} />
                ))}
              </div>
            )}
            {escalatedPlans.length > 0 && (
              <div className="space-y-2 mt-4">
                <p className="text-[9px] font-medium text-amber-600/60 uppercase tracking-wider px-1">⚠️ 설계 재검토 필요</p>
                {escalatedPlans.map((plan) => (
                  <SubtaskReviewView key={plan.id} plan={plan} onPlanUpdate={handlePlanUpdated} onSwitchToChat={onSwitchToChat} />
                ))}
              </div>
            )}
          </>
        );
      })() : activeStage === "dev" ? (
        filteredPlans.map((plan) => (
          <DevProgressView key={plan.id} plan={plan} onPlanUpdate={handlePlanUpdated} />
        ))
      ) : (
        filteredPlans.map((plan) => (
          <PlanCard
            key={plan.id}
            plan={plan}
            onStatusChange={handlePlanStatus}
            onPlanUpdated={handlePlanUpdated}
            onSwitchToChat={onSwitchToChat}
          />
        ))
      )}
    </div>
  );
}
