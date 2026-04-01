import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { Check, RotateCcw, ClipboardList, FileText, ArrowLeft, ChevronDown, ChevronRight, PenLine } from "lucide-react";
import type { Plan, PlanPhase, PlanSubtask, Branch } from "@/types";
import * as planApi from "@/lib/api/plans";
import { syncPlanDocument } from "@/lib/workflowOrchestration";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { markdownComponents } from "../chat/MarkdownComponents";
import { PlanDocumentModal } from "./PlanDocumentModal";
import { SUBTASK_STATUS_CFG } from "./plans/constants";

interface SubtaskReviewViewProps {
  plan: Plan;
  onPlanUpdate: (planId: string, update: Partial<Plan>) => void;
  onSwitchToChat?: () => void;
}

export function SubtaskReviewView({ plan, onPlanUpdate, onSwitchToChat }: SubtaskReviewViewProps) {
  const { sendWithEngine, selectedConversationId, getConversationEngine,
    createBranch, openThread, sendThreadMessage, saveConversationEngine, loadBranches } = useChatStore();
  const [subtasks, setSubtasks] = useState<PlanSubtask[]>([]);
  const [taskFiles, setTaskFiles] = useState<Record<number, string>>({});
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [showDoc, setShowDoc] = useState(false);

  useEffect(() => {
    setLoading(true);
    planApi.listSubtasks(plan.id)
      .then(setSubtasks)
      .catch(() => setSubtasks([]))
      .finally(() => setLoading(false));

    // Load task files from filesystem
    (async () => {
      try {
        const projectKey = useChatStore.getState().selectedProjectKey;
        if (!projectKey) return;
        const project = await invoke("get_project", { key: projectKey }) as { path?: string };
        if (!project?.path) return;
        const slug = plan.title.replace(/[^\w가-힣-]/g, "-").replace(/-+/g, "-").replace(/^-|-$/g, "").toLowerCase().slice(0, 80);
        const files: Record<number, string> = {};
        for (let i = 1; i <= 50; i++) {
          const taskPath = `${project.path}/docs/plans/${slug}-task-${String(i).padStart(2, "0")}.md`;
          try {
            const content = await invoke<{ content: string }>("read_text_file", { filePath: taskPath, projectPath: project.path });
            files[i] = content.content;
          } catch { break; }
        }
        setTaskFiles(files);
      } catch { /* silent */ }
    })();
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

  const handleFullRevision = async () => {
    if (!isActionable) return;
    setBusy(true);
    try {
      await planApi.updatePlanPhase(plan.id, "drafting");
      await planApi.createPlanEvent(plan.id, "plan_full_revision_requested", "user");
      onPlanUpdate(plan.id, { phase: "drafting" as PlanPhase });

      // Send context to main chat Architect
      const list = subtasks.map((s, i) =>
        `${i + 1}. ${s.title}${s.details ? ` — ${s.details}` : ""}`
      ).join("\n");
      const prompt = [
        `[전체 Plan 수정 요청] "${plan.title}" v${plan.versionMajor}.${plan.versionMinor}`,
        "",
        `Subtask 검토 결과 전체 Plan의 구조적 수정이 필요합니다.`,
        `기존 Plan을 검토하고 변경이 필요한 부분을 업데이트하세요.`,
        "",
        `### 현재 Subtasks`,
        list,
        "",
        `\`<!-- tunaflow:plan-proposal -->\` 형식으로 수정된 전체 Plan을 제안하세요.`,
        `**모든 subtask를 포함하세요. 누락된 subtask는 삭제됩니다.**`,
      ].join("\n");

      await sendWithEngine(mainEngine, prompt);
      onSwitchToChat?.();
    } catch { /* silent */ }
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
      saveConversationEngine(shadowConvId, { profileId: null, engine: mainEngine });

      await loadBranches(plan.conversationId);
      await openThread(branch.id);

      // Send lightweight revision prompt — agent reads the file directly
      const slug = plan.title.replace(/[^\w가-힣-]/g, "-").replace(/-+/g, "-").replace(/^-|-$/g, "").toLowerCase().slice(0, 80);
      const taskFile = `docs/plans/${slug}-task-${String(subtaskIdx + 1).padStart(2, "0")}.md`;
      const prompt = [
        `[Subtask ${subtaskIdx + 1} 수정 요청] "${st.title}"`,
        "",
        `### 검토 의견`,
        opinion,
        "",
        `작업 지시서 파일: \`${taskFile}\``,
        `위 검토 의견을 반영하여 파일을 직접 수정하고 변경 내용을 요약해주세요.`,
      ].join("\n");

      await sendThreadMessage(prompt, mainEngine);
      await planApi.createPlanEvent(plan.id, "subtask_revision_requested", "user",
        `subtask ${subtaskIdx + 1}: ${opinion.slice(0, 100)}`);
    } catch { /* silent */ }
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
      saveConversationEngine(shadowConvId, { profileId: null, engine: mainEngine });
      await loadBranches(plan.conversationId);
      await openThread(branch.id);

      const slug = plan.title.replace(/[^\w가-힣-]/g, "-").replace(/-+/g, "-").replace(/^-|-$/g, "").toLowerCase().slice(0, 80);
      const taskFile = `docs/plans/${slug}-task-${String(subtaskIdx + 1).padStart(2, "0")}.md`;
      const prompt = [
        `Subtask ${subtaskIdx + 1} "${st.title}"에 대해 논의합니다.`,
        `작업 지시서 파일: \`${taskFile}\``,
        `\n이 subtask에 대해 질문하거나 검토 의견을 나눠주세요.`,
      ].join("\n");

      await sendThreadMessage(prompt, mainEngine);
    } catch { /* silent */ }
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

      saveConversationEngine(shadowConvId, { profileId: null, engine: mainEngine });

      await loadBranches(plan.conversationId);
      await openThread(branch.id);

      const slug = plan.title.replace(/[^\w가-힣-]/g, "-").replace(/-+/g, "-").replace(/^-|-$/g, "").toLowerCase().slice(0, 80);
      const taskFile = `docs/plans/${slug}-task-${String(subtaskIdx + 1).padStart(2, "0")}.md`;
      const prompt = [
        `[작업 지시서 작성] Subtask ${subtaskIdx + 1}: "${st.title}"`,
        "",
        `파일: \`${taskFile}\``,
        `이 subtask의 상세 작업 지시서를 작성해주세요.`,
        `대상 파일, 구현 접근법, 의존성, 리스크, 완료 기준을 포함하세요.`,
      ].join("\n");

      await sendThreadMessage(prompt, mainEngine);
      await planApi.createPlanEvent(plan.id, "detail_design_requested", "user",
        `subtask ${subtaskIdx + 1}`);
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
          {(plan.versionMajor > 1 || plan.versionMinor > 0) && (
            <span className="text-[8px] font-mono text-muted-foreground/50 px-1 rounded bg-accent/50">v{plan.versionMajor}.{plan.versionMinor}</span>
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
          <button onClick={handleApprove} disabled={busy}
            className="flex items-center gap-1 px-3 py-1.5 rounded-md text-xs font-medium bg-status-approved/10 text-status-approved hover:bg-status-approved/20 disabled:opacity-50 transition-colors">
            <Check className="w-3.5 h-3.5" />승인 → Approved
          </button>
          <button onClick={handleFullRevision} disabled={busy}
            className="flex items-center gap-1 px-3 py-1.5 rounded-md text-xs font-medium bg-accent text-muted-foreground hover:text-foreground disabled:opacity-50 transition-colors">
            <ArrowLeft className="w-3.5 h-3.5" />전체 수정 → Chat
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
          {/* Work instruction — file content > DB details > empty */}
          {taskFileContent ? (
            <div>
              <div className="flex items-center gap-1 mb-1">
                <FileText className="w-3 h-3 text-primary/50" />
                <span className="text-[9px] text-muted-foreground/60 uppercase tracking-wide">작업 지시서</span>
                <span className="text-[8px] text-status-approved/50">파일</span>
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
                <span className="text-[9px] text-muted-foreground/60 uppercase tracking-wide">작업 지시서 (요약)</span>
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

          {/* Actions: discuss + revision request */}
          {actionable && !opinionMode && (
            <div className="flex items-center gap-2 pt-1">
              <button onClick={(e) => { e.stopPropagation(); onDiscuss(); }} disabled={busy}
                className="flex items-center gap-0.5 text-[9px] text-primary/60 hover:text-primary disabled:opacity-40 transition-colors">
                <ChevronRight className="w-2.5 h-2.5" />대화하기
              </button>
              {hasDetails && (
                <button onClick={(e) => { e.stopPropagation(); setOpinionMode(true); }} disabled={busy}
                  className="flex items-center gap-0.5 text-[9px] text-amber-600/60 hover:text-amber-600 disabled:opacity-40 transition-colors">
                  <RotateCcw className="w-2.5 h-2.5" />수정 요청
                </button>
              )}
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
