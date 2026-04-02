import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ClipboardList, Check, RotateCcw, Merge } from "lucide-react";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import type { ParsedPlanProposal } from "@/lib/planProposalParser";
import type { Plan, PlanEvent } from "@/types";
import * as planApi from "@/lib/api/plans";
import { syncPlanDocument } from "@/lib/workflowOrchestration";

interface PlanProposalCardProps {
  proposal: ParsedPlanProposal;
  conversationId: string;
}

export function PlanProposalCard({ proposal, conversationId }: PlanProposalCardProps) {
  const [status, setStatus] = useState<"loading" | "idle" | "promoting" | "promoted" | "merged" | "revising">("loading");
  const [revisionInput, setRevisionInput] = useState("");
  const [revisionTarget, setRevisionTarget] = useState<Plan | null>(null);
  const activeBranchId = useChatStore((s) => s.activeBranchId);
  const sendWithEngine = useChatStore((s) => s.sendWithEngine);
  const closeThread = useChatStore((s) => s.closeThread);
  const loadBranches = useChatStore((s) => s.loadBranches);
  const autoMergeAttempted = useRef(false);

  // On mount: check if this is a revision response → auto-merge
  useEffect(() => {
    if (autoMergeAttempted.current) return;
    autoMergeAttempted.current = true;

    const canonicalId = conversationId.startsWith("branch:") ? undefined : conversationId;
    if (!canonicalId) { setStatus("idle"); return; }

    planApi.listPlansByConversation(canonicalId).then(async (plans) => {
      const activePlans = plans.filter((p) => p.status !== "abandoned" && p.status !== "done");
      if (activePlans.length === 0) { setStatus("idle"); return; }

      // Check if a plan with the same title already exists (already promoted)
      const alreadyPromoted = activePlans.some((p) =>
        p.title.toLowerCase() === proposal.title.toLowerCase()
      );
      if (alreadyPromoted) {
        // Check if there's a pending revision that needs auto-merge
        // Only auto-merge if the LAST event is a revision request (not already merged/promoted)
        for (const plan of activePlans) {
          if (plan.title.toLowerCase() !== proposal.title.toLowerCase()) continue;
          const events = await planApi.listPlanEvents(plan.id);
          const lastEvent = events[events.length - 1];
          // Only auto-merge for Chat-level revision requests (not subtask slider)
          const lastIsRevisionRequest = lastEvent && (
            lastEvent.eventType === "revision_requested" ||
            lastEvent.eventType === "plan_full_revision_requested"
          );
          if (lastIsRevisionRequest) {
            const isFullRevision = lastEvent.eventType === "plan_full_revision_requested";
            setRevisionTarget(plan);
            await autoMerge(plan, isFullRevision);
            return;
          }
        }
        // Already promoted, no pending revision → just show "등록됨"
        setStatus("promoted");
        return;
      }

      // No matching plan — this is a fresh proposal
      setStatus("idle");
    }).catch(() => setStatus("idle"));
  }, [conversationId]);

  const autoMerge = async (targetPlan: Plan, isMajorRevision: boolean = false) => {
    setStatus("promoting");
    try {
      // Safety check: don't replace if proposal has far fewer subtasks than existing
      const existingSubtasks = await planApi.listSubtasks(targetPlan.id);
      if (existingSubtasks.length > 2 && proposal.subtasks.length < existingSubtasks.length / 2) {
        console.warn(`[auto-merge] Blocked: proposal has ${proposal.subtasks.length} subtasks but existing has ${existingSubtasks.length}. Falling back to manual.`);
        setStatus("idle");
        return;
      }

      await planApi.replacePlanSubtasks(targetPlan.id, proposal.subtasks.map((s) => ({
        title: s.title,
        details: s.details,
      })));
      await planApi.createPlanEvent(targetPlan.id, "review_merged", "system",
        isMajorRevision ? `Full plan revision (major)` : `Auto-merged revision (minor)`);
      if (isMajorRevision) {
        await planApi.bumpPlanMajorVersion(targetPlan.id);
      }
      syncPlanDocument(targetPlan.id);

      // Archive old implementation branch
      if (targetPlan.implementationBranchId) {
        await invoke("archive_branch", { id: targetPlan.implementationBranchId }).catch(() => {});
        await planApi.linkPlanBranch(targetPlan.id, "implementation", null);
        closeThread();
        await loadBranches(targetPlan.conversationId);
      }

      await planApi.updatePlanPhase(targetPlan.id, "approval");
      setStatus("merged");
    } catch {
      setStatus("idle"); // Fallback to manual UI
    }
  };

  const handlePromote = async () => {
    setStatus("promoting");
    try {
      const plan = await planApi.createPlan({
        conversationId,
        branchId: activeBranchId ?? undefined,
        title: proposal.title,
        description: proposal.description || undefined,
        expectedOutcome: proposal.expectedOutcome || undefined,
        subtasks: proposal.subtasks.map((s) => ({ title: s.title, details: s.details })),
      });
      // Plan starts at drafting — user must go through Plan → Subtask → Approved
      await planApi.createPlanEvent(plan.id, "promoted", "user", "Promoted from chat");

      // Auto-request Architect to write plan documents
      const slug = proposal.title.replace(/[^\w가-힣-]/g, "-").replace(/-+/g, "-").replace(/^-|-$/g, "").toLowerCase().slice(0, 80);
      const subtaskList = proposal.subtasks.map((s, i) =>
        `${i + 1}. ${s.title}${s.details ? ` — ${s.details}` : ""}`
      ).join("\n");

      const taskFiles = proposal.subtasks.map((s, i) =>
        `- \`docs/plans/${slug}-task-${String(i + 1).padStart(2, "0")}.md\` — ${s.title}`
      );
      const docPrompt = [
        `### 📋 문서 작성 요청`,
        ``,
        `**Plan**: "${proposal.title}"`,
        ``,
        `**작성할 문서**:`,
        `- \`docs/plans/${slug}.md\` — 전체 계획서`,
        ...taskFiles,
        ``,
        `**각 작업 지시서 포함 내용**:`,
        `- 대상 파일 및 경로`,
        `- 구현 접근법 (단계별)`,
        `- 의존성 (패키지, 다른 subtask)`,
        `- 리스크 및 주의사항`,
        `- 완료 기준`,
        ``,
        `> 완료 조건: 모든 문서 작성 후 알려주세요`,
      ].join("\n");

      await sendWithEngine(
        useChatStore.getState().getConversationEngine(conversationId)?.engine ?? "claude",
        docPrompt,
      );
      await planApi.createPlanEvent(plan.id, "detail_design_requested", "system", "Auto-requested on promotion");
      setStatus("promoted");
    } catch {
      setStatus("idle");
    }
  };

  // ─── Status-based renders ──────────────────────────────────────────────────

  if (status === "loading") return null;

  if (status === "merged") {
    const rev = revisionTarget ? revisionTarget.revision + 1 : "?";
    return (
      <div className="my-2 rounded-lg border border-primary/30 bg-primary/5 px-4 py-2.5 text-xs text-primary flex items-center gap-2">
        <Merge className="w-3.5 h-3.5" />
        <span>Plan &quot;{proposal.title}&quot; rev.{rev} — 수정 반영 완료 (재승인 필요)</span>
      </div>
    );
  }

  if (status === "promoted") {
    return (
      <div className="my-2 rounded-lg border border-status-approved/30 bg-status-approved/5 px-4 py-2.5 text-xs text-status-approved flex items-center gap-2">
        <Check className="w-3.5 h-3.5" />
        <span>Plan &quot;{proposal.title}&quot; — Plan 탭에 등록됨</span>
      </div>
    );
  }

  // ─── Full card (fresh proposal only) ──────────────────────────────────────

  return (
    <div className="my-2 rounded-lg border border-primary/20 bg-card/60 overflow-hidden">
      {/* Header */}
      <div className="flex items-center gap-2 px-4 py-2 bg-primary/5 border-b border-primary/10">
        <ClipboardList className="w-4 h-4 text-primary/70" />
        <span className="text-xs font-medium text-foreground/90">
          Plan Proposal: {proposal.title}
        </span>
      </div>

      {/* Body */}
      <div className="px-4 py-3 space-y-2.5 text-xs text-foreground/80">
        {proposal.description && (
          <div>
            <div className="text-[10px] text-muted-foreground/60 uppercase tracking-wide mb-0.5">Description</div>
            <p>{proposal.description}</p>
          </div>
        )}

        {proposal.expectedOutcome && (
          <div>
            <div className="text-[10px] text-muted-foreground/60 uppercase tracking-wide mb-0.5">Expected Outcome</div>
            <p>{proposal.expectedOutcome}</p>
          </div>
        )}

        {proposal.subtasks.length > 0 && (
          <div>
            <div className="text-[10px] text-muted-foreground/60 uppercase tracking-wide mb-1">
              Subtasks ({proposal.subtasks.length})
            </div>
            <ul className="space-y-0.5">
              {proposal.subtasks.map((st, i) => (
                <li key={i} className="flex items-start gap-1.5">
                  <span className="text-muted-foreground/40 shrink-0 w-4 text-right">{i + 1}.</span>
                  <span>
                    {st.title}
                    {st.details && (
                      <span className="text-muted-foreground/50"> — {st.details}</span>
                    )}
                  </span>
                </li>
              ))}
            </ul>
          </div>
        )}

        {proposal.constraints.length > 0 && (
          <div>
            <div className="text-[10px] text-muted-foreground/60 uppercase tracking-wide mb-0.5">Constraints</div>
            <ul className="space-y-0.5">
              {proposal.constraints.map((c, i) => (
                <li key={i} className="flex items-start gap-1.5">
                  <span className="text-muted-foreground/40">-</span>
                  <span>{c}</span>
                </li>
              ))}
            </ul>
          </div>
        )}

        {proposal.nonGoals.length > 0 && (
          <div>
            <div className="text-[10px] text-muted-foreground/60 uppercase tracking-wide mb-0.5">Non-goals</div>
            <ul className="space-y-0.5">
              {proposal.nonGoals.map((ng, i) => (
                <li key={i} className="flex items-start gap-1.5">
                  <span className="text-muted-foreground/40">-</span>
                  <span>{ng}</span>
                </li>
              ))}
            </ul>
          </div>
        )}
      </div>

      {/* Revision input */}
      {status === "revising" && (
        <div className="px-4 py-2 border-t border-border/10 space-y-1.5">
          <textarea
            value={revisionInput}
            onChange={(e) => setRevisionInput(e.target.value)}
            placeholder="수정 요청 내용을 입력하세요..."
            rows={2}
            className="w-full bg-input rounded-md px-2.5 py-1.5 text-xs outline-none text-foreground placeholder:text-muted-foreground border border-border focus:border-ring/50 resize-none"
            autoFocus
          />
          <div className="flex gap-1.5">
            <button
              onClick={async () => {
                if (!revisionInput.trim()) return;
                const feedback = `[Plan 수정 요청: ${proposal.title}]\n\n${revisionInput.trim()}\n\n위 피드백을 반영하여 Plan을 수정하고 \`<!-- tunaflow:plan-proposal -->\` 형식으로 다시 제안하세요.`;
                setStatus("idle");
                setRevisionInput("");
                await sendWithEngine("claude", feedback);
              }}
              className="px-2.5 py-1 rounded-md text-xs font-medium bg-primary/10 text-primary hover:bg-primary/20 transition-colors"
            >
              전송
            </button>
            <button
              onClick={() => { setStatus("idle"); setRevisionInput(""); }}
              className="px-2.5 py-1 rounded-md text-xs text-muted-foreground hover:text-foreground transition-colors"
            >
              취소
            </button>
          </div>
        </div>
      )}

      {/* Actions — fresh proposal only (revision responses auto-merge above) */}
      {status !== "revising" && (
        <div className="flex items-center gap-2 px-4 py-2 border-t border-border/10 bg-white/[0.02]">
          <button
            onClick={handlePromote}
            disabled={status === "promoting"}
            className={cn(
              "flex items-center gap-1.5 px-3 py-1 rounded-md text-xs font-medium transition-colors",
              "bg-primary/10 text-primary hover:bg-primary/20",
              status === "promoting" && "opacity-50 cursor-wait",
            )}
          >
            <Check className="w-3 h-3" />
            {status === "promoting" ? "처리 중..." : "Plan으로 승격"}
          </button>
          <button
            onClick={() => setStatus("revising")}
            className="flex items-center gap-1.5 px-3 py-1 rounded-md text-xs text-muted-foreground hover:text-foreground hover:bg-accent/30 transition-colors"
          >
            <RotateCcw className="w-3 h-3" />
            수정 요청
          </button>
        </div>
      )}
    </div>
  );
}
