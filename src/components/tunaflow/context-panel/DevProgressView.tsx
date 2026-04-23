import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { GitBranch, Check, Loader2, Clock, RotateCcw, Plus, ClipboardList, FileText } from "lucide-react";
import type { Plan, PlanPhase, PlanSubtask } from "@/types";
import * as planApi from "@/lib/api/plans";
import { getPlanSlug, syncResultReport, startReviewRT } from "@/lib/workflowOrchestration";
import { runProjectTests } from "@/lib/api/testRunner";
import type { Branch, Message } from "@/types";
import { PlanDocumentModal } from "./PlanDocumentModal";
import { useSubtaskProgress } from "./useSubtaskProgress";
import { ApprovalGate } from "./plans/ApprovalGate";

interface DevProgressViewProps {
  plan: Plan;
  onPlanUpdate: (planId: string, update: Partial<Plan>) => void;
}

export function DevProgressView({ plan, onPlanUpdate }: DevProgressViewProps) {
  const { openThread, sendThreadMessage, sendThreadRoundtable, loadBranches, saveConversationEngine } = useChatStore();
  const profiles = useChatStore((s) => s.agentProfiles);
  const runningThreadIds = useChatStore((s) => s.runningThreadIds);

  const {
    subtasks, completedNums, implComplete, loading,
    testResult, testRunning, reviewVerdict, designReviewSuggested,
    failCount, doomLoopEscalated,
  } = useSubtaskProgress(plan);

  const [busy, setBusy] = useState(false);
  const [showDoc, setShowDoc] = useState(false);
  // reviewMode state 제거 (2026-04-19) — Dev 시작과 동일하게 한 번 클릭으로 즉시 실행.
  // 이전엔 "Review 시작" → select 모드 전환 → reviewer 선택 + "Review 시작" 3단계였고,
  // 이제는 implComplete 가 되면 곧바로 reviewer 선택 UI 가 노출되고 "Review 시작" 1회로 끝.
  const branchRunning = plan.implementationBranchId
    ? runningThreadIds.includes(`branch:${plan.implementationBranchId}`)
    : false;

  const [selectedReviewerId, setSelectedReviewerId] = useState(() => {
    const reviewer = profiles.find((p) => p.label.toLowerCase().includes("review"));
    return reviewer?.id ?? profiles[0]?.id ?? "";
  });

  // S3: Quick vs Deep RT review track. Quick = 단일 chat, Deep = RT (≥2 reviewers).
  const [reviewTrack, setReviewTrack] = useState<"quick" | "deep">("quick");
  const [selectedDeepIds, setSelectedDeepIds] = useState<Set<string>>(() => {
    // 기본값: "review" 라벨 포함 프로필 두 개 이상 있으면 자동 선택, 없으면 빈 set.
    const reviewers = profiles.filter((p) => p.label.toLowerCase().includes("review")).slice(0, 2);
    return new Set(reviewers.map((p) => p.id));
  });
  const toggleDeepReviewer = (id: string) => {
    setSelectedDeepIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id); else next.add(id);
      return next;
    });
  };

  const handleOpenBranch = () => {
    if (plan.implementationBranchId) openThread(plan.implementationBranchId);
  };

  const handleRerunSubtask = async (subtask: PlanSubtask, index: number) => {
    if (!plan.implementationBranchId) return;
    setBusy(true);
    try {
      await openThread(plan.implementationBranchId);
      const prompt = `Subtask ${index + 1} "${subtask.title}"을(를) 다시 구현해주세요.\n\n상세 설계:\n${subtask.details ?? "(없음)"}`;
      const shadowConvId = `branch:${plan.implementationBranchId}`;
      const saved = useChatStore.getState().getConversationEngine(shadowConvId);
      await sendThreadMessage(prompt, saved?.engine ?? "claude", saved?.model ?? undefined);
    } catch (e) { console.warn("[tunaflow]", e); }
    setBusy(false);
  };

  const handleStartReview = async () => {
    if (!plan.implementationBranchId) return;
    const selectedProfile = profiles.find((p) => p.id === selectedReviewerId);
    if (!selectedProfile) return;
    setBusy(true);
    try {
      const implShadow = `branch:${plan.implementationBranchId}`;
      const msgs = await invoke<Message[]>("list_messages", { conversationId: implShadow });
      await syncResultReport(plan.id, msgs, plan.developerEngine ?? undefined);

      const isRework = plan.phase === "rework" || !!reviewVerdict;
      const reviewRound = (plan.versionMinor || 0) + 1;
      const roundLabel = isRework ? `review (${reviewRound})` : `review`;
      await planApi.updatePlanPhase(plan.id, "review");
      await planApi.createPlanEvent(plan.id, "review_started", "user",
        `reviewer=${selectedProfile.label}${isRework ? " (rework)" : ""}`);

      // A안: 같은 chat 모드 리뷰 브랜치가 있으면 재사용. 모드 다른 브랜치는 archive 후 신규.
      // 이전 라운드 대화를 reviewer 가 자연스럽게 참고할 수 있어 context 보존도 커짐.
      const { getOrCreateReviewBranch } = await import("@/lib/workflow/helpers");
      const { branch, shadowConvId, reused } = await getOrCreateReviewBranch(
        plan, `${roundLabel}: ${plan.title.slice(0, 25)}`, "chat",
      );
      if (reused) console.debug("[handleStartReview] reusing branch:", branch.id);
      saveConversationEngine(shadowConvId, { profileId: selectedReviewerId, engine: selectedProfile.engine, model: selectedProfile.model });

      const slug = getPlanSlug(plan);

      await loadBranches(plan.conversationId);
      await openThread(branch.id);

      // Scope re-review to failed subtasks only
      const failedIds = reviewVerdict?.failedSubtaskIds ?? [];
      const isScoped = failedIds.length > 0 && reviewVerdict;

      const prevFindingsBlock = reviewVerdict && reviewVerdict.findings.length > 0
        ? [``, `**이전 Review Findings (중점 확인 대상)**:`,
           ...reviewVerdict.findings.map((f, i) => `${i + 1}. ${f.slice(0, 500)}`),
           ``, `> 위 사항이 해결되었는지 확인하세요.`]
        : [];

      const taskScope = isScoped
        ? failedIds.map((id) => `- \`docs/plans/${slug}-task-${String(id).padStart(2, "0")}.md\``)
        : [`- \`docs/plans/${slug}-task-*.md\``];

      const scopeNote = isScoped
        ? [``, `**리뷰 범위**: Task ${failedIds.join(", ")}번만 검토하세요. 나머지 subtask는 이전 리뷰에서 pass되었습니다.`]
        : [];

      const prompt = [
        `### 🔍 ${roundLabel} 요청`, ``,
        `**Plan**: "${plan.title}"`, ``,
        `**검증 문서**:`,
        `- Plan: \`docs/plans/${slug}.md\``,
        `- 결과: \`docs/plans/${slug}-result.md\``,
        ...taskScope,
        ...scopeNote,
        ...prevFindingsBlock, ``,
        `각 task 파일의 **변경 내용**과 **검증 조건**을 기준으로 구현 결과를 검증하세요.`,
        `Plan에 명시되지 않은 코드 스타일이나 범위 밖 파일의 기존 문제는 fail 사유가 아닙니다.`, ``,
        `> 완료 후 리뷰 verdict를 제출하세요.`,
      ].join("\n");

      // 자동 생성 프롬프트 → role="system" 으로 persist 후 sendThreadMessage 에 pre-existing
      // id 로 전달. UI 는 사용자 말풍선이 아닌 접힘 시스템 블록으로 렌더 (s37 원칙).
      const sysMsgId = await invoke<string>("persist_system_msg", {
        conversationId: shadowConvId,
        content: prompt,
      }).catch((e) => { console.warn("[handleStartReview] persist_system_msg failed:", e); return null; });
      await sendThreadMessage(
        prompt,
        selectedProfile.engine,
        selectedProfile.model,
        sysMsgId ? { userMessageId: sysMsgId } : undefined,
      );
      onPlanUpdate(plan.id, { phase: "review" as PlanPhase, reviewBranchId: branch.id });
    } catch (e) { console.warn("[tunaflow]", e); }
    setBusy(false);
  };

  // Deep RT track — multi-engine review. Uses the existing startReviewRT flow
  // (roundtable with ≥2 reviewer engines) and auto-runs project tests first so
  // reviewers see real verification output instead of trusting Developer's self-report.
  const handleStartReviewRT = async () => {
    if (!plan.implementationBranchId) return;
    const chosenProfiles = profiles.filter((p) => selectedDeepIds.has(p.id));
    if (chosenProfiles.length < 2) return;
    // 진입 게이트 — reviewers 역할 설정/모델 상태를 검증. missing 이면 toast + Settings 열기.
    const { assertRoleReady } = await import("@/lib/roleAssignments");
    const gate = await assertRoleReady("reviewers", profiles);
    if (!gate.ok) return;
    setBusy(true);
    try {
      const implShadow = `branch:${plan.implementationBranchId}`;
      const msgs = await invoke<Message[]>("list_messages", { conversationId: implShadow });
      // S2 Agent-as-Judge: run project tests, pass output to reviewers.
      let testOutput: string | undefined;
      try {
        const projectKey = useChatStore.getState().selectedProjectKey;
        if (projectKey) {
          const project = await invoke<{ path?: string }>("get_project", { key: projectKey });
          if (project?.path) {
            const result = await runProjectTests(project.path);
            testOutput = result.output;
          }
        }
      } catch (e) { console.debug("[test-before-review-rt]", e); }

      // 프로필의 engine+model 을 모두 전달해야 RT executor → codex_app_server 까지
      // 실제 설정값이 흐른다. 이전엔 `.map(p => p.engine)` 로 string[] 만 넘겨서
      // model 이 유실 → codex_app_server fallback(gpt-5-codex) 로 귀결 → ChatGPT
      // 구독 계정에서 400 에러가 발생했음 (s37 재현).
      const reviewers = chosenProfiles.map((p) => ({ engine: p.engine, model: p.model, name: p.label }));
      const { branch, participants, prompt, mode } = await startReviewRT(plan, msgs, testOutput, reviewers);
      onPlanUpdate(plan.id, { phase: "review" as PlanPhase, reviewBranchId: branch.id });
      await loadBranches(plan.conversationId);
      await openThread(branch.id);
      // Deep review = ≥2 reviewers → auto-synthesize MoA summary after the round.
      await sendThreadRoundtable(prompt, participants, mode, { autoSynthesize: true });
    } catch (e) { console.warn("[tunaflow]", e); }
    setBusy(false);
  };

  const handleCreateSubPlan = async (subtask: PlanSubtask, index: number) => {
    const { sendWithEngine } = useChatStore.getState();
    setBusy(true);
    try {
      const prompt = [
        `[Sub-plan 요청] Plan "${plan.title}" → Subtask ${index + 1} "${subtask.title}"`, "",
        `이 subtask의 구현이 복잡하여 별도 Plan이 필요합니다.`,
        subtask.details ? `\n### 기존 상세 설계\n${subtask.details}` : "", "",
        `이 subtask를 위한 별도 Plan을 \`<!-- tunaflow:plan-proposal -->\` 형식으로 제안하세요.`,
        `부모 Plan: ${plan.title}`,
      ].filter(Boolean).join("\n");
      await sendWithEngine("claude", prompt);
    } catch (e) { console.warn("[tunaflow]", e); }
    setBusy(false);
  };

  const handleRework = async () => {
    if (!plan.implementationBranchId) return;
    setBusy(true);
    try {
      await planApi.updatePlanPhase(plan.id, "implementation");
      await planApi.createPlanEvent(plan.id, "rework_requested", "user");
      await openThread(plan.implementationBranchId);
      const findingItems = reviewVerdict?.findings.map((f, i) => {
        const fileMatch = f.match(/([a-zA-Z0-9_./-]+\.[a-zA-Z]+(?:[:#]L?\d+)?)/);
        const file = fileMatch ? fileMatch[1] : "";
        const summary = f.slice(0, 500);
        return file ? `□ ${i + 1}. ${summary}\n  파일: ${file}` : `□ ${i + 1}. ${summary}`;
      }) ?? [];
      const recItems = reviewVerdict?.recommendations.map((r) => `• ${r.slice(0, 300)}`) ?? [];

      const events = await planApi.listPlanEvents(plan.id);
      // Count failures since last escalation (not total)
      let lastEscIdx = -1;
      for (let i = events.length - 1; i >= 0; i--) {
        if (events[i].eventType === "doom_loop_escalated" || events[i].eventType === "architect_redesign_requested") { lastEscIdx = i; break; }
      }
      const sinceReset = lastEscIdx >= 0 ? events.slice(lastEscIdx + 1) : events;
      const failEvents = sinceReset.filter((e) => e.eventType === "review_failed");
      const failCount = failEvents.length;
      const pressureWarning = failCount >= 2
        ? `\n> ⚠️ 이전 ${failCount}회 Review 실패. ${failCount >= 3 ? "이번이 마지막 기회입니다." : "다음 실패 시 설계 재검토로 에스컬레이션됩니다."}`
        : "";

      let historySection = "";
      const previousFails = failEvents.slice(0, -1).slice(-3);
      if (previousFails.length > 0) {
        const historyItems = previousFails.map((ev, i) => {
          try {
            const d = JSON.parse(ev.detail ?? "{}");
            const findings = (d.findings as string[] ?? []).slice(0, 3).map((f: string) => f.slice(0, 200));
            return `- ${i + 1}차: ${findings.join("; ") || "상세 없음"} → ❌`;
          } catch { return `- ${i + 1}차: (파싱 불가) → ❌`; }
        });
        historySection = [`**이전 시도 이력** (${previousFails.length}회 실패):`, ...historyItems, ``].join("\n");
      }

      const failedIds = reviewVerdict?.failedSubtaskIds ?? [];
      const slug = getPlanSlug(plan);
      let targetSection = "";
      if (failedIds.length > 0 && subtasks.length > 0) {
        const targetNames = failedIds
          .map((id) => { const st = subtasks.find((s) => s.idx === id); return st ? `Task ${String(id).padStart(2, "0")} (${st.title})` : `Task ${String(id).padStart(2, "0")}`; })
          .join(", ");
        const otherCount = subtasks.length - failedIds.length;
        targetSection = [`**대상 서브태스크**: ${targetNames}`,
          otherCount > 0 ? `**나머지 ${otherCount}개 태스크**: 이미 완료됨 — 수정하지 마세요.` : "", ``].filter(Boolean).join("\n");
      }

      const reworkPrompt = [
        `### 🔄 Rework`, ``, historySection, targetSection,
        `**수정 항목** (${findingItems.length}건):`,
        ...findingItems.map((f) => `- ${f}`),
        ...(recItems.length > 0 ? [``, `**Recommendations**:`, ...recItems.map((r) => `- ${r}`)] : []),
        ``, `> 완료 조건: 위 항목 모두 해결 후 완료를 알려주세요.${failedIds.length > 0 ? " 다른 태스크의 코드를 변경하지 마세요." : ""}`,
        pressureWarning,
      ].filter(Boolean).join("\n");
      const shadowConvId = `branch:${plan.implementationBranchId}`;
      const saved = useChatStore.getState().getConversationEngine(shadowConvId);
      await sendThreadMessage(reworkPrompt, saved?.engine ?? "claude", saved?.model ?? undefined);
      onPlanUpdate(plan.id, { phase: "implementation" as PlanPhase });
    } catch (e) { console.warn("[tunaflow]", e); }
    setBusy(false);
  };

  if (loading) {
    return <p className="text-xs text-muted-foreground px-2">Loading...</p>;
  }

  // Approval phase: show the gate UI before implementation starts
  if (plan.phase === "approval") {
    return (
      <div className="rounded-lg border border-border bg-card p-3 space-y-2">
        <div className="flex items-center gap-2">
          <ClipboardList className="w-4 h-4 text-primary/60" />
          <span className="text-xs font-medium text-foreground flex-1">{plan.title}</span>
          <button onClick={() => setShowDoc(true)} className="flex items-center gap-1 text-[9px] text-muted-foreground/50 hover:text-primary/60 transition-colors">
            <FileText className="w-3 h-3" />문서
          </button>
        </div>
        <ApprovalGate
          plan={plan}
          subtasks={subtasks}
          onPlanUpdate={(update) => onPlanUpdate(plan.id, update)}
        />
        {showDoc && <PlanDocumentModal plan={plan} onClose={() => setShowDoc(false)} />}
      </div>
    );
  }

  return (
    <div className="space-y-3">
      {/* Plan header + branch link */}
      <div className={cn(
        "rounded-lg border bg-card p-3",
        branchRunning ? "border-primary/30" : "border-border"
      )}>
        <div className="flex items-center gap-2">
          {branchRunning
            ? <Loader2 className="w-4 h-4 text-primary animate-spin" />
            : <ClipboardList className="w-4 h-4 text-primary/60" />}
          <span className="text-xs font-medium text-foreground flex-1">{plan.title}</span>
          {branchRunning && (
            <span className="text-[9px] text-primary/60">구현 중...</span>
          )}
          <button onClick={() => setShowDoc(true)} className="flex items-center gap-1 text-[9px] text-muted-foreground/50 hover:text-primary/60 transition-colors">
            <FileText className="w-3 h-3" />문서
          </button>
          {plan.implementationBranchId && (
            <button onClick={handleOpenBranch} className="flex items-center gap-1 text-[9px] text-primary/60 hover:text-primary transition-colors">
              <GitBranch className="w-3 h-3" />Branch 열기
            </button>
          )}
        </div>
      </div>

      {/* Subtask progress */}
      <div className="space-y-1.5">
        {subtasks.map((st, i) => {
          const num = i + 1;
          const isDone = completedNums.has(num);

          return (
            <div key={st.id} className={cn(
              "rounded-md border p-2.5 flex items-start gap-2",
              isDone ? "border-status-approved/30 bg-status-approved/5" : "border-border bg-card"
            )}>
              <div className="shrink-0 mt-0.5">
                {isDone ? <Check className="w-3.5 h-3.5 text-status-approved" /> :
                 <Clock className="w-3.5 h-3.5 text-muted-foreground/30" />}
              </div>
              <div className="flex-1 min-w-0">
                <p className={cn("text-[11px] font-medium leading-snug",
                  isDone ? "text-status-approved/80" : "text-foreground"
                )}>
                  {num}. {st.title}
                </p>
                {st.details && (
                  <p className="text-[10px] text-muted-foreground/60 leading-snug mt-0.5 line-clamp-2">{st.details}</p>
                )}
                <div className="flex items-center gap-2 mt-1.5">
                  {isDone && <span className="text-[9px] text-status-approved/60">완료</span>}
                  {!isDone && (
                    <button onClick={() => handleRerunSubtask(st, i)} disabled={busy}
                      className="flex items-center gap-0.5 text-[9px] text-primary/60 hover:text-primary disabled:opacity-40 transition-colors">
                      <RotateCcw className="w-2.5 h-2.5" />재수행
                    </button>
                  )}
                  <button onClick={() => handleCreateSubPlan(st, i)} disabled={busy}
                    className="flex items-center gap-0.5 text-[9px] text-muted-foreground/50 hover:text-foreground disabled:opacity-40 transition-colors">
                    <Plus className="w-2.5 h-2.5" />Sub-plan
                  </button>
                </div>
              </div>
            </div>
          );
        })}
      </div>

      {/* Rework notice */}
      {plan.phase === "rework" && (
        <div className={cn(
          "rounded-md border p-2.5 text-[10px] space-y-2",
          doomLoopEscalated
            ? "border-amber-500/40 bg-amber-500/10 text-amber-600"
            : "border-status-rejected/30 bg-status-rejected/5 text-status-rejected"
        )}>
          {doomLoopEscalated ? (
            <>
              {failCount === 0 ? (
                <>
                  <p className="font-semibold">⚠️ 설계 재검토 필요 — Architect 재설계 진행 중</p>
                  <p className="text-[9px] text-foreground/60">
                    이전 싸이클에서 Review 5회 이상 실패로 에스컬레이션 되었습니다.
                    Architect 의 rev 제안을 검토/승인한 뒤 Dev 를 다시 시작하세요.
                  </p>
                </>
              ) : failCount === 1 ? (
                <>
                  <p className="font-semibold">⚠️ Review 1회 실패 — 이전 싸이클 설계 재검토 이력 있음</p>
                  <p className="text-[9px] text-foreground/60">
                    동일 패턴이 반복되면 설계 재검토가 또 필요할 수 있습니다.
                    Rework 으로 해소되지 않으면 Architect 재설계를 요청하세요.
                  </p>
                </>
              ) : (
                <>
                  <p className="font-semibold">⚠️ Review {failCount}회 연속 실패 — 설계 재검토가 필요합니다</p>
                  <p className="text-[9px] text-foreground/60">
                    동일한 문제가 반복되고 있어 Rework 으로 해결되지 않습니다.
                    Subtask 설계를 재검토하고 Architect 에게 수정을 요청하세요.
                  </p>
                </>
              )}
            </>
          ) : (
            <p className="font-medium">Rework 필요 — Review에서 다음 사항이 지적되었습니다.</p>
          )}
          {designReviewSuggested && !doomLoopEscalated && (
            <p className="text-amber-500 font-medium">⚠️ 동일 파일에서 반복 실패 — Rework 대신 설계 재검토를 권장합니다.</p>
          )}
          {reviewVerdict && reviewVerdict.findings.length > 0 && (
            <ul className="space-y-0.5 text-[9px] text-foreground/60 pl-2">
              {reviewVerdict.findings.map((f, i) => <li key={i}>- {f.slice(0, 200)}</li>)}
            </ul>
          )}
          {reviewVerdict && reviewVerdict.recommendations.length > 0 && (
            <div className="text-[9px] text-muted-foreground/50">
              Recommendations: {reviewVerdict.recommendations.map((r) => r.slice(0, 100)).join("; ")}
            </div>
          )}
          <div className="flex items-center gap-2">
            {!doomLoopEscalated && (
              <button onClick={handleRework} disabled={busy}
                className="px-2.5 py-1 rounded-md text-[10px] font-medium bg-primary/10 text-primary hover:bg-primary/20 disabled:opacity-50 transition-colors">
                {busy ? "전달 중..." : "Developer에게 전달 + Rework"}
              </button>
            )}
            <button
              onClick={async () => {
                setBusy(true);
                try {
                  await planApi.updatePlanPhase(plan.id, "subtask_review");
                  await planApi.createPlanEvent(plan.id, "reverted_to_subtask_review", "user",
                    doomLoopEscalated ? "Doom loop escalation — design review required" : "Design change needed from rework");
                  onPlanUpdate(plan.id, { phase: "subtask_review" as PlanPhase });
                } catch (e) { console.warn("[tunaflow]", e); }
                setBusy(false);
              }}
              disabled={busy}
              className={cn("px-2.5 py-1 rounded-md text-[10px] font-medium disabled:opacity-50 transition-colors",
                doomLoopEscalated
                  ? "bg-amber-500/20 text-amber-600 hover:bg-amber-500/30"
                  : "bg-accent text-muted-foreground hover:text-foreground"
              )}>
              {doomLoopEscalated ? "설계 재검토 시작 →" : "설계 변경 → Subtask"}
            </button>
          </div>
        </div>
      )}

      {/* Test results */}
      {testRunning && (
        <div className="rounded-md border border-primary/20 bg-primary/5 p-2.5 text-[10px] text-primary flex items-center gap-2">
          <Loader2 className="w-3.5 h-3.5 animate-spin" />테스트 실행 중...
        </div>
      )}
      {testResult && !testRunning && (
        <div className={cn(
          "rounded-md border p-2.5 text-[10px] space-y-1",
          testResult.success ? "border-status-approved/30 bg-status-approved/5 text-status-approved" : "border-status-rejected/30 bg-status-rejected/5 text-status-rejected"
        )}>
          <div className="font-medium">{testResult.testType} 테스트: {testResult.success ? "PASS" : "FAIL"}</div>
          <div className="flex gap-3 text-[9px]">
            <span>통과: {testResult.passed}</span>
            <span>실패: {testResult.failed}</span>
            {testResult.skipped > 0 && <span>건너뜀: {testResult.skipped}</span>}
            <span>{testResult.durationMs >= 60000 ? `${Math.floor(testResult.durationMs / 60000)}m ${Math.round((testResult.durationMs % 60000) / 1000)}s` : `${Math.round(testResult.durationMs / 1000)}s`}</span>
          </div>
          {!testResult.success && testResult.output && (
            <details className="mt-1">
              <summary className="text-[9px] cursor-pointer text-muted-foreground/60 hover:text-foreground">출력 보기</summary>
              <pre className="text-[8px] mt-1 max-h-32 overflow-auto bg-card/50 rounded p-1.5 whitespace-pre-wrap">{testResult.output.slice(0, 2000)}</pre>
            </details>
          )}
        </div>
      )}

      {/* Re-review findings summary */}
      {implComplete && plan.phase !== "rework" && reviewVerdict && (
        <div className="rounded-md border border-amber-500/20 bg-amber-500/5 p-2.5 text-[10px] space-y-1.5">
          <div className="font-medium text-amber-600">이전 Review Findings (Re-review #{(plan.versionMinor || 0) + 1})</div>
          <ul className="space-y-0.5 text-[9px] text-foreground/60 pl-2">
            {reviewVerdict.findings.slice(0, 5).map((f, i) => <li key={i}>□ {f.slice(0, 150)}</li>)}
          </ul>
          <p className="text-[9px] text-muted-foreground/50">위 사항이 수정되었는지 중심으로 재검증됩니다.</p>
        </div>
      )}

      {/* Summary + actions */}
      <div className="pt-2 border-t border-border/30 space-y-1.5">
        <div className="flex items-center gap-2">
          <span className="text-[10px] text-muted-foreground/50">{completedNums.size}/{subtasks.length} 완료</span>
        </div>
        {implComplete && plan.phase !== "rework" && (
          <div className="space-y-1.5">
            {/* Re-review scope indicator */}
            {reviewVerdict && reviewVerdict.failedSubtaskIds.length > 0 && (
              <div className="flex items-center gap-1.5 text-[9px] text-amber-600/70 flex-wrap">
                <span>리뷰 대상:</span>
                {reviewVerdict.failedSubtaskIds.map((id) => {
                  const st = subtasks[id - 1];
                  return (
                    <span key={id} className="px-1.5 py-0.5 rounded bg-amber-500/10 font-medium">
                      Task {id}{st ? ` — ${st.title.slice(0, 20)}` : ""}
                    </span>
                  );
                })}
                <span className="text-muted-foreground/40">나머지는 이전 pass</span>
              </div>
            )}
            {/* Track selector: Quick = single reviewer chat / Deep = RT with ≥2 engines */}
            <div className="flex items-center gap-3 text-[10px]">
              <label className="flex items-center gap-1 cursor-pointer">
                <input type="radio" name="review-track" checked={reviewTrack === "quick"}
                  onChange={() => setReviewTrack("quick")} className="accent-primary" />
                <span>Quick <span className="text-muted-foreground/60">(단일)</span></span>
              </label>
              <label className="flex items-center gap-1 cursor-pointer">
                <input type="radio" name="review-track" checked={reviewTrack === "deep"}
                  onChange={() => setReviewTrack("deep")} className="accent-primary" />
                <span>Deep RT <span className="text-muted-foreground/60">(다명 합의)</span></span>
              </label>
            </div>

            {reviewTrack === "quick" ? (
              <div className="flex items-center gap-2">
                <span className="text-[10px] text-muted-foreground shrink-0">Reviewer:</span>
                <select value={selectedReviewerId} onChange={(e) => setSelectedReviewerId(e.target.value)}
                  disabled={busy}
                  className="flex-1 text-[10px] bg-input border border-border rounded px-1.5 py-0.5 outline-none disabled:opacity-40">
                  {profiles.map((p) => (
                    <option key={p.id} value={p.id}>{p.label} ({p.engine}{p.model ? `/${p.model.slice(0, 24)}` : ""})</option>
                  ))}
                </select>
                <button onClick={handleStartReview} disabled={busy || !selectedReviewerId || branchRunning}
                  className={cn("flex items-center gap-1 px-2.5 py-1 rounded-md text-[10px] font-medium disabled:opacity-50 transition-colors",
                    reviewVerdict ? "bg-amber-500/10 text-amber-600 hover:bg-amber-500/20" : "bg-status-approved/10 text-status-approved hover:bg-status-approved/20",
                  )}>
                  <Check className="w-3 h-3" />{busy ? "시작 중..." : reviewVerdict ? "Re-review 시작" : "Review 시작"}
                </button>
              </div>
            ) : (
              <div className="space-y-1.5">
                <div className="flex items-center gap-2 flex-wrap">
                  <span className="text-[10px] text-muted-foreground shrink-0">Reviewers:</span>
                  {profiles.map((p) => (
                    <label key={p.id} className={cn(
                      "flex items-center gap-1 px-1.5 py-0.5 rounded border cursor-pointer text-[10px]",
                      selectedDeepIds.has(p.id) ? "border-primary/50 bg-primary/5" : "border-border/40 text-muted-foreground"
                    )}>
                      <input type="checkbox" checked={selectedDeepIds.has(p.id)}
                        onChange={() => toggleDeepReviewer(p.id)} className="accent-primary" />
                      <span>{p.label} <span className="text-muted-foreground/50">({p.engine})</span></span>
                    </label>
                  ))}
                </div>
                <div className="flex items-center gap-2">
                  <span className="text-[9px] text-muted-foreground/60">
                    {selectedDeepIds.size}명 선택{selectedDeepIds.size < 2 ? " — 최소 2명 필요" : ""}
                  </span>
                  <button onClick={handleStartReviewRT} disabled={busy || selectedDeepIds.size < 2}
                    className={cn("ml-auto flex items-center gap-1 px-2.5 py-1 rounded-md text-[10px] font-medium disabled:opacity-40 disabled:cursor-not-allowed transition-colors",
                      reviewVerdict ? "bg-amber-500/10 text-amber-600 hover:bg-amber-500/20" : "bg-status-approved/10 text-status-approved hover:bg-status-approved/20",
                    )}>
                    <Check className="w-3 h-3" />{busy ? "시작 중..." : reviewVerdict ? "Re-review RT 시작" : "Review RT 시작"}
                  </button>
                </div>
              </div>
            )}
          </div>
        )}
      </div>
      {showDoc && <PlanDocumentModal plan={plan} onClose={() => setShowDoc(false)} />}
    </div>
  );
}
