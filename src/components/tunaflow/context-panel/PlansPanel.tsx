import { useState, useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { useChatStore } from "@/stores/chatStore";
import { ClipboardList } from "lucide-react";
import { cn } from "@/lib/utils";
import type { Plan, PlanPhase, PlanStatus } from "@/types";
import * as planApi from "@/lib/api/plans";
import { SubtaskReviewView } from "./SubtaskReviewView";
import { DevProgressView } from "./DevProgressView";
import { PlanCard } from "./plans/PlanCard";

// ─── PlansPanel (main export) ────────────────────────────────────────────────

/** Phase filter mapping for stage IDs.
 *  `plan-check` = drafting + subtask_review 통합 (s37) — "사용자가 plan 을
 *  검토·확정하는" 동일 맥락의 두 phase 를 하나의 stage 로.
 *  emptyKey 는 workflow.plans_panel.* 의 key suffix (t() 로 해석). */
const STAGE_PHASE_MAP: Record<string, { phases: PlanPhase[]; includeAbandoned?: boolean; emptyKey: string }> = {
  "plan-check": { phases: ["drafting", "subtask_review"],          emptyKey: "empty_plan_check" },
  dev:          { phases: ["approval", "implementation", "rework"], emptyKey: "empty_dev" },
  review:       { phases: ["review"],                               emptyKey: "empty_review" },
  done:         { phases: ["done"], includeAbandoned: true,         emptyKey: "empty_done" },
};

interface PlansPanelProps {
  /** Active stage from HarnessSummary — filters plans by phase */
  activeStage?: string;
  /** Callback when a plan's phase changes — parent can update stage */
  onPhaseChanged?: (planId: string, newPhase: PlanPhase) => void;
  /** Callback when a plan's status changes — parent can refresh badge count */
  onStatusChanged?: () => void;
  /** Switch to Chat tab — used after sending prompts to Architect */
  onSwitchToChat?: () => void;
}

export function PlansPanel({ activeStage, onPhaseChanged, onStatusChanged, onSwitchToChat }: PlansPanelProps) {
  const { t } = useTranslation("workflow");
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

  // Meta 알림에서 "이동" 클릭 시 해당 plan 카드로 스크롤 + 하이라이트.
  // Subscribe to `uiRouterSlice.focusedPlanId` directly — MetaFloatingChat
  // writes the request via `focusPlan(id)` (Finding 1-4).
  const focusedPlanId = useChatStore((s) => s.focusedPlanId);
  const focusPlan = useChatStore((s) => s.focusPlan);
  useEffect(() => {
    if (!focusedPlanId) return;
    loadPlans();
    const timer = setTimeout(() => {
      const el = containerRef.current?.querySelector(`[data-plan-id="${focusedPlanId}"]`);
      if (el) {
        el.scrollIntoView({ behavior: "smooth", block: "center" });
        el.classList.add("ring-2", "ring-primary/60");
        setTimeout(() => el.classList.remove("ring-2", "ring-primary/60"), 2000);
      }
      // Clear after handling so re-selecting the same plan re-fires.
      focusPlan(null);
    }, 150);
    return () => clearTimeout(timer);
  }, [focusedPlanId, focusPlan]);

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
      // When marking done/abandoned, also set phase to match (prevents orphaned phase states)
      if (status === "done") {
        await planApi.updatePlanPhase(planId, "done");
        await planApi.createPlanEvent(planId, "manual_done", "user", "수동 완료 처리");
      }
      // When reverting from abandoned → draft/active, reset phase to drafting
      // so the plan appears in the "plan" stage rather than being invisible in all stages
      const isUnAbandoning = status === "draft" || status === "active";
      const wasAbandoned = plans.find((p) => p.id === planId)?.status === "abandoned";
      if (isUnAbandoning && wasAbandoned) {
        await planApi.updatePlanPhase(planId, "drafting");
        await planApi.createPlanEvent(planId, "phase_manual_override", "user", "Reverted from abandoned → drafting");
      }
      // Auto-archive related branches when plan is abandoned or done
      if (status === "abandoned" || status === "done") {
        const plan = plans.find((p) => p.id === planId);
        if (plan?.implementationBranchId) {
          invoke("archive_branch", { id: plan.implementationBranchId }).catch((e) => console.debug("[archive]", e));
        }
        if (plan?.reviewBranchId) {
          invoke("archive_branch", { id: plan.reviewBranchId }).catch((e) => console.debug("[archive]", e));
        }
      }
      const phaseOverride = status === "done" ? "done"
        : (isUnAbandoning && wasAbandoned) ? "drafting"
        : undefined;
      setPlans((prev) => prev.map((p) => (p.id === planId ? { ...p, status, ...(phaseOverride ? { phase: phaseOverride as PlanPhase } : {}) } : p)));
      // Notify parent to refresh badge count
      onStatusChanged?.();
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

  // Filter by active stage — "all" shows everything including abandoned
  const stageCfg = activeStage && activeStage !== "all" ? STAGE_PHASE_MAP[activeStage] : null;
  const filteredPlans = activeStage === "all"
    ? plans
    : stageCfg
      ? plans.filter((p) => {
          // Abandoned plans only show in decision stage (with includeAbandoned flag)
          if (p.status === "abandoned") return !!stageCfg.includeAbandoned;
          return stageCfg.phases.includes(p.phase);
        })
      : plans.filter((p) => p.status !== "abandoned");
  const emptyMessage = stageCfg
    ? t(`plans_panel.${stageCfg.emptyKey}` as "plans_panel.empty_plan_check")
    : t("plans_panel.empty_default");

  return (
    <div ref={containerRef} className="space-y-2">
      {filteredPlans.length === 0 && (
        <div className="text-center py-4">
          <ClipboardList className="w-5 h-5 text-muted-foreground/40 mx-auto mb-2" />
          <p className="text-xs text-muted-foreground">{emptyMessage}</p>
        </div>
      )}

      {activeStage === "plan-check" ? (() => {
        // plan-check = drafting + subtask_review 통합 (s37).
        // - drafting: 아직 subtask 가 채워지지 않은 상태 → 일반 PlanCard 로 노출
        // - subtask_review (normal): 새로 생성돼 검토 대기 중 → SubtaskReviewView
        // - subtask_review (escalated, 이미 impl branch 존재): 설계 재검토 필요 → 별도 섹션
        const draftingPlans = filteredPlans.filter((p) => p.phase === "drafting");
        const normalPlans = filteredPlans.filter((p) => p.phase === "subtask_review" && !p.implementationBranchId);
        const escalatedPlans = filteredPlans.filter((p) => p.phase === "subtask_review" && !!p.implementationBranchId);
        return (
          <>
            {draftingPlans.length > 0 && (
              <div className="space-y-2">
                {(normalPlans.length > 0 || escalatedPlans.length > 0) && (
                  <p className="text-[9px] font-medium text-muted-foreground/40 uppercase tracking-wider px-1">{t("plans_panel.section_drafting")}</p>
                )}
                {draftingPlans.map((plan) => (
                  <div key={plan.id} data-plan-id={plan.id} className="rounded-lg transition-all">
                    <PlanCard
                      plan={plan}
                      onStatusChange={handlePlanStatus}
                      onPlanUpdated={handlePlanUpdated}
                      onSwitchToChat={onSwitchToChat}
                    />
                  </div>
                ))}
              </div>
            )}
            {normalPlans.length > 0 && (
              <div className={cn("space-y-2", draftingPlans.length > 0 && "mt-4")}>
                {(draftingPlans.length > 0 || escalatedPlans.length > 0) && (
                  <p className="text-[9px] font-medium text-muted-foreground/40 uppercase tracking-wider px-1">{t("plans_panel.section_pending_review")}</p>
                )}
                {normalPlans.map((plan) => (
                  <div key={plan.id} data-plan-id={plan.id} className="rounded-lg transition-all">
                    <SubtaskReviewView plan={plan} onPlanUpdate={handlePlanUpdated} onSwitchToChat={onSwitchToChat} />
                  </div>
                ))}
              </div>
            )}
            {escalatedPlans.length > 0 && (
              <div className="space-y-2 mt-4">
                <p className="text-[9px] font-medium text-amber-600/60 uppercase tracking-wider px-1">{t("plans_panel.section_escalated")}</p>
                {escalatedPlans.map((plan) => (
                  <div key={plan.id} data-plan-id={plan.id} className="rounded-lg transition-all">
                    <SubtaskReviewView plan={plan} onPlanUpdate={handlePlanUpdated} onSwitchToChat={onSwitchToChat} />
                  </div>
                ))}
              </div>
            )}
          </>
        );
      })() : activeStage === "dev" ? (
        filteredPlans.map((plan) => (
          <div key={plan.id} data-plan-id={plan.id} className="rounded-lg transition-all">
            <DevProgressView plan={plan} onPlanUpdate={handlePlanUpdated} />
          </div>
        ))
      ) : (
        filteredPlans.map((plan) => (
          <div key={plan.id} data-plan-id={plan.id} className="rounded-lg transition-all">
            <PlanCard
              plan={plan}
              onStatusChange={handlePlanStatus}
              onPlanUpdated={handlePlanUpdated}
              onSwitchToChat={onSwitchToChat}
            />
          </div>
        ))
      )}
    </div>
  );
}
