import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useChatStore } from "@/stores/chatStore";
import { invoke } from "@tauri-apps/api/core";
import { Check, Pause } from "lucide-react";
import type { Plan, PlanPhase, PlanStatus, PlanSubtask } from "@/types";
import * as planApi from "@/lib/api/plans";
import {
  approveAndStartImplementation,
} from "@/lib/workflowOrchestration";
import { errorMessage } from "@/lib/utils";
import { toast } from "sonner";

export function ApprovalGate({
  plan,
  subtasks,
  onPlanUpdate,
}: {
  plan: Plan;
  subtasks: PlanSubtask[] | null;
  onPlanUpdate: (update: Partial<Plan>) => void;
}) {
  const { t } = useTranslation("workflow");
  const { openThread, loadBranches, sendThreadMessage, saveConversationEngine } = useChatStore();
  const profiles = useChatStore((s) => s.agentProfiles);
  const [mode, setMode] = useState<"idle" | "agent-select" | "busy">("idle");
  // Developer = profile linked to implementer persona; fallback to first profile
  const defaultDeveloper = profiles.find((p) => p.personaId === "persona_implementer") ?? profiles[0];
  const [selectedProfileId, setSelectedProfileId] = useState(defaultDeveloper?.id ?? "");

  const selectedProfile = profiles.find((p) => p.id === selectedProfileId);

  const handleDevStart = async () => {
    setMode("busy");
    try {
      // WIP limit check: warn if too many active plans
      const projectKey = useChatStore.getState().selectedProjectKey;
      if (projectKey) {
        const activePlans = await invoke<number>("count_active_plans", { projectKey }).catch(() => 0);
        if (activePlans >= 5) {
          toast.warning(t("approval.wip_warning", { count: activePlans }));
        }
      }
      const engine = selectedProfile?.engine ?? "claude";
      const { branch, prompt } = await approveAndStartImplementation(plan, engine);
      onPlanUpdate({ phase: "implementation", status: "active" as PlanStatus, implementationBranchId: branch.id });
      await loadBranches(plan.conversationId);

      const shadowConvId = `branch:${branch.id}`;
      saveConversationEngine(shadowConvId, { profileId: selectedProfileId, engine, model: selectedProfile?.model });

      await openThread(branch.id);
      await sendThreadMessage(prompt, engine, selectedProfile?.model);
    } catch (e) {
      console.error("[ApprovalGate] dev start failed:", e);
      toast.error(t("approval.dev_start_error", { error: errorMessage(e) }));
      setMode("idle");
    }
  };

  const handleRevert = async () => {
    setMode("busy");
    try {
      await planApi.updatePlanPhase(plan.id, "subtask_review");
      await planApi.createPlanEvent(plan.id, "reverted_to_subtask_review", "user");
      onPlanUpdate({ phase: "subtask_review" as PlanPhase });
    } catch (e) {
      console.error("[ApprovalGate] revert failed:", e);
      toast.error(t("approval.revert_error", { error: errorMessage(e) }));
      setMode("idle");
    }
  };

  // Dev 진입 UX 단순화: 에이전트 선택 UI 를 상단에 상시 노출(기본 coder preselect) +
  // "Dev 시작" 한 번 클릭으로 즉시 실행. 이전엔 클릭→선택 모드 전환→다시 클릭 3단계였음.
  const engineLabel = selectedProfile ? `${selectedProfile.label} (${selectedProfile.engine}${selectedProfile.model ? `/${selectedProfile.model}` : ""})` : "—";
  return (
    <div className="mt-2 pt-2 border-t border-border/20 space-y-1.5">
      <div className="flex items-center gap-2">
        <span className="text-[10px] text-muted-foreground shrink-0">{t("approval.developer_label")}</span>
        <select
          value={selectedProfileId}
          onChange={(e) => setSelectedProfileId(e.target.value)}
          disabled={mode === "busy"}
          className="flex-1 text-[10px] bg-input border border-border rounded px-1.5 py-0.5 outline-none disabled:opacity-40"
          title={engineLabel}
        >
          {profiles.map((p) => (
            <option key={p.id} value={p.id}>{p.label} ({p.engine}{p.model ? `/${p.model.slice(0, 24)}` : ""})</option>
          ))}
        </select>
      </div>
      <div className="flex gap-1.5">
        <button
          onClick={handleDevStart}
          disabled={mode === "busy" || !selectedProfile}
          className="flex items-center gap-1 px-2.5 py-1 rounded-md text-[10px] font-medium bg-status-approved/10 text-status-approved hover:bg-status-approved/20 disabled:opacity-50 transition-colors"
          title={t("approval.dev_start_tooltip")}
        >
          <Check className="w-3 h-3" />{mode === "busy" ? t("approval.dev_start_busy") : t("approval.dev_start_button")}
        </button>
        <button
          onClick={handleRevert}
          disabled={mode === "busy"}
          className="flex items-center gap-1 px-2.5 py-1 rounded-md text-[10px] font-medium bg-accent text-muted-foreground hover:text-foreground disabled:opacity-50 transition-colors"
        >
          <Pause className="w-3 h-3" />{t("approval.revert_button")}
        </button>
      </div>
    </div>
  );
}
