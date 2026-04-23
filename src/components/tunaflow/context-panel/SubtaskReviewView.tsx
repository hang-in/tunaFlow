import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { Check, RotateCcw, ClipboardList, FileText, ArrowLeft, ChevronDown, ChevronRight, PenLine } from "lucide-react";
import type { Plan, PlanPhase, PlanSubtask, Branch } from "@/types";
import * as planApi from "@/lib/api/plans";
import { getPlanSlug, syncPlanDocument } from "@/lib/workflowOrchestration";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { markdownComponents } from "../chat/MarkdownComponents";
import { PlanDocumentModal } from "./PlanDocumentModal";
import { SUBTASK_STATUS_CFG } from "./plans/constants";

/** Extract title from markdown file content (first # heading) */
function extractTitleFromMd(content: string): string | null {
  const match = content.match(/^#\s+(.+)$/m);
  return match ? match[1].trim() : null;
}

interface SubtaskReviewViewProps {
  plan: Plan;
  onPlanUpdate: (planId: string, update: Partial<Plan>) => void;
  onSwitchToChat?: () => void;
}

export function SubtaskReviewView({ plan, onPlanUpdate, onSwitchToChat }: SubtaskReviewViewProps) {
  const { t } = useTranslation("workflow");
  const { sendWithEngine, selectedConversationId, getConversationEngine,
    createBranch, openThread, sendThreadMessage, saveConversationEngine, loadBranches } = useChatStore();
  const [subtasks, setSubtasks] = useState<PlanSubtask[]>([]);
  const [taskFiles, setTaskFiles] = useState<Record<number, string>>({});
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [showDoc, setShowDoc] = useState(false);
  const [reviewHistory, setReviewHistory] = useState<{ round: number; findings: string[]; failedIds: number[] }[]>([]);
  const [isDoomLoop, setIsDoomLoop] = useState(false);

  useEffect(() => {
    setLoading(true);
    planApi.listSubtasks(plan.id)
      .then(setSubtasks)
      .catch(() => setSubtasks([]))
      .finally(() => setLoading(false));

    // Load review failure history for doom loop context
    planApi.listPlanEvents(plan.id).then((events) => {
      const failEvents = events.filter((e) => e.eventType === "review_failed");
      setIsDoomLoop(events.some((e) => e.eventType === "doom_loop_escalated"));
      const history = failEvents.map((ev, i) => {
        try {
          const d = JSON.parse(ev.detail ?? "{}");
          return {
            round: i + 1,
            findings: (d.findings as string[] ?? []).slice(0, 5),
            failedIds: (d.failedSubtaskIds as number[] ?? []),
          };
        } catch { return { round: i + 1, findings: [], failedIds: [] }; }
      });
      setReviewHistory(history);
    }).catch((e) => console.warn("[subtask-review]", e));

    // Load task files from filesystem
    (async () => {
      try {
        const projectKey = useChatStore.getState().selectedProjectKey;
        if (!projectKey) return;
        const project = await invoke("get_project", { key: projectKey }) as { path?: string };
        if (!project?.path) return;
        const slug = getPlanSlug(plan);
        const files: Record<number, string> = {};
        for (let i = 1; i <= 50; i++) {
          const taskPath = `${project.path}/docs/plans/${slug}-task-${String(i).padStart(2, "0")}.md`;
          try {
            const content = await invoke<{ content: string }>("read_text_file", { filePath: taskPath, projectPath: project.path });
            files[i] = content.content;
          } catch { break; }
        }
        setTaskFiles(files);
      } catch (e) { console.warn("[tunaflow]", e); }
    })();
  }, [plan.id, plan.revision]);

  const isActionable = plan.phase === "subtask_review";

  // Gating (s37) — 승인 버튼은 아래 두 조건 모두 만족해야 활성:
  //   (A) 메인 chat 에서 Architect 가 응답 생성 중이 아님
  //   (B) 모든 subtask 에 details 가 채워져 있음
  // "Plan 문서 반영" 은 (A) 만 검사 (subtask 수정 후 반영 목적이라 details 조건 불필요).
  const runningThreadIds = useChatStore((s) => s.runningThreadIds);
  const mainConvBusy = !!selectedConversationId && runningThreadIds.includes(selectedConversationId);
  const missingDetailsCount = subtasks.filter((s) => !s.details?.trim()).length;
  const approveDisabledReason: string | null = busy
    ? null  // 로컬 busy 는 disable 이지만 별도 안내 불필요
    : mainConvBusy
      ? t("subtask_review.actions.approve_tooltip_busy")
      : missingDetailsCount > 0
        ? t("subtask_review.actions.approve_tooltip_missing", { count: missingDetailsCount })
        : null;
  const approveBlocked = busy || mainConvBusy || missingDetailsCount > 0;
  const syncBlocked = busy || mainConvBusy;

  // Resolve main chat's agent engine + model (not hardcoded "claude")
  const mainSaved = selectedConversationId ? getConversationEngine(selectedConversationId) : null;
  const mainEngine = mainSaved?.engine ?? "claude";
  const mainModel = mainSaved?.model;

  const handleApprove = async () => {
    if (!isActionable) return;
    setBusy(true);
    try {
      await planApi.updatePlanPhase(plan.id, "approval");
      await planApi.createPlanEvent(plan.id, "subtask_review_completed", "user");
      syncPlanDocument(plan.id);
      onPlanUpdate(plan.id, { phase: "approval" as PlanPhase });
    } catch (e) { console.warn("[tunaflow]", e); }
    setBusy(false);
  };

  const handleSyncToMainPlan = async () => {
    if (!isActionable) return;
    setBusy(true);
    try {
      const slug = getPlanSlug(plan);

      // Create a branch for plan document sync
      const branchLabel = `Plan 문서 반영: ${plan.title.slice(0, 20)}`;
      const input = { conversationId: plan.conversationId, label: branchLabel, mode: "chat" };
      const branch = await invoke<Branch>("create_branch", { input });
      const shadowConvId = await invoke<string>("open_branch_stream", { branchId: branch.id });
      saveConversationEngine(shadowConvId, { profileId: null, engine: mainEngine, model: mainModel });
      await loadBranches(plan.conversationId);
      await openThread(branch.id);

      const prompt = [
        `### 📌 Plan 문서 반영`,
        ``,
        `**Plan**: "${plan.title}"`,
        `- 메인: \`docs/plans/${slug}.md\``,
        `- 지시서: \`docs/plans/${slug}-task-*.md\``,
        ``,
        `수정된 작업 지시서의 내용을 메인 문서의 subtask 요약에 반영하세요.`,
        ``,
        `> 완료 조건: 변경 내용 요약`,
      ].join("\n");

      await sendThreadMessage(prompt, mainEngine, mainModel);
      await planApi.createPlanEvent(plan.id, "plan_sync_requested", "user");
    } catch (e) { console.warn("[tunaflow]", e); }
    setBusy(false);
  };

  const handleRevisionRequest = async (subtaskIdx: number, opinion: string) => {
    if (!isActionable) return;
    setBusy(true);
    try {
      const st = subtasks[subtaskIdx];

      // Create a branch for this subtask revision discussion
      const branchLabel = `Subtask ${subtaskIdx + 1}: ${st.title.slice(0, 30)}`;
      const input = { conversationId: plan.conversationId, label: branchLabel, mode: "chat" };
      const branch = await invoke<Branch>("create_branch", { input });
      const shadowConvId = await invoke<string>("open_branch_stream", { branchId: branch.id });

      // Save same agent as main chat
      saveConversationEngine(shadowConvId, { profileId: null, engine: mainEngine, model: mainModel });

      await loadBranches(plan.conversationId);
      await openThread(branch.id);

      // Send revision prompt with review failure context
      const slug = getPlanSlug(plan);
      const taskFile = `docs/plans/${slug}-task-${String(subtaskIdx + 1).padStart(2, "0")}.md`;

      // Build failure history context for the Architect
      const failHistoryBlock = reviewHistory.length > 0 ? [
        ``,
        `**Review 실패 이력** (${reviewHistory.length}회):`,
        ...reviewHistory.slice(-3).map((h) => {
          const targetNote = h.failedIds.length > 0 ? ` (Task ${h.failedIds.join(",")})` : "";
          const topFindings = h.findings.slice(0, 2).map((f) => f.slice(0, 150)).join("; ");
          return `- ${h.round}차${targetNote}: ${topFindings || "상세 없음"}`;
        }),
        ``,
        `> 위 findings가 반복되는 이유를 분석하고, 작업 지시서의 **Verification 명령**과 **Changed files**를 구체화하세요.`,
      ] : [];

      const prompt = [
        `### ✏️ Subtask 수정 요청`,
        ``,
        `**Subtask ${subtaskIdx + 1}**: "${st.title}"`,
        `- 파일: \`${taskFile}\``,
        ``,
        `**검토 의견**:`,
        `${opinion}`,
        ...failHistoryBlock,
        ``,
        `> 완료 조건: 파일 수정 후 변경 내용 요약`,
      ].join("\n");

      await sendThreadMessage(prompt, mainEngine, mainModel);
      await planApi.createPlanEvent(plan.id, "subtask_revision_requested", "user",
        `subtask ${subtaskIdx + 1}: ${opinion.slice(0, 100)}`);
    } catch (e) { console.warn("[tunaflow]", e); }
    setBusy(false);
  };

  const handleDiscuss = async (subtaskIdx: number) => {
    if (!isActionable) return;
    setBusy(true);
    try {
      const st = subtasks[subtaskIdx];
      const branchLabel = `검토: ${st.title.slice(0, 30)}`;
      const input = { conversationId: plan.conversationId, label: branchLabel, mode: "chat" };
      const branch = await invoke<Branch>("create_branch", { input });
      const shadowConvId = await invoke<string>("open_branch_stream", { branchId: branch.id });
      saveConversationEngine(shadowConvId, { profileId: null, engine: mainEngine, model: mainModel });
      await loadBranches(plan.conversationId);
      await openThread(branch.id);

      const slug = getPlanSlug(plan);
      const taskFile = `docs/plans/${slug}-task-${String(subtaskIdx + 1).padStart(2, "0")}.md`;
      const prompt = [
        `### 💬 Subtask 논의`,
        ``,
        `**Subtask ${subtaskIdx + 1}**: "${st.title}"`,
        `- 파일: \`${taskFile}\``,
        ``,
        `이 subtask에 대해 논의합니다. 질문하거나 검토 의견을 나눠주세요.`,
      ].join("\n");

      await sendThreadMessage(prompt, mainEngine, mainModel);
    } catch (e) { console.warn("[tunaflow]", e); }
    setBusy(false);
  };

  const handleDetailRequest = async (subtaskIdx: number) => {
    if (!isActionable) return;
    setBusy(true);
    try {
      const st = subtasks[subtaskIdx];

      // Create a branch for this subtask detail writing
      const branchLabel = `작업지시 작성: ${st.title.slice(0, 30)}`;
      const input = { conversationId: plan.conversationId, label: branchLabel, mode: "chat" };
      const branch = await invoke<Branch>("create_branch", { input });
      const shadowConvId = await invoke<string>("open_branch_stream", { branchId: branch.id });

      saveConversationEngine(shadowConvId, { profileId: null, engine: mainEngine, model: mainModel });

      await loadBranches(plan.conversationId);
      await openThread(branch.id);

      const slug = getPlanSlug(plan);
      const taskFile = `docs/plans/${slug}-task-${String(subtaskIdx + 1).padStart(2, "0")}.md`;
      const prompt = [
        `### 📝 작업 지시서 작성`,
        ``,
        `**Subtask ${subtaskIdx + 1}**: "${st.title}"`,
        `- 파일: \`${taskFile}\``,
        ``,
        `**포함 내용**:`,
        `- 대상 파일 및 경로`,
        `- 구현 접근법 (단계별)`,
        `- 의존성 (패키지, 다른 subtask)`,
        `- 리스크 및 주의사항`,
        `- 완료 기준`,
        ``,
        `> 완료 조건: 파일 작성 후 알려주세요`,
      ].join("\n");

      await sendThreadMessage(prompt, mainEngine, mainModel);
      await planApi.createPlanEvent(plan.id, "detail_design_requested", "user",
        `subtask ${subtaskIdx + 1}`);
    } catch (e) { console.warn("[tunaflow]", e); }
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
          {(plan.versionMajor > 1 || plan.versionMinor > 0) && (
            <span className="text-[8px] font-mono text-muted-foreground/50 px-1 rounded bg-accent/50">v{plan.versionMajor}.{plan.versionMinor}</span>
          )}
          <button onClick={() => setShowDoc(true)} className="flex items-center gap-1 text-[9px] text-primary/60 hover:text-primary transition-colors">
            <FileText className="w-3 h-3" />{t("subtask_review.header.view_doc_button")}
          </button>
        </div>
        {plan.description && (
          <p className="text-[11px] text-muted-foreground leading-snug mb-1">{plan.description}</p>
        )}
        {plan.expectedOutcome && (
          <p className="text-[10px] text-muted-foreground/60 italic">{t("subtask_review.header.goal_prefix", { outcome: plan.expectedOutcome })}</p>
        )}
      </div>

      {/* Doom loop context — why we're here */}
      {isDoomLoop && reviewHistory.length > 0 && (
        <div className="rounded-md border border-amber-500/30 bg-amber-500/5 p-3 space-y-2">
          <p className="text-[11px] font-semibold text-amber-600">
            {reviewHistory.length === 1
              ? t("subtask_review.doom_loop.title_one_fail")
              : t("subtask_review.doom_loop.title_many_fails", { count: reviewHistory.length })}
          </p>
          <p className="text-[10px] text-foreground/60">
            {reviewHistory.length === 1
              ? t("subtask_review.doom_loop.body_one_fail")
              : t("subtask_review.doom_loop.body_many_fails")}
          </p>
          <details className="text-[9px]">
            <summary className="cursor-pointer text-muted-foreground/60 hover:text-foreground">
              {t("subtask_review.doom_loop.history_summary", { count: reviewHistory.length })}
            </summary>
            <div className="mt-1.5 space-y-1.5 pl-2">
              {reviewHistory.map((h) => (
                <div key={h.round} className="border-l-2 border-amber-500/20 pl-2">
                  <span className="font-medium text-foreground/50">{t("subtask_review.doom_loop.round_fail", { round: h.round })}</span>
                  {h.failedIds.length > 0 && (
                    <span className="text-muted-foreground/40 ml-1">
                      (Task {h.failedIds.join(", ")})
                    </span>
                  )}
                  {h.findings.length > 0 && (
                    <ul className="mt-0.5 text-muted-foreground/50">
                      {h.findings.map((f, i) => (
                        <li key={i} className="truncate">- {f.slice(0, 120)}</li>
                      ))}
                    </ul>
                  )}
                </div>
              ))}
            </div>
          </details>
        </div>
      )}

      {/* Subtask review list — clickable cards */}
      <div className="space-y-1.5">
        {subtasks.map((st, i) => (
          <SubtaskReviewCard
            key={st.id}
            subtask={st}
            index={i}
            taskFileContent={taskFiles[i + 1]}
            onRevisionRequest={(opinion) => handleRevisionRequest(i, opinion)}
            onDetailRequest={() => handleDetailRequest(i)}
            onDiscuss={() => handleDiscuss(i)}
            busy={busy}
            actionable={isActionable}
          />
        ))}
      </div>

      {/* Actions */}
      {isActionable && (
        <div className="flex items-center gap-2 pt-2 border-t border-border/30 flex-wrap">
          {/* Doom loop: Architect redesign request (primary action) */}
          {isDoomLoop && (
            <button
              onClick={async () => {
                setBusy(true);
                try {
                  const failHistoryText = reviewHistory.slice(-3).map((h) => {
                    const targetNote = h.failedIds.length > 0 ? ` (Task ${h.failedIds.join(",")})` : "";
                    const topFindings = h.findings.slice(0, 3).map((f) => f.slice(0, 200)).join("\n  - ");
                    return `- ${h.round}차 fail${targetNote}:\n  - ${topFindings || "상세 없음"}`;
                  }).join("\n");
                  const list = subtasks.map((s, i) =>
                    `${i + 1}. ${s.title}${s.details ? ` — ${s.details.slice(0, 100)}` : ""}`
                  ).join("\n");
                  const prompt = [
                    `[설계 재검토 요청] "${plan.title}"`,
                    "",
                    reviewHistory.length === 1
                      ? `이 Plan 은 **Review 1회 실패** + 이전 싸이클 재검토 이력으로 설계 재검토가 필요합니다.`
                      : `이 Plan은 **Review ${reviewHistory.length}회 연속 실패**로 설계 재검토가 필요합니다.`,
                    "",
                    `## 실패 이력`,
                    failHistoryText,
                    "",
                    `## 현재 Subtasks`,
                    list,
                    "",
                    `## 요청`,
                    `위 실패 이력을 분석하고, **작업 지시서의 Verification 명령과 Changed files를 구체화**하여`,
                    `수정된 Plan을 \`<!-- tunaflow:plan-proposal -->\` 형식으로 제안하세요.`,
                    `특히 반복 실패한 subtask의 범위와 검증 방법을 재설계하세요.`,
                  ].join("\n");
                  await sendWithEngine(mainEngine, prompt, mainModel);
                  await planApi.updatePlanPhase(plan.id, "drafting");
                  await planApi.createPlanEvent(plan.id, "architect_redesign_requested", "user",
                    `doom loop ${reviewHistory.length} failures`);
                  // Baton moved to Architect — review branch is no longer active.
                  const { archiveReviewBranchForHandoff } = await import("@/lib/workflowOrchestration");
                  await archiveReviewBranchForHandoff(plan);
                  const { dispatchMetaNotification } = await import("@/lib/metaNotifications");
                  const redesignProjectKey = useChatStore.getState().selectedProjectKey ?? undefined;
                  dispatchMetaNotification({
                    kind: "architect_redesign_requested",
                    title: `🔄 Plan "${plan.title}" Architect 재설계 시작`,
                    summary: `사용자 요청으로 재설계 사이클 진입 (${reviewHistory.length}회 실패 후).`,
                    projectKey: redesignProjectKey,
                    route: { tab: "workflow", stage: "plan-check", planId: plan.id },
                  });
                  onPlanUpdate(plan.id, { phase: "drafting" as PlanPhase });
                  onSwitchToChat?.();
                } catch (e) { console.warn("[tunaflow]", e); }
                setBusy(false);
              }}
              disabled={busy}
              className="flex items-center gap-1 px-3 py-1.5 rounded-md text-xs font-medium bg-amber-500/15 text-amber-600 hover:bg-amber-500/25 disabled:opacity-50 transition-colors"
            >
              <ArrowLeft className="w-3.5 h-3.5" />{t("subtask_review.actions.redesign_button")}
            </button>
          )}
          <button onClick={handleApprove} disabled={approveBlocked}
            className={cn(
              "flex items-center gap-1 px-3 py-1.5 rounded-md text-xs font-medium disabled:opacity-50 transition-colors",
              isDoomLoop
                ? "bg-muted text-muted-foreground/50 hover:text-muted-foreground"
                : "bg-status-approved/10 text-status-approved hover:bg-status-approved/20"
            )}
            title={
              isDoomLoop ? t("subtask_review.actions.approve_tooltip_doom")
              : approveDisabledReason ?? undefined
            }
          >
            <Check className="w-3.5 h-3.5" />
            {isDoomLoop ? t("subtask_review.actions.approve_doom_loop") : t("subtask_review.actions.approve_normal")}
            {/* 상태 뱃지 — 왜 비활성인지 한 눈에 보이도록 */}
            {mainConvBusy && (
              <span className="ml-1 text-[9px] px-1 rounded bg-amber-500/15 text-amber-600">{t("subtask_review.actions.badge_locked")}</span>
            )}
            {!mainConvBusy && missingDetailsCount > 0 && (
              <span className="ml-1 text-[9px] px-1 rounded bg-muted text-muted-foreground/70">{t("subtask_review.actions.badge_missing_details", { count: missingDetailsCount })}</span>
            )}
          </button>
          {!isDoomLoop && (
            /* Plan 문서 반영 — 일반 flow 에서 거의 안 쓰는 부가 기능.
               아이콘 전용 버튼으로 축소해 승인 버튼과의 시각적 경쟁 제거. (s37) */
            <button
              onClick={handleSyncToMainPlan}
              disabled={syncBlocked}
              className="flex items-center justify-center w-8 h-8 rounded-md text-muted-foreground/40 hover:text-foreground hover:bg-accent disabled:opacity-40 transition-colors"
              title={
                mainConvBusy
                  ? t("subtask_review.actions.sync_doc_tooltip_busy")
                  : t("subtask_review.actions.sync_doc_tooltip_normal")
              }
              aria-label={t("subtask_review.actions.sync_doc_aria")}
            >
              <FileText className="w-3.5 h-3.5" />
            </button>
          )}
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
                  await sendWithEngine(mainEngine, prompt, mainModel);
                  await planApi.createPlanEvent(plan.id, "detail_design_requested", "user", "all subtasks");
                  onSwitchToChat?.();
                } catch (e) { console.warn("[tunaflow]", e); }
                setBusy(false);
              }}
              disabled={busy}
              className="flex items-center gap-1 px-2.5 py-1 rounded-md text-[10px] font-medium bg-muted text-muted-foreground/60 hover:text-muted-foreground border border-dashed border-border/40 disabled:opacity-40 transition-colors"
              title={t("subtask_review.actions.debug_all_tasks_tooltip")}
            >
              <PenLine className="w-3 h-3" />{t("subtask_review.actions.debug_all_tasks_button")}
            </button>
          )}
        </div>
      )}

      {!isActionable && (
        <p className="text-[10px] text-muted-foreground/40 italic pt-2 border-t border-border/30">
          {t("subtask_review.actions.read_only_hint", { phase: plan.phase })}
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
  taskFileContent,
  onRevisionRequest,
  onDetailRequest,
  onDiscuss,
  busy,
  actionable,
}: {
  subtask: PlanSubtask;
  index: number;
  taskFileContent?: string;
  onRevisionRequest: (opinion: string) => void;
  onDetailRequest: () => void;
  onDiscuss: () => void;
  busy: boolean;
  actionable: boolean;
}) {
  const { t } = useTranslation("workflow");
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
            <span className="text-[11px] font-medium text-foreground">
              {(taskFileContent && extractTitleFromMd(taskFileContent)) || subtask.title}
            </span>
            <span className={cn("text-[8px] font-semibold px-1 py-0 rounded-full border shrink-0", statusCfg.cls)}>
              {statusCfg.label}
            </span>
            {!taskFileContent && !hasDetails && (
              <span className="text-[8px] text-amber-600/50 shrink-0">{t("subtask_review.card.status_file_missing")}</span>
            )}
            {taskFileContent && (
              <span className="text-[8px] text-status-approved/40 shrink-0">{t("subtask_review.card.status_file_exists")}</span>
            )}
          </div>
          {!expanded && taskFileContent && (
            <p className="text-[10px] text-muted-foreground/50 mt-0.5 line-clamp-1">
              {extractTitleFromMd(taskFileContent) ?? subtask.details ?? ""}
            </p>
          )}
          {!expanded && !taskFileContent && hasDetails && (
            <p className="text-[10px] text-muted-foreground/50 mt-0.5 line-clamp-1">{subtask.details}</p>
          )}
        </div>
      </button>

      {/* Expanded: full work instruction + actions */}
      {expanded && (
        <div className="px-2.5 pb-2.5 ml-9 space-y-2 border-t border-border/20 pt-2">
          {/* Work instruction — file content > DB details > empty */}
          {taskFileContent ? (
            <div>
              <div className="flex items-center gap-1 mb-1">
                <FileText className="w-3 h-3 text-primary/50" />
                <span className="text-[9px] text-muted-foreground/60 uppercase tracking-wide">{t("subtask_review.card.instructions_label")}</span>
                <span className="text-[8px] text-status-approved/50">{t("subtask_review.card.status_file_exists")}</span>
              </div>
              <div className="rounded bg-card/80 border border-border/30 px-3 py-2 prose prose-invert max-w-none text-[11px] leading-relaxed [&>*:first-child]:mt-0 [&>*:last-child]:mb-0 [&_h1]:text-[13px] [&_h2]:text-[12px] [&_h3]:text-[11px] [&_h1]:mt-3 [&_h2]:mt-2 [&_h3]:mt-1.5 [&_p]:my-1 [&_ul]:my-1 [&_li]:my-0.5 [&_code]:text-[10px]">
                <ReactMarkdown remarkPlugins={[[remarkGfm, { singleTilde: false }]]} components={markdownComponents}>
                  {taskFileContent}
                </ReactMarkdown>
              </div>
            </div>
          ) : hasDetails ? (
            <div>
              <div className="flex items-center gap-1 mb-1">
                <FileText className="w-3 h-3 text-primary/50" />
                <span className="text-[9px] text-muted-foreground/60 uppercase tracking-wide">{t("subtask_review.card.instructions_summary_label")}</span>
              </div>
              <div className="rounded bg-card/80 border border-border/30 px-3 py-2">
                <p className="text-[11px] text-foreground/80 leading-relaxed whitespace-pre-wrap">{subtask.details}</p>
              </div>
            </div>
          ) : (
            <div className="rounded bg-amber-500/5 border border-amber-500/15 px-3 py-2">
              <p className="text-[10px] text-amber-600/60 mb-1.5">{t("subtask_review.card.empty_instructions")}</p>
              {actionable && (
                <button onClick={(e) => { e.stopPropagation(); onDetailRequest(); }} disabled={busy}
                  className="flex items-center gap-1 px-2 py-0.5 rounded text-[9px] font-medium bg-amber-500/10 text-amber-600 hover:bg-amber-500/20 disabled:opacity-40 transition-colors">
                  <PenLine className="w-2.5 h-2.5" />{busy ? t("subtask_review.card.write_busy") : t("subtask_review.card.write_button")}
                </button>
              )}
            </div>
          )}

          {/* Metadata */}
          {subtask.ownerAgent && (
            <p className="text-[9px] text-muted-foreground/40">{t("subtask_review.card.owner_label", { owner: subtask.ownerAgent })}</p>
          )}

          {/* Actions: discuss + revision request */}
          {actionable && !opinionMode && (
            <div className="flex items-center gap-2 pt-1">
              <button onClick={(e) => { e.stopPropagation(); onDiscuss(); }} disabled={busy}
                className="flex items-center gap-0.5 text-[9px] text-primary/60 hover:text-primary disabled:opacity-40 transition-colors">
                <ChevronRight className="w-2.5 h-2.5" />{t("subtask_review.card.discuss_button")}
              </button>
              {hasDetails && (
                <button onClick={(e) => { e.stopPropagation(); setOpinionMode(true); }} disabled={busy}
                  className="flex items-center gap-0.5 text-[9px] text-amber-600/60 hover:text-amber-600 disabled:opacity-40 transition-colors">
                  <RotateCcw className="w-2.5 h-2.5" />{t("subtask_review.card.revise_button")}
                </button>
              )}
            </div>
          )}

          {opinionMode && (
            <div className="space-y-1.5 pt-1">
              <textarea
                value={opinion}
                onChange={(e) => setOpinion(e.target.value)}
                placeholder={t("subtask_review.card.opinion_placeholder")}
                rows={2}
                className="w-full bg-input rounded-md px-2 py-1.5 text-[10px] outline-none text-foreground placeholder:text-muted-foreground border border-border focus:border-ring/50 resize-none"
                autoFocus
                onClick={(e) => e.stopPropagation()}
              />
              <div className="flex gap-1.5">
                <button onClick={(e) => { e.stopPropagation(); handleSubmitOpinion(); }} disabled={!opinion.trim() || busy}
                  className="px-2 py-0.5 rounded text-[9px] font-medium bg-amber-500/10 text-amber-600 hover:bg-amber-500/20 disabled:opacity-40 transition-colors">
                  {t("subtask_review.card.send_revise")}
                </button>
                <button onClick={(e) => { e.stopPropagation(); setOpinionMode(false); setOpinion(""); }}
                  className="px-2 py-0.5 rounded text-[9px] text-muted-foreground hover:text-foreground transition-colors">
                  {t("subtask_review.card.cancel_button")}
                </button>
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
