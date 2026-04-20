import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useChatStore } from "@/stores/chatStore";
import { FileText, Search, RefreshCw, Loader2 } from "lucide-react";
import type { Plan, PlanPhase, PlanSubtask } from "@/types";
import * as planApi from "@/lib/api/plans";
import { errorMessage } from "@/lib/utils";
import { getPlanSlug } from "@/lib/workflowOrchestration";
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
  const { sendWithEngine, selectedConversationId, getConversationEngine, projects, selectedProjectKey } = useChatStore();
  const runningThreadIds = useChatStore((s) => s.runningThreadIds);
  const [busy, setBusy] = useState(false);
  const [syncing, setSyncing] = useState(false);
  const hasEmptyDetails = subtasks.some((s) => !s.details?.trim());
  const hasSubtasks = subtasks.length > 0;
  // Architect 가 해당 plan 의 대화에서 여전히 실행 중인가. Drafting 단계에서 agent
  // running 은 대부분 "plan 문서 작성 중" — 확정 시그널 아니지만 가장 실용적인 proxy.
  const isArchitectWriting = runningThreadIds.includes(plan.conversationId) && hasEmptyDetails;
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

  /** Read task docs and populate subtask details from file content summaries. */
  const handleSyncFromDocs = async () => {
    const project = projects.find((p) => p.key === selectedProjectKey);
    if (!project?.path) { toast.error("프로젝트 경로를 찾을 수 없습니다."); return; }
    setSyncing(true);
    try {
      const slug = getPlanSlug(plan);
      const entries = await invoke<{ name: string; path: string; isDir: boolean }[]>(
        "list_directory", { path: `${project.path}/docs/plans` }
      ).catch(() => [] as { name: string; path: string; isDir: boolean }[]);

      const taskFiles = entries
        .filter((e) => !e.isDir && e.name.match(new RegExp(`^${slug}-task-\\d+\\.md$`)))
        .sort((a, b) => a.name.localeCompare(b.name));

      if (taskFiles.length === 0) {
        toast.error(`docs/plans/${slug}-task-*.md 파일을 찾지 못했습니다.`);
        setSyncing(false);
        return;
      }

      // Read each task file and extract a concise details summary
      const updatedSubtasks = await Promise.all(subtasks.map(async (st, i) => {
        const file = taskFiles[i];
        if (!file) return { title: st.title, details: st.details ?? undefined };
        const content = await invoke<string>("read_file_content", { path: file.path }).catch(() => "");
        // Extract "Change description" / "변경 내용" section first paragraph
        const details = extractDetailsFromDoc(content) ?? st.details ?? undefined;
        return { title: st.title, details };
      }));

      await planApi.replacePlanSubtasks(plan.id, updatedSubtasks);
      await planApi.createPlanEvent(plan.id, "review_merged", "system", `Synced details from ${taskFiles.length} task docs`);
      toast.success(`${taskFiles.length}개 subtask 상세 설계 동기화 완료`);
      // Reload the plan card
      onPlanUpdate({ phase: plan.phase });
    } catch (e) {
      console.error("[DraftingActions] sync from docs failed:", e);
      toast.error("문서 동기화 실패: " + errorMessage(e));
    }
    setSyncing(false);
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
      {hasEmptyDetails && hasSubtasks && !isArchitectWriting && (
        <p className="text-[9px] text-amber-600/60">일부 subtask에 상세 설계가 없습니다.</p>
      )}
      <div className="flex items-center gap-2 flex-wrap">
        {hasEmptyDetails && hasSubtasks && (
          <>
            <button onClick={handleDetailDesign} disabled={busy || syncing || isArchitectWriting} className="flex items-center gap-1 px-2.5 py-1 rounded-md text-[10px] font-medium bg-amber-500/10 text-amber-600 hover:bg-amber-500/20 disabled:opacity-50 transition-colors">
              <FileText className="w-3 h-3" />{busy ? "요청 중..." : "상세 설계 요청"}
            </button>
            <button onClick={handleSyncFromDocs} disabled={busy || syncing || isArchitectWriting} className="flex items-center gap-1 px-2.5 py-1 rounded-md text-[10px] font-medium bg-primary/10 text-primary hover:bg-primary/20 disabled:opacity-50 transition-colors">
              {syncing ? <Loader2 className="w-3 h-3 animate-spin" /> : <RefreshCw className="w-3 h-3" />}
              {syncing ? "동기화 중..." : "docs에서 동기화"}
            </button>
          </>
        )}
        {/* Subtask 검토 버튼: 상세 설계가 모두 채워졌을 때만 노출. 문서 작성 완료 전에는
            Dev 로 올라가는 관문이 아예 보이지 않음 — 사용자가 미완성 문서로 진행하는 사고 방지. */}
        {hasSubtasks && !hasEmptyDetails && (
          <button onClick={handleStartReview} disabled={busy || syncing} className="flex items-center gap-1 px-2.5 py-1 rounded-md text-[10px] font-medium bg-primary/10 text-primary hover:bg-primary/20 disabled:opacity-50 transition-colors">
            <Search className="w-3 h-3" />{busy ? "이동 중..." : "Subtask 검토"}
          </button>
        )}
        {!hasSubtasks && (
          <p className="text-[9px] text-muted-foreground/50">Subtask가 없습니다. Chat에서 Architect에게 Plan 수정을 요청하세요.</p>
        )}
      </div>
      {/* 아키텍트 문서 작성 중 표시 — agent 가 해당 conv 에서 실행 중이고 아직 details 비어있을 때 */}
      {isArchitectWriting && (
        <div className="flex items-center gap-1.5 pt-1 text-[10px] text-muted-foreground">
          <Loader2 className="w-3 h-3 animate-spin text-primary" />
          <span>아키텍트 문서 작성 중...</span>
        </div>
      )}
    </div>
  );
}

/** Extract a concise details summary from a task markdown file.
 *  Looks for "Change description" / "변경 내용" section first, then falls back to
 *  the first non-heading paragraph.
 */
function extractDetailsFromDoc(md: string): string | null {
  if (!md.trim()) return null;

  // Try to find "Change description" or "변경 내용" section
  const sectionMatch = md.match(/^#+\s+(?:Change description|변경 내용|구현 방법)[^\n]*\n+([\s\S]+?)(?=\n#+\s|\n---|\n\*\*|$)/im);
  if (sectionMatch) {
    const text = sectionMatch[1].replace(/^[-*]\s+/gm, "").trim();
    const lines = text.split("\n").filter((l) => l.trim()).slice(0, 3);
    if (lines.length > 0) return lines.join(" / ").slice(0, 300);
  }

  // Fallback: first non-heading paragraph after the H1
  const paragraphMatch = md.match(/^#{1,2}[^\n]+\n+((?:[^#\n][^\n]*\n?)+)/m);
  if (paragraphMatch) {
    const text = paragraphMatch[1].trim().split("\n")[0];
    if (text.length > 10) return text.slice(0, 200);
  }

  return null;
}
