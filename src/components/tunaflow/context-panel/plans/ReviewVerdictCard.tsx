import { useState } from "react";
import { cn } from "@/lib/utils";
import type { Plan, PlanStatus } from "@/types";
import type { ParsedReviewVerdict } from "@/lib/planProposalParser";
import { processReviewVerdict } from "@/lib/workflowOrchestration";

export function ReviewVerdictCard({
  verdict,
  plan,
  onPlanUpdate,
}: {
  verdict: ParsedReviewVerdict;
  plan: Plan;
  onPlanUpdate: (update: Partial<Plan>) => void;
}) {
  const [busy, setBusy] = useState(false);

  const handleProcess = async () => {
    setBusy(true);
    try {
      await processReviewVerdict(plan, verdict);
      if (verdict.verdict === "pass") {
        onPlanUpdate({ phase: "done", status: "done" as PlanStatus });
      } else if (verdict.verdict === "fail") {
        onPlanUpdate({ phase: "rework" });
      }
    } catch { /* silent */ }
    setBusy(false);
  };

  const verdictColors = {
    pass: "text-status-approved border-status-approved/30 bg-status-approved/5",
    fail: "text-status-rejected border-status-rejected/30 bg-status-rejected/5",
    conditional: "text-agent-gemini border-agent-gemini/30 bg-agent-gemini/5",
  };

  return (
    <div className={cn("mt-2 rounded-md border p-2.5 space-y-1.5", verdictColors[verdict.verdict])}>
      <div className="text-[10px] font-medium uppercase">Review Verdict: {verdict.verdict}</div>
      {verdict.findings.length > 0 && (
        <ul className="space-y-0.5 text-[10px]">
          {verdict.findings.map((f, i) => <li key={i} className="pl-2">- {f}</li>)}
        </ul>
      )}
      {verdict.recommendations.length > 0 && (
        <div className="text-[9px] text-muted-foreground/60">
          Recommendations: {verdict.recommendations.join("; ")}
        </div>
      )}
      {verdict.verdict !== "conditional" ? (
        <button onClick={handleProcess} disabled={busy} className="px-2.5 py-1 rounded-md text-[10px] font-medium bg-card/80 hover:bg-card transition-colors disabled:opacity-50">
          {verdict.verdict === "pass" ? "완료 처리" : "Rework 전환"}
        </button>
      ) : (
        <div className="flex gap-1.5">
          <button onClick={async () => { setBusy(true); await processReviewVerdict(plan, { ...verdict, verdict: "pass" }); onPlanUpdate({ phase: "done", status: "done" as PlanStatus }); setBusy(false); }} disabled={busy} className="px-2.5 py-1 rounded-md text-[10px] font-medium bg-status-approved/10 text-status-approved hover:bg-status-approved/20 disabled:opacity-50 transition-colors">승인</button>
          <button onClick={async () => { setBusy(true); await processReviewVerdict(plan, { ...verdict, verdict: "fail" }); onPlanUpdate({ phase: "rework" }); setBusy(false); }} disabled={busy} className="px-2.5 py-1 rounded-md text-[10px] font-medium bg-status-rejected/10 text-status-rejected hover:bg-status-rejected/20 disabled:opacity-50 transition-colors">Rework</button>
        </div>
      )}
    </div>
  );
}
