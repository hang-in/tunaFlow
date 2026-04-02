import { useState } from "react";
import { cn } from "@/lib/utils";
import type { Plan, PlanPhase, PlanStatus } from "@/types";
import type { ParsedReviewVerdict } from "@/lib/planProposalParser";
import { processReviewVerdict } from "@/lib/workflowOrchestration";
import * as planApi from "@/lib/api/plans";

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

  const handleApprove = async () => {
    setBusy(true);
    try {
      await processReviewVerdict(plan, { ...verdict, verdict: "pass" });
      onPlanUpdate({ phase: "done" as PlanPhase, status: "done" as PlanStatus });
    } catch { /* silent */ }
    setBusy(false);
  };

  const handleRework = async () => {
    setBusy(true);
    try {
      await processReviewVerdict(plan, { ...verdict, verdict: "fail" });
      onPlanUpdate({ phase: "rework" as PlanPhase });
    } catch { /* silent */ }
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

      {/* User decision — always both options regardless of verdict */}
      <div className="flex items-center gap-2 pt-1 border-t border-current/10">
        <span className="text-[9px] text-muted-foreground/50">사용자 판단:</span>
        <button onClick={handleApprove} disabled={busy}
          className="px-2.5 py-1 rounded-md text-[10px] font-medium bg-status-approved/10 text-status-approved hover:bg-status-approved/20 disabled:opacity-50 transition-colors">
          완료 → Decision
        </button>
        <button onClick={handleRework} disabled={busy}
          className="px-2.5 py-1 rounded-md text-[10px] font-medium bg-status-rejected/10 text-status-rejected hover:bg-status-rejected/20 disabled:opacity-50 transition-colors">
          Rework
        </button>
      </div>
    </div>
  );
}
