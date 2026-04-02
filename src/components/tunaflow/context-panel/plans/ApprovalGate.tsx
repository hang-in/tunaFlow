import { useState } from "react";
import { useChatStore } from "@/stores/chatStore";
import { Check, Pause } from "lucide-react";
import type { Plan, PlanPhase, PlanStatus, PlanSubtask } from "@/types";
import * as planApi from "@/lib/api/plans";
import {
  approveAndStartImplementation,
} from "@/lib/workflowOrchestration";
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
  const { openThread, loadBranches, sendThreadMessage, saveConversationEngine } = useChatStore();
  const profiles = useChatStore((s) => s.agentProfiles);
  const [mode, setMode] = useState<"idle" | "agent-select" | "busy">("idle");
  const [selectedProfileId, setSelectedProfileId] = useState(profiles[0]?.id ?? "");

  const selectedProfile = profiles.find((p) => p.id === selectedProfileId);

  const handleDevStart = async () => {
    setMode("busy");
    try {
      const engine = selectedProfile?.engine ?? "claude";
      const { branch, prompt } = await approveAndStartImplementation(plan, engine);
      onPlanUpdate({ phase: "implementation", status: "active" as PlanStatus, implementationBranchId: branch.id });
      await loadBranches(plan.conversationId);

      const shadowConvId = `branch:${branch.id}`;
      saveConversationEngine(shadowConvId, { profileId: selectedProfileId, engine });

      await openThread(branch.id);
      await sendThreadMessage(prompt, engine);
    } catch (e) {
      console.error("[ApprovalGate] dev start failed:", e);
      toast.error("Dev 시작 실패: " + (e instanceof Error ? e.message : String(e)));
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
      toast.error("되돌리기 실패: " + (e instanceof Error ? e.message : String(e)));
      setMode("idle");
    }
  };

  if (mode === "agent-select") {
    return (
      <div className="mt-2 pt-2 border-t border-border/20 space-y-1.5">
        <div className="flex items-center gap-2">
          <span className="text-[10px] text-muted-foreground">Developer 에이전트:</span>
          <select value={selectedProfileId} onChange={(e) => setSelectedProfileId(e.target.value)} className="text-[10px] bg-input border border-border rounded px-1.5 py-0.5 outline-none">
            {profiles.map((p) => <option key={p.id} value={p.id}>{p.label} ({p.engine})</option>)}
          </select>
        </div>
        <div className="flex gap-1.5">
          <button onClick={handleDevStart} className="px-2.5 py-1 rounded-md text-[10px] font-medium bg-status-approved/10 text-status-approved hover:bg-status-approved/20 transition-colors">Dev 시작</button>
          <button onClick={() => setMode("idle")} className="px-2.5 py-1 rounded-md text-[10px] text-muted-foreground hover:text-foreground transition-colors">취소</button>
        </div>
      </div>
    );
  }

  return (
    <div className="flex items-center gap-2 mt-2 pt-2 border-t border-border/20">
      <button onClick={() => setMode("agent-select")} disabled={mode === "busy"} className="flex items-center gap-1 px-2.5 py-1 rounded-md text-[10px] font-medium bg-status-approved/10 text-status-approved hover:bg-status-approved/20 disabled:opacity-50 transition-colors">
        <Check className="w-3 h-3" />Dev 시작
      </button>
      <button onClick={handleRevert} disabled={mode === "busy"} className="flex items-center gap-1 px-2.5 py-1 rounded-md text-[10px] font-medium bg-accent text-muted-foreground hover:text-foreground disabled:opacity-50 transition-colors">
        <Pause className="w-3 h-3" />되돌리기
      </button>
    </div>
  );
}
