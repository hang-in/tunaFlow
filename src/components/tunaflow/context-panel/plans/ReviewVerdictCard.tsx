import { useState } from "react";
import { useTranslation } from "react-i18next";
import { cn, errorMessage } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import type { Plan, PlanPhase, PlanStatus } from "@/types";
import type { ParsedReviewVerdict } from "@/lib/planProposalParser";
import { processReviewVerdict, approveAndStartImplementation } from "@/lib/workflowOrchestration";
import * as planApi from "@/lib/api/plans";
import { toast } from "sonner";
import { invoke } from "@tauri-apps/api/core";

export function ReviewVerdictCard({
  verdict,
  plan,
  onPlanUpdate,
}: {
  verdict: ParsedReviewVerdict;
  plan: Plan;
  onPlanUpdate: (update: Partial<Plan>) => void;
}) {
  const { t } = useTranslation("workflow");
  const [busy, setBusy] = useState(false);
  // Done 확인 단계 — fail/conditional 판정일 때 한 번 더 확인
  const [doneConfirm, setDoneConfirm] = useState(false);

  const handleApprove = async () => {
    setBusy(true);
    try {
      await processReviewVerdict(plan, { ...verdict, verdict: "pass" });
      onPlanUpdate({ phase: "done" as PlanPhase, status: "done" as PlanStatus });
    } catch (e) {
      console.error("[ReviewVerdictCard] approve failed:", e);
      toast.error(t("review.verdict.approve_error", { error: errorMessage(e) }));
    }
    setBusy(false);
  };

  const handleRework = async () => {
    setBusy(true);
    try {
      await processReviewVerdict(plan, { ...verdict, verdict: "fail" });
      onPlanUpdate({ phase: "rework" as PlanPhase });
    } catch (e) {
      console.error("[ReviewVerdictCard] rework failed:", e);
      toast.error(t("review.verdict.rework_error", { error: errorMessage(e) }));
    }
    setBusy(false);
  };

  // Conditional-specific: record verdict and send findings to Developer immediately
  const handleSendToDevDirect = async () => {
    if (!plan.implementationBranchId) return;
    setBusy(true);
    try {
      // Record as conditional, then immediately transition to implementation
      await processReviewVerdict(plan, verdict);
      await import("@/lib/api/plans").then((api) => api.updatePlanPhase(plan.id, "implementation"));
      await import("@/lib/api/plans").then((api) => api.createPlanEvent(plan.id, "rework_requested", "user"));
      onPlanUpdate({ phase: "implementation" as PlanPhase });

      const { openThread, sendThreadMessage, getConversationEngine } = useChatStore.getState();
      await openThread(plan.implementationBranchId);
      const saved = getConversationEngine(`branch:${plan.implementationBranchId}`);
      const findings = verdict.findings.length > 0
        ? `\n- ${verdict.findings.join("\n- ")}`
        : t("review.verdict.findings_empty_conditional");
      const recs = verdict.recommendations.length > 0 ? `\n- ${verdict.recommendations.join("\n- ")}` : "";
      const recsBlock = recs ? t("review.verdict.conditional_recs_block", { recs }) : "";
      const prompt = t("review.verdict.conditional_prompt", { findings, recsBlock });
      await sendThreadMessage(prompt, saved?.engine ?? "claude", saved?.model ?? undefined);
      toast.success(t("review.verdict.dev_delivery_success"));
    } catch (e) {
      console.error("[ReviewVerdictCard] sendToDevDirect failed:", e);
      toast.error(t("review.verdict.dev_delivery_error", { error: errorMessage(e) }));
    }
    setBusy(false);
  };

  // 처음부터 재설계: 원안 폐기(abandoned) → Architect에게 findings 전달 → rev.2 작성
  const handleRedesign = async () => {
    setBusy(true);
    try {
      // 1. 리뷰 판정 기록
      await processReviewVerdict(plan, { ...verdict, verdict: "fail" });

      // 2. 현재 플랜 폐기 (원안 abandoned)
      await planApi.updatePlanPhase(plan.id, "done");
      await planApi.updatePlanStatus(plan.id, "abandoned");
      await planApi.createPlanEvent(plan.id, "abandoned", "user", t("review.verdict.redesign_event_reason"));

      // 3. 서브태스크 전부 pending 리셋
      const subtasks = await invoke<{ id: string }[]>("list_subtasks", { planId: plan.id });
      for (const st of subtasks) {
        await invoke("update_subtask_status", { id: st.id, status: "pending" }).catch(() => {});
      }

      // 4. Architect에게 findings 포함 재설계 요청 전송
      const { sendWithEngine, getConversationEngine } = useChatStore.getState();
      const convId = plan.conversationId;
      const saved = getConversationEngine(convId);
      const engine = saved?.engine ?? "claude";

      const findings = verdict.findings.length > 0
        ? verdict.findings.map((f) => `- ${f}`).join("\n")
        : t("review.verdict.findings_empty_redesign");
      const recs = verdict.recommendations.length > 0
        ? verdict.recommendations.map((r) => `- ${r}`).join("\n")
        : "";
      const recsBlock = recs ? t("review.verdict.redesign_recs_block", { recs }) : "";
      const prompt = t("review.verdict.redesign_prompt", {
        title: plan.title,
        revision: plan.revision,
        verdict: verdict.verdict.toUpperCase(),
        findings,
        recsBlock,
        nextRevision: (plan.revision ?? 1) + 1,
      });

      await sendWithEngine(engine, prompt);

      onPlanUpdate({ phase: "done" as PlanPhase, status: "abandoned" as PlanStatus });
      toast.success(t("review.verdict.redesign_success"));
    } catch (e) {
      console.error("[ReviewVerdictCard] redesign failed:", e);
      toast.error(t("review.verdict.redesign_error", { error: errorMessage(e) }));
    }
    setBusy(false);
  };

  const verdictColors = {
    pass: "text-status-approved border-status-approved/30 bg-status-approved/5",
    fail: "text-status-rejected border-status-rejected/30 bg-status-rejected/5",
    conditional: "text-agent-gemini border-agent-gemini/30 bg-agent-gemini/5",
  };

  const verdictLabels = {
    pass: "PASS",
    fail: "FAIL",
    conditional: "CONDITIONAL",
  };

  return (
    <div className={cn("mt-2 rounded-md border p-2.5 space-y-2", verdictColors[verdict.verdict])}>
      {/* Verdict header */}
      <div className="text-[10px] font-medium uppercase">
        Reviewer Verdict: {verdictLabels[verdict.verdict]}
      </div>

      {/* Rubric scores */}
      {verdict.rubric && (
        <div className="flex items-center gap-3 text-[9px]">
          {[
            { label: "Plan", score: verdict.rubric.planCoverage },
            { label: "Code", score: verdict.rubric.codeQuality },
            { label: "Test", score: verdict.rubric.testCoverage },
            { label: "Doc", score: verdict.rubric.docQuality },
            { label: "Conv", score: verdict.rubric.convention },
          ].map(({ label, score }) => (
            <span key={label} className="flex items-center gap-0.5">
              <span className="text-muted-foreground/50">{label}</span>
              <span className={cn(
                "font-medium",
                score >= 4 ? "text-status-approved" : score >= 3 ? "text-foreground" : "text-status-rejected"
              )}>{score}/5</span>
            </span>
          ))}
        </div>
      )}

      {/* Findings */}
      {verdict.findings.length > 0 && (
        <div>
          <div className="text-[9px] text-muted-foreground/60 mb-0.5">Findings:</div>
          <ul className="space-y-0.5 text-[10px]">
            {verdict.findings.map((f, i) => (
              <li key={i} className="pl-2">- {f.slice(0, 200)}</li>
            ))}
          </ul>
        </div>
      )}

      {/* Recommendations */}
      {verdict.recommendations.length > 0 && (
        <div className="text-[9px] text-muted-foreground/60">
          Recommendations: {verdict.recommendations.map((r) => r.slice(0, 100)).join("; ")}
        </div>
      )}

      {/* User decision — only show when plan is still in review/rework phase */}
      {plan.phase !== "done" && (
        <div className="pt-1 border-t border-current/10">
          {doneConfirm ? (
            <div className="space-y-1.5">
              <p className="text-[9px] text-status-rejected/80 leading-snug">
                {t("review.verdict.done_confirm_prefix")}
                <strong>{verdict.verdict === "fail" ? t("review.verdict.verdict_fail_label") : t("review.verdict.verdict_conditional_label")}</strong>
                {t("review.verdict.done_confirm_suffix")}
              </p>
              <div className="flex items-center gap-1.5">
                <button onClick={handleApprove} disabled={busy}
                  className="px-2 py-1 rounded text-[10px] font-medium bg-status-approved/10 text-status-approved hover:bg-status-approved/20 disabled:opacity-50 transition-colors">
                  {t("review.verdict.approve_anyway_button")}
                </button>
                <button onClick={() => setDoneConfirm(false)} disabled={busy}
                  className="px-2 py-1 rounded text-[10px] text-muted-foreground hover:text-foreground disabled:opacity-50 transition-colors">
                  {t("review.verdict.back_button")}
                </button>
              </div>
            </div>
          ) : (
            <div className="flex items-center justify-between gap-1.5 flex-wrap">
              <div className="flex items-center gap-1.5 flex-wrap">
                <span className="text-[9px] text-muted-foreground/50">{t("review.verdict.edit_label")}</span>
                {verdict.verdict === "conditional" && plan.implementationBranchId ? (
                  <button onClick={handleSendToDevDirect} disabled={busy}
                    className="px-2 py-1 rounded text-[10px] font-medium bg-muted/40 text-muted-foreground hover:bg-muted/60 disabled:opacity-50 transition-colors"
                    title={t("review.verdict.code_rewrite_tooltip")}>
                    {t("review.verdict.code_rewrite_button")}
                  </button>
                ) : (
                  <button onClick={handleRework} disabled={busy}
                    className="px-2 py-1 rounded text-[10px] font-medium bg-muted/40 text-muted-foreground hover:bg-muted/60 disabled:opacity-50 transition-colors">
                    {t("review.verdict.code_rewrite_button")}
                  </button>
                )}
                <button onClick={handleRedesign} disabled={busy}
                  className="px-2 py-1 rounded text-[10px] font-medium bg-muted/40 text-muted-foreground hover:bg-muted/60 disabled:opacity-50 transition-colors"
                  title={t("review.verdict.redesign_tooltip")}>
                  {t("review.verdict.redesign_button")}
                </button>
              </div>
              <button
                onClick={verdict.verdict === "pass" ? handleApprove : () => setDoneConfirm(true)}
                disabled={busy}
                className="px-2.5 py-1 rounded text-[10px] font-medium bg-status-approved/10 text-status-approved hover:bg-status-approved/20 disabled:opacity-50 transition-colors">
                {t("review.verdict.done_button")}
              </button>
            </div>
          )}
        </div>
      )}
      {plan.phase === "done" && (
        <div className="pt-1 border-t border-current/10 text-[9px] text-muted-foreground/50">
          {t("review.verdict.auto_completed")}
        </div>
      )}
    </div>
  );
}
