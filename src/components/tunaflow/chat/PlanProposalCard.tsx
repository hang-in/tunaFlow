import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ClipboardList, Check, RotateCcw, Merge, X } from "lucide-react";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import type { ParsedPlanProposal } from "@/lib/planProposalParser";
import { mergeDispositions } from "@/lib/planProposalParser";
import type { Plan, PlanEvent } from "@/types";
import * as planApi from "@/lib/api/plans";
import { slugifyPlanTitle, syncPlanDocument } from "@/lib/workflowOrchestration";

interface PlanProposalCardProps {
  proposal: ParsedPlanProposal;
  conversationId: string;
}

export function PlanProposalCard({ proposal, conversationId }: PlanProposalCardProps) {
  const [status, setStatus] = useState<"loading" | "idle" | "promoting" | "promoted" | "merged" | "revising" | "warn-empty" | "dismissed">("loading");
  const [revisionInput, setRevisionInput] = useState("");
  const [revisionTarget, setRevisionTarget] = useState<Plan | null>(null);
  // b 정책 revision overwrite 후보 — `promoted` 상태에서 active 매칭 plan 이 있고
  // 제안 subtasks 가 기존과 다르면 이 값으로 "rev 로 덮어쓰기" 수동 버튼이 노출된다.
  const [overwriteCandidate, setOverwriteCandidate] = useState<Plan | null>(null);
  const activeBranchId = useChatStore((s) => s.activeBranchId);
  const threadBranchId = useChatStore((s) => s.threadBranchId);
  const threadBranchConvId = useChatStore((s) => s.threadBranchConvId);
  const selectedConversationId = useChatStore((s) => s.selectedConversationId);
  const sendWithEngine = useChatStore((s) => s.sendWithEngine);
  const closeThread = useChatStore((s) => s.closeThread);
  const adoptBranch = useChatStore((s) => s.adoptBranch);
  // Plans must be linked to the main (non-shadow) conversation so they appear in Plans tab
  const planConvId = conversationId.startsWith("branch:") ? (selectedConversationId ?? conversationId) : conversationId;
  const loadBranches = useChatStore((s) => s.loadBranches);
  const autoMergeAttempted = useRef(false);

  // Normalize plan titles for fuzzy matching
  // Strips: "Plan: " prefix, "(상세설계)"/"(수정)" parentheticals, leading/trailing whitespace
  const normalizeTitle = (t: string) =>
    t.toLowerCase()
      .replace(/^plan:\s*/i, "")
      .replace(/^plan\s+/i, "")
      .replace(/\s*[\(（【][^)）】]*[\)）】]\s*/g, "")
      .trim();

  // On mount: check if this is a revision response → auto-merge
  useEffect(() => {
    if (autoMergeAttempted.current) return;
    autoMergeAttempted.current = true;

    // Branch context: use parent conversation to find plans, but never auto-merge from a branch
    // (the main chat PlanProposalCard handles auto-merge; branch card just shows "등록됨")
    const isBranchContext = conversationId.startsWith("branch:");
    const canonicalId = isBranchContext ? (selectedConversationId ?? undefined) : conversationId;
    if (!canonicalId) { setStatus("idle"); return; }

    planApi.listPlansByConversation(canonicalId).then(async (plans) => {
      // Search ALL plans (including done/abandoned) for a title match.
      // A done/abandoned plan still means the proposal was already acted on — suppress the promote button.
      const matchingPlan = plans.find((p) =>
        normalizeTitle(p.title) === normalizeTitle(proposal.title)
      );

      if (!matchingPlan) {
        // Truly new proposal — no plan with this title exists at all
        if (isBranchContext) { setStatus("promoted"); return; }
        setStatus("idle");
        return;
      }

      // Branch context 에서는 main chat 의 card 가 promote/overwrite 를 전담.
      if (isBranchContext) {
        setStatus("promoted");
        return;
      }

      // done/abandoned plan — "등록됨" 표시는 유지하되, 제안 subtasks 가 기존과 다르면
      // **완료된 Plan 재개** 용 overwrite 후보로 세팅. 사용자가 버튼 눌러야 재개.
      if (matchingPlan.status === "done" || matchingPlan.status === "abandoned") {
        const existingSubtasks = await planApi.listSubtasks(matchingPlan.id);
        const proposalKey = proposal.subtasks.map((s) => s.title.toLowerCase()).join("|");
        const existingKey = existingSubtasks.map((s) => s.title.toLowerCase()).join("|");
        if (proposal.subtasks.length > 0 && proposalKey !== existingKey) {
          setOverwriteCandidate(matchingPlan);
        }
        setStatus("promoted");
        return;
      }

      // Active matching plan — check for a pending revision to auto-merge
      const events = await planApi.listPlanEvents(matchingPlan.id);
      const lastEvent = events[events.length - 1];

      const lastIsRevisionRequest = lastEvent && (
        lastEvent.eventType === "revision_requested" ||
        lastEvent.eventType === "plan_full_revision_requested" ||
        lastEvent.eventType === "architect_redesign_requested" ||
        lastEvent.eventType === "detail_design_requested"
      );

      if (lastIsRevisionRequest) {
        // Guard: if this proposal's subtasks are identical to current DB subtasks,
        // this is a stale/already-merged card — don't auto-merge again
        const existingSubtasks = await planApi.listSubtasks(matchingPlan.id);
        const proposalKey = proposal.subtasks.map((s) => s.title.toLowerCase()).join("|");
        const existingKey = existingSubtasks.map((s) => s.title.toLowerCase()).join("|");
        if (proposalKey === existingKey && existingSubtasks.length > 0) {
          setStatus("promoted");
          return;
        }

        const isFullRevision = lastEvent.eventType === "plan_full_revision_requested" || lastEvent.eventType === "architect_redesign_requested";
        const isDetailUpdate = lastEvent.eventType === "detail_design_requested";
        setRevisionTarget(matchingPlan);
        await autoMerge(matchingPlan, isFullRevision, isDetailUpdate ? "subtask_review" : undefined);
        return;
      }

      // Active plan, no pending revision → already promoted.
      // b 정책: 기존 subtasks 와 제안 subtasks 가 다르면 사용자가 수동 overwrite 할 수 있게
      // `overwriteCandidate` 세팅. 동일하면 이미 반영된 상태라 버튼 숨김.
      const existingSubtasks = await planApi.listSubtasks(matchingPlan.id);
      const proposalKey = proposal.subtasks.map((s) => s.title.toLowerCase()).join("|");
      const existingKey = existingSubtasks.map((s) => s.title.toLowerCase()).join("|");
      if (proposal.subtasks.length > 0 && proposalKey !== existingKey) {
        setOverwriteCandidate(matchingPlan);
      }
      setStatus("promoted");
    }).catch(() => setStatus("idle"));
  }, [conversationId]);

  const handleOverwrite = async () => {
    if (!overwriteCandidate) return;
    const isResumingDone = overwriteCandidate.status === "done" || overwriteCandidate.status === "abandoned";
    // 명시적 사용자 confirm — 기존 subtasks/메타가 교체되고 branches 가 archive 됨을 경고.
    const confirmMsg = isResumingDone
      ? `완료된 Plan "${overwriteCandidate.title}" 을 이 제안으로 재개하시겠습니까?\n\n` +
        `- 이미 ${overwriteCandidate.status === "done" ? "완료" : "폐기"} 된 Plan 을 active 로 되돌려 Dev 단계로 진입시킵니다\n` +
        `- 기존 subtasks 는 모두 제안 내용으로 교체됩니다\n` +
        `- 이미 archive 된 impl/review branches 는 그대로 (재사용하지 않음 — 신규 Dev 시작 시 새 브랜치)\n` +
        `- 이미 수정된 파일은 자동 revert 되지 않습니다`
      : `기존 Plan "${overwriteCandidate.title}" 을 이 제안으로 덮어쓰시겠습니까?\n\n` +
        `- 기존 subtasks 는 모두 교체됩니다\n` +
        `- 진행 중인 implementation/review branches 는 archive 됩니다\n` +
        `- Phase 가 Approval 로 리셋되어 Dev 시작 가능 상태가 됩니다\n` +
        `- 이미 수정된 파일은 자동 revert 되지 않습니다 (필요 시 수동 처리)`;
    const ok = window.confirm(confirmMsg);
    if (!ok) return;

    setStatus("promoting");
    try {
      // done/abandoned → active 로 복원해야 워크플로우 탭에 다시 노출됨
      if (isResumingDone) {
        await planApi.updatePlanStatus(overwriteCandidate.id, "active");
      }
      // 1) meta (title/description/expectedOutcome) 업데이트
      await planApi.updatePlanMeta(overwriteCandidate.id, {
        title: proposal.title || null,
        description: proposal.description || null,
        expectedOutcome: proposal.expectedOutcome || null,
      });
      // 2) subtasks 전량 교체
      await planApi.replacePlanSubtasks(overwriteCandidate.id, proposal.subtasks.map((s) => ({
        title: s.title, details: s.details,
      })));
      // 3) major version bump + 이벤트 기록
      await planApi.bumpPlanMajorVersion(overwriteCandidate.id);
      await planApi.createPlanEvent(overwriteCandidate.id, "plan_full_revision_requested", "user",
        "Manual overwrite via PlanProposalCard (b policy)");
      await planApi.createPlanEvent(overwriteCandidate.id, "review_merged", "system",
        "Plan overwritten with revision proposal");
      // 4) impl/review branch archive
      if (overwriteCandidate.implementationBranchId) {
        await invoke("archive_branch", { id: overwriteCandidate.implementationBranchId }).catch(() => {});
        await planApi.linkPlanBranch(overwriteCandidate.id, "implementation", null);
      }
      if (overwriteCandidate.reviewBranchId) {
        await invoke("archive_branch", { id: overwriteCandidate.reviewBranchId }).catch(() => {});
        await planApi.linkPlanBranch(overwriteCandidate.id, "review", null);
      }
      closeThread();
      await loadBranches(overwriteCandidate.conversationId);
      // 5) phase = approval (b 정책: Dev 시작 바로 가능)
      await planApi.updatePlanPhase(overwriteCandidate.id, "approval");
      syncPlanDocument(overwriteCandidate.id);
      // 워크플로우 탭 전환
      window.dispatchEvent(new CustomEvent("tunaflow:switch-tab", { detail: "workflow" }));
      window.dispatchEvent(new CustomEvent("tunaflow:switch-stage", { detail: "plan-check" }));
      setRevisionTarget(overwriteCandidate);
      setStatus("merged");

      // PR-3: subtask 에 File disposition 이 명시돼 있으면 사용자에게 요약 토스트.
      // 자동 revert 는 하지 않음(위험). 안내만 — 사용자가 IDE/git 으로 직접 처리.
      const disp = mergeDispositions(proposal.subtasks);
      const totalFiles = disp.keep.length + disp.modify.length + disp.revert.length;
      if (totalFiles > 0) {
        const { toast } = await import("sonner");
        const summary = [
          disp.keep.length > 0 ? `Keep ${disp.keep.length}` : null,
          disp.modify.length > 0 ? `Modify ${disp.modify.length}` : null,
          disp.revert.length > 0 ? `Revert ${disp.revert.length}` : null,
        ].filter(Boolean).join(" · ");
        if (disp.revert.length > 0) {
          toast.warning(
            `파일 처리 방침: ${summary}\n\nRevert 대상 (수동 처리 필요):\n${disp.revert.slice(0, 6).map((f) => `• ${f}`).join("\n")}${disp.revert.length > 6 ? `\n… +${disp.revert.length - 6}` : ""}`,
            { duration: 12000 },
          );
        } else {
          toast.info(`파일 처리 방침: ${summary}`, { duration: 5000 });
        }
      }
    } catch (e) {
      console.warn("[overwrite] failed:", e);
      setStatus("idle");
    }
  };

  const autoMerge = async (targetPlan: Plan, isMajorRevision: boolean = false, overrideNextPhase?: string) => {
    setStatus("promoting");
    try {
      const existingSubtasks = await planApi.listSubtasks(targetPlan.id);

      // If proposal has no subtasks but existing plan does → preserve existing subtasks,
      // just advance the phase. (Architect wrote docs but didn't list subtasks in marker)
      if (proposal.subtasks.length === 0 && existingSubtasks.length > 0) {
        const nextPhase = overrideNextPhase ?? (isMajorRevision ? "subtask_review" : "approval");
        await planApi.updatePlanPhase(targetPlan.id, nextPhase as import("@/types").PlanPhase);
        await planApi.createPlanEvent(targetPlan.id, "review_merged", "system", "Phase advanced (no subtask changes)");
        syncPlanDocument(targetPlan.id);
        setStatus("merged");
        return;
      }

      // Safety check: don't replace if proposal has far fewer subtasks than existing
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
        await invoke("archive_branch", { id: targetPlan.implementationBranchId }).catch((e) => console.debug("[archive]", e));
        await planApi.linkPlanBranch(targetPlan.id, "implementation", null);
        closeThread();
        await loadBranches(targetPlan.conversationId);
      }

      // Phase transitions:
      // - major redesign → subtask_review (re-confirm before approval)
      // - detail_design update → subtask_review (review updated details)
      // - minor revision from implementation → approval (can proceed immediately)
      const nextPhase = overrideNextPhase ?? (isMajorRevision ? "subtask_review" : "approval");
      await planApi.updatePlanPhase(targetPlan.id, nextPhase as import("@/types").PlanPhase);
      setStatus("merged");
    } catch {
      setStatus("idle"); // Fallback to manual UI
    }
  };

  const handlePromote = async (force = false) => {
    if (!force && proposal.subtasks.length === 0) {
      setStatus("warn-empty");
      return;
    }
    setStatus("promoting");
    try {
      const plan = await planApi.createPlan({
        conversationId: planConvId,
        branchId: activeBranchId ?? undefined,
        title: proposal.title,
        description: proposal.description || undefined,
        expectedOutcome: proposal.expectedOutcome || undefined,
        subtasks: proposal.subtasks.map((s) => ({ title: s.title, details: s.details })),
      });
      // Plan starts at drafting — user must go through Plan → Subtask → Approved
      await planApi.createPlanEvent(plan.id, "promoted", "user", "Promoted from chat");

      // Auto-request Architect to write plan documents
      const slug = slugifyPlanTitle(proposal.title);
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
        `- **Changed files** — 대상 파일 경로 (가능하면 줄번호까지)`,
        `- **Change description** — 구현 접근법 (단계별)`,
        `- **Dependencies** — 의존성 (패키지, 다른 subtask)`,
        `- **Verification** — 완료를 증명하는 **실행 가능한 셸 명령** (예: \`cargo test\`, \`npx tsc --noEmit\`)`,
        `- **Risks** — 리스크 및 주의사항`,
        `- **Scope boundary** — 수정 금지 파일 목록 (다른 task 영역 침범 방지)`,
        ``,
        `> 완료 조건: 모든 문서 작성 후 알려주세요`,
      ].join("\n");

      await sendWithEngine(
        useChatStore.getState().getConversationEngine(conversationId)?.engine ?? "claude",
        docPrompt,
      );
      // Only advance to subtask_review if subtasks were included in the proposal.
      // If promoted with 0 subtasks (force=true), stay in drafting so docs can be
      // written first, then user recovers subtasks via "docs에서 동기화".
      if (proposal.subtasks.length > 0) {
        await planApi.updatePlanPhase(plan.id, "subtask_review");
        await planApi.createPlanEvent(plan.id, "detail_design_requested", "system", "Auto-requested on promotion");
      } else {
        await planApi.createPlanEvent(plan.id, "detail_design_requested", "system", "Promoted without subtasks — awaiting doc sync");
      }

      // Auto-adopt branch if promoted from a branch context
      // activeBranchId = full-screen branch mode; threadBranchId = drawer mode
      const adoptId = activeBranchId ?? threadBranchId;
      const adoptConvId = activeBranchId ? planConvId : threadBranchConvId;
      if (adoptId && adoptConvId) {
        await adoptBranch(adoptId, adoptConvId).catch((e) =>
          console.warn("[promote] adopt branch failed:", e)
        );
      }

      // Switch Workflow tab to plan-check stage so newly promoted plan is visible
      window.dispatchEvent(new CustomEvent("tunaflow:switch-tab", { detail: "workflow" }));
      window.dispatchEvent(new CustomEvent("tunaflow:switch-stage", { detail: "plan-check" }));

      setStatus("promoted");
    } catch {
      setStatus("idle");
    }
  };

  // ─── Status-based renders ──────────────────────────────────────────────────

  if (status === "loading" || status === "dismissed") return null;

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
    const isDone = overwriteCandidate?.status === "done" || overwriteCandidate?.status === "abandoned";
    const locationHint = isDone ? " (워크플로우 탭 → Done)" : "";
    return (
      <div className="my-2 rounded-lg border border-status-approved/30 bg-status-approved/5 px-4 py-2.5 text-xs text-status-approved flex items-center gap-2 flex-wrap">
        <Check className="w-3.5 h-3.5 shrink-0" />
        <span className="flex-1 min-w-0">Plan &quot;{proposal.title}&quot; — Plan 탭에 등록됨{locationHint}</span>
        {overwriteCandidate && (
          <button
            onClick={handleOverwrite}
            className="shrink-0 px-2 py-0.5 rounded text-[11px] font-medium bg-primary/10 text-primary hover:bg-primary/20 transition-colors"
            title={
              isDone
                ? "완료된 Plan 을 이 제안으로 재개 (active + Approval)"
                : "이 제안으로 기존 Plan 덮어쓰기 (Approval 로 리셋 + branches archive)"
            }
          >
            {isDone ? "완료 Plan 재개" : "rev 로 덮어쓰기"}
          </button>
        )}
      </div>
    );
  }

  if (status === "warn-empty") {
    return (
      <div className="my-2 rounded-lg border border-amber-500/30 bg-amber-500/5 px-4 py-3 text-xs space-y-2">
        <p className="text-amber-400 font-medium">서브태스크를 인식하지 못했습니다</p>
        <p className="text-muted-foreground text-[11px]">
          에이전트가 서브태스크를 잘못된 형식으로 작성했을 수 있습니다.
          파서가 인식하는 형식:
        </p>
        <pre className="text-[10px] text-muted-foreground/70 bg-black/20 rounded px-2 py-1.5 font-mono leading-relaxed">
{`### Subtasks        ← 삼중 # 필수
1. 첫 번째 작업 — 설명
2. 두 번째 작업 — 설명`}
        </pre>
        <p className="text-muted-foreground/60 text-[10px]">
          ❌ <code className="font-mono">## Subtasks</code> (이중 #) &nbsp;·&nbsp;
          ❌ 마크다운 테이블 <code className="font-mono">| # | 제목 |</code> &nbsp;·&nbsp;
          ❌ 마커 밖 작성
        </p>
        <div className="flex gap-1.5 pt-0.5">
          <button
            onClick={() => setStatus("revising")}
            className="px-2.5 py-1 rounded-md text-xs font-medium bg-primary/10 text-primary hover:bg-primary/20 transition-colors"
          >
            수정 요청
          </button>
          <button
            onClick={() => { void handlePromote(true); }}
            className="px-2.5 py-1 rounded-md text-xs font-medium bg-amber-500/15 text-amber-400 hover:bg-amber-500/25 transition-colors"
          >
            그래도 승격
          </button>
          <button
            onClick={() => setStatus("idle")}
            className="px-2.5 py-1 rounded-md text-xs text-muted-foreground hover:text-foreground transition-colors"
          >
            취소
          </button>
        </div>
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
            onClick={() => { void handlePromote(); }}
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
          <button
            onClick={() => setStatus("dismissed")}
            className="ml-auto flex items-center gap-1 px-2 py-1 rounded-md text-xs text-muted-foreground/50 hover:text-muted-foreground hover:bg-accent/20 transition-colors"
            title="이 제안 닫기"
          >
            <X className="w-3 h-3" />
            닫기
          </button>
        </div>
      )}
    </div>
  );
}
