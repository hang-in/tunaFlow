import { useState, useEffect } from "react";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { Check, RotateCcw, ClipboardList, FileText, ArrowLeft, ChevronDown, ChevronRight, PenLine } from "lucide-react";
import type { Plan, PlanPhase, PlanSubtask } from "@/types";
import * as planApi from "@/lib/api/plans";
import { syncPlanDocument } from "@/lib/workflowOrchestration";
import { PlanDocumentModal } from "./PlanDocumentModal";
import { SUBTASK_STATUS_CFG } from "./plans/constants";

interface SubtaskReviewViewProps {
  plan: Plan;
  onPlanUpdate: (planId: string, update: Partial<Plan>) => void;
  onSwitchToChat?: () => void;
}

export function SubtaskReviewView({ plan, onPlanUpdate, onSwitchToChat }: SubtaskReviewViewProps) {
  const { sendWithEngine, selectedConversationId, getConversationEngine } = useChatStore();
  const [subtasks, setSubtasks] = useState<PlanSubtask[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [showDoc, setShowDoc] = useState(false);

  useEffect(() => {
    setLoading(true);
    planApi.listSubtasks(plan.id)
      .then(setSubtasks)
      .catch(() => setSubtasks([]))
      .finally(() => setLoading(false));
  }, [plan.id, plan.revision]);

  const isActionable = plan.phase === "subtask_review";

  // Resolve main chat's agent engine (not hardcoded "claude")
  const mainEngine = (() => {
    if (!selectedConversationId) return "claude";
    const saved = getConversationEngine(selectedConversationId);
    return saved?.engine ?? "claude";
  })();

  const handleApprove = async () => {
    if (!isActionable) return;
    setBusy(true);
    try {
      await planApi.updatePlanPhase(plan.id, "approval");
      await planApi.createPlanEvent(plan.id, "subtask_review_completed", "user");
      syncPlanDocument(plan.id);
      onPlanUpdate(plan.id, { phase: "approval" as PlanPhase });
    } catch { /* silent */ }
    setBusy(false);
  };

  const handleBackToPlan = async () => {
    if (!isActionable) return;
    setBusy(true);
    try {
      await planApi.updatePlanPhase(plan.id, "drafting");
      await planApi.createPlanEvent(plan.id, "reverted_to_drafting", "user");
      onPlanUpdate(plan.id, { phase: "drafting" as PlanPhase });
    } catch { /* silent */ }
    setBusy(false);
  };

  const handleRevisionRequest = async (subtaskIdx: number, opinion: string) => {
    if (!isActionable) return;
    setBusy(true);
    try {
      const list = subtasks.map((s, i) =>
        `${i + 1}. ${s.title}${s.details ? ` — ${s.details}` : ""}`
      ).join("\n");

      const planContext = `## Plan: ${plan.title}\n${plan.description ?? ""}\n\n### Subtasks\n${list}`;
      const prompt = [
        `[Subtask 검토 — 수정 요청] "${plan.title}" Subtask ${subtaskIdx + 1}`,
        "",
        `### 검토 의견`,
        opinion,
        "",
        planContext,
        "",
        `위 검토 의견을 반영하여 수정된 Plan을 \`<!-- tunaflow:plan-proposal -->\` 형식으로 제안하세요.`,
      ].join("\n");

      await sendWithEngine(mainEngine, prompt);
      await planApi.createPlanEvent(plan.id, "subtask_revision_requested", "user",
        `subtask ${subtaskIdx + 1}: ${opinion.slice(0, 100)}`);
      onSwitchToChat?.();
    } catch { /* silent */ }
    setBusy(false);
  };

  const handleDetailRequest = async (subtaskIdx: number) => {
    if (!isActionable) return;
    setBusy(true);
    try {
      const st = subtasks[subtaskIdx];
      const prompt = [
        `[작업 지시서 작성 요청] "${plan.title}" Subtask ${subtaskIdx + 1}: "${st.title}"`,
        "",
        `이 subtask의 상세 작업 지시서(how)를 작성해주세요.`,
        `수정/생성할 파일, 접근 방법, 주의사항을 포함하세요.`,
        "",
        `\`<!-- tunaflow:plan-proposal -->\` 형식으로 이 subtask의 details가 포함된 수정 Plan을 제안하세요.`,
      ].join("\n");

      await sendWithEngine(mainEngine, prompt);
      await planApi.createPlanEvent(plan.id, "detail_design_requested", "user",
        `subtask ${subtaskIdx + 1}`);
      onSwitchToChat?.();
    } catch { /* silent */ }
    setBusy(false);
  };

  if (loading) {
    return <p className="text-xs text-muted-foreground px-2">Loading...</p>;
  }

  return (
    <div className="space-y-3">
      {/* Plan header */}
      <div className="rounded-lg border border-border bg-card p-3">
        <div className="flex items-center gap-2 mb-1.5">
          <ClipboardList className="w-4 h-4 text-primary/60" />
          <span className="text-xs font-medium text-foreground flex-1">{plan.title}</span>
          {plan.revision > 0 && (
            <span className="text-[8px] font-mono text-muted-foreground/50 px-1 rounded bg-accent/50">rev.{plan.revision}</span>
          )}
          <button onClick={() => setShowDoc(true)} className="flex items-center gap-1 text-[9px] text-primary/60 hover:text-primary transition-colors">
            <FileText className="w-3 h-3" />문서 보기
          </button>
        </div>
        {plan.description && (
          <p className="text-[11px] text-muted-foreground leading-snug mb-1">{plan.description}</p>
        )}
        {plan.expectedOutcome && (
          <p className="text-[10px] text-muted-foreground/60 italic">Goal: {plan.expectedOutcome}</p>
        )}
      </div>

      {/* Subtask review list — clickable cards */}
      <div className="space-y-1.5">
        {subtasks.map((st, i) => (
          <SubtaskReviewCard
            key={st.id}
            subtask={st}
            index={i}
            onRevisionRequest={(opinion) => handleRevisionRequest(i, opinion)}
            onDetailRequest={() => handleDetailRequest(i)}
            busy={busy}
            actionable={isActionable}
          />
        ))}
      </div>

      {/* Actions */}
      {isActionable && (
        <div className="flex items-center gap-2 pt-2 border-t border-border/30 flex-wrap">
          <button onClick={handleApprove} disabled={busy}
            className="flex items-center gap-1 px-3 py-1.5 rounded-md text-xs font-medium bg-status-approved/10 text-status-approved hover:bg-status-approved/20 disabled:opacity-50 transition-colors">
            <Check className="w-3.5 h-3.5" />승인 → Approved
          </button>
          <button onClick={handleBackToPlan} disabled={busy}
            className="flex items-center gap-1 px-3 py-1.5 rounded-md text-xs font-medium bg-accent text-muted-foreground hover:text-foreground disabled:opacity-50 transition-colors">
            <ArrowLeft className="w-3.5 h-3.5" />Plan 수정
          </button>
          {/* Debug: 전체 작업지시서 일괄 요청 — details 없는 subtask가 있을 때만 */}
          {subtasks.some((s) => !s.details?.trim()) && (
            <button
              onClick={async () => {
                setBusy(true);
                try {
                  const list = subtasks.map((s, i) =>
                    `${i + 1}. ${s.title}${s.details ? ` — ${s.details}` : " — (미작성)"}`
                  ).join("\n");
                  const prompt = [
                    `[전체 작업지시서 작성 요청] "${plan.title}"`,
                    "",
                    `아래 Plan의 **모든 subtask**에 상세 작업 지시서(how)를 작성해주세요.`,
                    `각 subtask별로: 수정/생성할 파일, 접근 방법, 주의사항을 details에 포함하세요.`,
                    "",
                    `## Plan: ${plan.title}`,
                    plan.description ?? "",
                    "",
                    `### Subtasks`,
                    list,
                    "",
                    `\`<!-- tunaflow:plan-proposal -->\` 형식으로 모든 subtask에 details가 포함된 수정 Plan을 제안하세요.`,
                  ].join("\n");
                  await sendWithEngine(mainEngine, prompt);
                  await planApi.createPlanEvent(plan.id, "detail_design_requested", "user", "all subtasks");
                  onSwitchToChat?.();
                } catch { /* silent */ }
                setBusy(false);
              }}
              disabled={busy}
              className="flex items-center gap-1 px-2.5 py-1 rounded-md text-[10px] font-medium bg-muted text-muted-foreground/60 hover:text-muted-foreground border border-dashed border-border/40 disabled:opacity-40 transition-colors"
              title="디버깅용 — 모든 subtask의 작업지시서를 일괄 요청"
            >
              <PenLine className="w-3 h-3" />전체 작업지시서 (debug)
            </button>
          )}
        </div>
      )}

      {!isActionable && (
        <p className="text-[10px] text-muted-foreground/40 italic pt-2 border-t border-border/30">
          읽기 전용 — 현재 phase: {plan.phase}
        </p>
      )}

      {showDoc && <PlanDocumentModal plan={plan} onClose={() => setShowDoc(false)} />}
    </div>
  );
}

// ─── SubtaskReviewCard (clickable, expandable) ──────────────────────────────

function SubtaskReviewCard({
  subtask,
  index,
  onRevisionRequest,
  onDetailRequest,
  busy,
  actionable,
}: {
  subtask: PlanSubtask;
  index: number;
  onRevisionRequest: (opinion: string) => void;
  onDetailRequest: () => void;
  busy: boolean;
  actionable: boolean;
}) {
  const [expanded, setExpanded] = useState(false);
  const [opinionMode, setOpinionMode] = useState(false);
  const [opinion, setOpinion] = useState("");
  const hasDetails = !!subtask.details?.trim();
  const statusCfg = SUBTASK_STATUS_CFG[subtask.status];

  const handleSubmitOpinion = () => {
    if (!opinion.trim()) return;
    onRevisionRequest(opinion.trim());
    setOpinion("");
    setOpinionMode(false);
  };

  return (
    <div className={cn(
      "rounded-md border transition-colors",
      expanded ? "border-primary/30 bg-primary/[0.03]" :
      hasDetails ? "border-border bg-card" : "border-amber-500/20 bg-amber-500/5",
    )}>
      {/* Summary row — clickable */}
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full flex items-start gap-2 p-2.5 text-left hover:bg-accent/20 transition-colors rounded-md"
      >
        <span className="mt-0.5 shrink-0 text-muted-foreground/40">
          {expanded ? <ChevronDown className="w-3 h-3" /> : <ChevronRight className="w-3 h-3" />}
        </span>
        <span className="text-[10px] text-muted-foreground/40 font-mono shrink-0 mt-0.5 w-4 text-right">{index + 1}.</span>
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-1.5">
            <span className="text-[11px] font-medium text-foreground">{subtask.title}</span>
            <span className={cn("text-[8px] font-semibold px-1 py-0 rounded-full border shrink-0", statusCfg.cls)}>
              {statusCfg.label}
            </span>
            {!hasDetails && (
              <span className="text-[8px] text-amber-600/50 shrink-0">작업지시 없음</span>
            )}
          </div>
          {!expanded && hasDetails && (
            <p className="text-[10px] text-muted-foreground/50 mt-0.5 line-clamp-1">{subtask.details}</p>
          )}
        </div>
      </button>

      {/* Expanded: full work instruction + actions */}
      {expanded && (
        <div className="px-2.5 pb-2.5 ml-9 space-y-2 border-t border-border/20 pt-2">
          {/* Work instruction */}
          {hasDetails ? (
            <div>
              <div className="flex items-center gap-1 mb-1">
                <FileText className="w-3 h-3 text-primary/50" />
                <span className="text-[9px] text-muted-foreground/60 uppercase tracking-wide">작업 지시서</span>
              </div>
              <div className="rounded bg-card/80 border border-border/30 px-3 py-2">
                <p className="text-[11px] text-foreground/80 leading-relaxed whitespace-pre-wrap">{subtask.details}</p>
              </div>
            </div>
          ) : (
            <div className="rounded bg-amber-500/5 border border-amber-500/15 px-3 py-2">
              <p className="text-[10px] text-amber-600/60 mb-1.5">작업 지시서가 아직 작성되지 않았습니다.</p>
              {actionable && (
                <button onClick={(e) => { e.stopPropagation(); onDetailRequest(); }} disabled={busy}
                  className="flex items-center gap-1 px-2 py-0.5 rounded text-[9px] font-medium bg-amber-500/10 text-amber-600 hover:bg-amber-500/20 disabled:opacity-40 transition-colors">
                  <PenLine className="w-2.5 h-2.5" />{busy ? "요청 중..." : "작성 요청"}
                </button>
              )}
            </div>
          )}

          {/* Metadata */}
          {subtask.ownerAgent && (
            <p className="text-[9px] text-muted-foreground/40">Owner: {subtask.ownerAgent}</p>
          )}

          {/* Opinion-based revision request */}
          {actionable && hasDetails && !opinionMode && (
            <div className="flex items-center gap-2 pt-1">
              <button onClick={(e) => { e.stopPropagation(); setOpinionMode(true); }} disabled={busy}
                className="flex items-center gap-0.5 text-[9px] text-amber-600/60 hover:text-amber-600 disabled:opacity-40 transition-colors">
                <RotateCcw className="w-2.5 h-2.5" />수정 요청
              </button>
            </div>
          )}

          {opinionMode && (
            <div className="space-y-1.5 pt-1">
              <textarea
                value={opinion}
                onChange={(e) => setOpinion(e.target.value)}
                placeholder="검토 의견을 작성하세요 (왜 수정이 필요한지)..."
                rows={2}
                className="w-full bg-input rounded-md px-2 py-1.5 text-[10px] outline-none text-foreground placeholder:text-muted-foreground border border-border focus:border-ring/50 resize-none"
                autoFocus
                onClick={(e) => e.stopPropagation()}
              />
              <div className="flex gap-1.5">
                <button onClick={(e) => { e.stopPropagation(); handleSubmitOpinion(); }} disabled={!opinion.trim() || busy}
                  className="px-2 py-0.5 rounded text-[9px] font-medium bg-amber-500/10 text-amber-600 hover:bg-amber-500/20 disabled:opacity-40 transition-colors">
                  수정 요청 전송
                </button>
                <button onClick={(e) => { e.stopPropagation(); setOpinionMode(false); setOpinion(""); }}
                  className="px-2 py-0.5 rounded text-[9px] text-muted-foreground hover:text-foreground transition-colors">
                  취소
                </button>
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
