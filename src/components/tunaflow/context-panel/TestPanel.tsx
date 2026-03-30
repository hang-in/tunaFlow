import { useState } from "react";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { TestTube, CheckCircle2, Clock, XCircle } from "lucide-react";
import type { Artifact } from "@/types";
import { EvaluationPanel } from "./EvaluationPanel";

const STATUS_ICON: Record<string, React.ReactNode> = {
  draft: <Clock className="w-2.5 h-2.5" />,
  approved: <CheckCircle2 className="w-2.5 h-2.5" />,
  rejected: <XCircle className="w-2.5 h-2.5" />,
};

const STATUS_CLS: Record<string, string> = {
  draft: "text-muted-foreground/60 bg-muted",
  approved: "text-status-approved/70 bg-status-approved/8",
  rejected: "text-status-rejected/70 bg-status-rejected/8",
};

function TestCard({ artifact }: { artifact: Artifact }) {
  const { updateArtifactStatus } = useChatStore();
  // Parse pass/fail from content if available
  const passMatch = artifact.content.match(/(\d+)\s*pass/i);
  const failMatch = artifact.content.match(/(\d+)\s*fail/i);

  return (
    <div className="rounded-md border border-border/30 bg-card/60 p-2.5 space-y-1.5">
      <div className="flex items-start gap-2">
        <TestTube className="w-3.5 h-3.5 text-agent-codex/50 shrink-0 mt-0.5" />
        <div className="flex-1 min-w-0">
          <span className="text-[11px] font-medium text-foreground truncate block">{artifact.title}</span>
          <div className="flex items-center gap-2 mt-0.5">
            <span className={cn("inline-flex items-center gap-0.5 text-[9px] px-1 py-0.5 rounded", STATUS_CLS[artifact.status])}>
              {STATUS_ICON[artifact.status]} {artifact.status}
            </span>
            {passMatch && (
              <span className="text-[9px] text-status-approved/60">{passMatch[1]} pass</span>
            )}
            {failMatch && (
              <span className="text-[9px] text-status-rejected/60">{failMatch[1]} fail</span>
            )}
          </div>
        </div>
      </div>
      <p className="text-[10px] text-foreground/70 leading-relaxed whitespace-pre-wrap line-clamp-8 font-mono">
        {artifact.content}
      </p>
      <div className="flex items-center gap-2 pt-1 border-t border-border/20 text-[9px]">
        {artifact.status !== "approved" && (
          <button onClick={() => updateArtifactStatus(artifact.id, "approved")} className="text-status-approved/70 hover:underline">Approve</button>
        )}
        {artifact.status !== "rejected" && (
          <button onClick={() => updateArtifactStatus(artifact.id, "rejected")} className="text-status-rejected/70 hover:underline">Reject</button>
        )}
        <span className="flex-1" />
        <span className="text-muted-foreground/30 font-mono">
          {new Date(artifact.updatedAt * 1000).toLocaleString()}
        </span>
      </div>
    </div>
  );
}

export function TestPanel() {
  const { artifacts } = useChatStore();
  const [subView, setSubView] = useState<"reports" | "evaluation">("reports");

  const testReports = artifacts
    .filter((a) => a.type === "test-report")
    .sort((a, b) => b.updatedAt - a.updatedAt);

  const approved = testReports.filter((a) => a.status === "approved").length;
  const rejected = testReports.filter((a) => a.status === "rejected").length;
  const draft = testReports.filter((a) => a.status === "draft").length;

  return (
    <div className="space-y-3">
      {/* Sub-view tabs */}
      <div className="flex items-center gap-1 mb-2">
        <button onClick={() => setSubView("reports")}
          className={cn("px-2.5 py-1 rounded-md text-[12px] font-medium transition-colors",
            subView === "reports" ? "bg-accent text-foreground" : "text-muted-foreground/50 hover:text-foreground hover:bg-accent/50")}>
          Reports
        </button>
        <button onClick={() => setSubView("evaluation")}
          className={cn("px-2.5 py-1 rounded-md text-[12px] font-medium transition-colors",
            subView === "evaluation" ? "bg-accent text-foreground" : "text-muted-foreground/50 hover:text-foreground hover:bg-accent/50")}>
          Evaluation
        </button>
      </div>

      {subView === "reports" && (
        <>
          {testReports.length > 0 && (
            <div className="flex items-center gap-3 text-[9px] text-sidebar-foreground/50">
              <span className="flex items-center gap-1">
                <TestTube className="w-3 h-3 text-agent-codex/50" />
                {testReports.length} report{testReports.length > 1 ? "s" : ""}
              </span>
              {approved > 0 && <span className="text-status-approved/60">{approved} passed</span>}
              {rejected > 0 && <span className="text-status-rejected/60">{rejected} failed</span>}
              {draft > 0 && <span className="text-muted-foreground/40">{draft} pending</span>}
            </div>
          )}
          {testReports.length > 0 ? (
            <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
              {testReports.map((a) => <TestCard key={a.id} artifact={a} />)}
            </div>
          ) : (
            <div className="text-center py-6">
              <TestTube className="w-5 h-5 text-muted-foreground/20 mx-auto mb-2" />
              <p className="text-[11px] text-muted-foreground/40">No test reports yet</p>
            </div>
          )}
        </>
      )}

      {subView === "evaluation" && <EvaluationPanel />}
    </div>
  );
}
