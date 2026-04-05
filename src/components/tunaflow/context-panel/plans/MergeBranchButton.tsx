import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useChatStore } from "@/stores/chatStore";
import { Merge } from "lucide-react";
import type { Plan, PlanPhase, Message } from "@/types";
import { syncPlanDocument } from "@/lib/workflowOrchestration";
import * as planApi from "@/lib/api/plans";
import { splitPlanProposals, hasPlanProposal } from "@/lib/planProposalParser";
import { errorMessage } from "@/lib/utils";
import { toast } from "sonner";

export function MergeBranchButton({
  plan,
  branchId,
  branchType,
  onPlanUpdate,
}: {
  plan: Plan;
  branchId: string;
  branchType: "review" | "implementation";
  onPlanUpdate: (update: Partial<Plan>) => void;
}) {
  const { closeThread, loadBranches } = useChatStore();
  const [busy, setBusy] = useState(false);

  const handleMerge = async () => {
    setBusy(true);
    try {
      // Load branch messages, find last plan-proposal
      const shadowConvId = `branch:${branchId}`;
      const msgs = await invoke<Message[]>("list_messages", { conversationId: shadowConvId });
      const lastAssistant = [...msgs].reverse().find((m) => m.role === "assistant" && hasPlanProposal(m.content));
      if (lastAssistant) {
        const segments = splitPlanProposals(lastAssistant.content);
        const proposalSeg = segments.find((s) => s.type === "plan-proposal");
        if (proposalSeg && proposalSeg.type === "plan-proposal") {
          const p = proposalSeg.proposal;
          await planApi.replacePlanSubtasks(plan.id, p.subtasks.map((s) => ({ title: s.title, details: s.details })));
          await planApi.createPlanEvent(plan.id, "review_merged", "user", `Merged from branch ${branchId} (rev.${plan.revision + 1})`);
          syncPlanDocument(plan.id);

          // Archive the merged branch — it served its purpose
          await invoke("archive_branch", { id: branchId }).catch((e) => console.debug("[archive]", e));
          // Clear the branch link on the plan
          await planApi.linkPlanBranch(plan.id, branchType, null);
          // Return to approval phase for re-review
          await planApi.updatePlanPhase(plan.id, "approval");
          const update: Partial<Plan> = branchType === "review"
            ? { reviewBranchId: undefined, phase: "approval" as PlanPhase }
            : { implementationBranchId: undefined, phase: "approval" as PlanPhase };
          onPlanUpdate(update);

          // Close thread drawer if this branch was open
          closeThread();
          await loadBranches(plan.conversationId);
        }
      }
    } catch (e) {
      console.error("[MergeBranchButton] merge failed:", e);
      toast.error("Plan 병합 실패: " + errorMessage(e));
    }
    setBusy(false);
  };

  return (
    <button onClick={handleMerge} disabled={busy} className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-[9px] font-medium text-primary/70 hover:text-primary hover:bg-primary/10 disabled:opacity-50 transition-colors">
      <Merge className="w-3 h-3" />{busy ? "병합 중..." : "Plan에 병합"}
    </button>
  );
}
