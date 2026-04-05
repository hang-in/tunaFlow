import { useState } from "react";
import { useChatStore } from "@/stores/chatStore";
import { FileText, Search } from "lucide-react";
import type { Plan, PlanPhase, PlanSubtask } from "@/types";
import * as planApi from "@/lib/api/plans";
import { errorMessage } from "@/lib/utils";
import { toast } from "sonner";

export function DraftingActions({
  plan,
  subtasks,
  onPlanUpdate,
  onSwitchToChat,
}: {
  plan: Plan;
  subtasks: PlanSubtask[];
  onPlanUpdate: (update: Partial<Plan>) => void;
  onSwitchToChat?: () => void;
}) {
  const { sendWithEngine, selectedConversationId, getConversationEngine } = useChatStore();
  const [busy, setBusy] = useState(false);
  const hasEmptyDetails = subtasks.some((s) => !s.details?.trim());
  const hasSubtasks = subtasks.length > 0;
  const mainEngine = (() => {
    if (!selectedConversationId) return "claude";
    const saved = getConversationEngine(selectedConversationId);
    return saved?.engine ?? "claude";
  })();

  const handleDetailDesign = async () => {
    setBusy(true);
    try {
      const list = subtasks.map((s, i) => {
        const detail = s.details?.trim() ? ` — ${s.details}` : " — (상세 설계 없음)";
        return `${i + 1}. ${s.title}${detail}`;
      }).join("\n");
      const planContext = `## Plan: ${plan.title}\n${plan.description ?? ""}\n\n### Subtasks\n${list}`;
      const prompt = [
        `[상세 설계 요청] "${plan.title}"`,
        "",
        `아래 Plan의 각 subtask에 **구현 방법(how)**을 추가해주세요.`,
        `각 subtask별로: 수정/생성할 파일, 접근 방법, 주의사항을 details에 작성하세요.`,
        "",
        planContext,
        "",
        `\`<!-- tunaflow:plan-proposal -->\` 형식으로 상세 설계가 포함된 수정 Plan을 제안하세요.`,
      ].join("\n");
      await sendWithEngine(mainEngine, prompt);
      await planApi.createPlanEvent(plan.id, "detail_design_requested", "user");
      onSwitchToChat?.();
    } catch (e) {
      console.error("[DraftingActions] detail design request failed:", e);
      toast.error("상세 설계 요청 실패: " + errorMessage(e));
    }
    setBusy(false);
  };

  const handleStartReview = async () => {
    setBusy(true);
    try {
      await planApi.updatePlanPhase(plan.id, "subtask_review");
      await planApi.createPlanEvent(plan.id, "subtask_review_started", "user");
      onPlanUpdate({ phase: "subtask_review" as PlanPhase });
    } catch (e) {
      console.error("[DraftingActions] start review failed:", e);
      toast.error("Subtask 검토 전환 실패: " + errorMessage(e));
    }
    setBusy(false);
  };

  return (
    <div className="mt-2 pt-2 border-t border-border/20 space-y-1.5">
      {hasEmptyDetails && hasSubtasks && (
        <p className="text-[9px] text-amber-600/60">일부 subtask에 상세 설계가 없습니다.</p>
      )}
      <div className="flex items-center gap-2 flex-wrap">
        {hasEmptyDetails && hasSubtasks && (
          <button onClick={handleDetailDesign} disabled={busy} className="flex items-center gap-1 px-2.5 py-1 rounded-md text-[10px] font-medium bg-amber-500/10 text-amber-600 hover:bg-amber-500/20 disabled:opacity-50 transition-colors">
            <FileText className="w-3 h-3" />{busy ? "요청 중..." : "상세 설계 요청"}
          </button>
        )}
        {!hasEmptyDetails && hasSubtasks && (
          <button onClick={handleStartReview} disabled={busy} className="flex items-center gap-1 px-2.5 py-1 rounded-md text-[10px] font-medium bg-primary/10 text-primary hover:bg-primary/20 disabled:opacity-50 transition-colors">
            <Search className="w-3 h-3" />{busy ? "이동 중..." : "Subtask 검토"}
          </button>
        )}
        {!hasSubtasks && (
          <p className="text-[9px] text-muted-foreground/50">Subtask가 없습니다. Chat에서 Architect에게 Plan 수정을 요청하세요.</p>
        )}
      </div>
    </div>
  );
}
