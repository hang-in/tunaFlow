import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import * as ContextMenu from "@radix-ui/react-context-menu";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { ChevronDown, ChevronRight, GitBranch, Check, FileText, Ban, CheckCircle, Play, RotateCcw, Plus, Loader2, FolderOpen } from "lucide-react";
import type { Plan, PlanEvent, PlanPhase, PlanSubtask, PlanStatus, SubtaskStatus, Message } from "@/types";
import * as planApi from "@/lib/api/plans";
import { startReviewRT } from "@/lib/workflowOrchestration";
import { loadPlanExpandData } from "@/lib/workflow/planWorkflowService";
import type { ParsedReviewVerdict } from "@/lib/planProposalParser";
import { PLAN_STATUS_CFG, PLAN_PHASE_CFG } from "./constants";
import { PlanDocumentModal } from "../PlanDocumentModal";
import { SubtaskRow } from "./SubtaskRow";
import { EventTimeline } from "./EventTimeline";
import { DraftingActions } from "./DraftingActions";
import { ApprovalGate } from "./ApprovalGate";
import { ReviewVerdictCard } from "./ReviewVerdictCard";
import { MergeBranchButton } from "./MergeBranchButton";

const ctxMenuContent = "min-w-[160px] bg-popover border border-border/40 rounded-lg shadow-xl p-1 z-[100] animate-in fade-in-0 zoom-in-95";
const ctxMenuItem = "flex items-center gap-2 px-2.5 py-1.5 text-tf-caption rounded-md cursor-pointer outline-none transition-colors text-foreground/80 data-[highlighted]:bg-accent data-[highlighted]:text-foreground";
const ctxMenuDestructive = "flex items-center gap-2 px-2.5 py-1.5 text-tf-caption rounded-md cursor-pointer outline-none transition-colors text-destructive/70 data-[highlighted]:bg-destructive/10 data-[highlighted]:text-destructive";
const ctxMenuSeparator = "h-px bg-border/30 my-1 mx-1";
const ctxMenuIcon = "w-3.5 h-3.5 text-muted-foreground/60";

export function PlanCard({
  plan: initialPlan,
  onStatusChange,
  onPlanUpdated,
  onSwitchToChat,
  defaultExpanded = false,
}: {
  plan: Plan;
  onStatusChange: (id: string, status: PlanStatus) => void;
  onPlanUpdated: (planId: string, update: Partial<Plan>) => void;
  onSwitchToChat?: () => void;
  defaultExpanded?: boolean;
}) {
  const { sendFollowup, setHandoffSource, branches, openThread, sendThreadRoundtable, loadBranches, runningThreadIds } = useChatStore();
  const [plan, setPlan] = useState(initialPlan);
  // Sync local plan state when parent re-renders with updated plan (e.g., auto-detect verdict)
  useEffect(() => {
    setPlan(initialPlan);
  }, [initialPlan.status, initialPlan.phase]);
  const [expanded, setExpanded] = useState(defaultExpanded);
  const [subtasks, setSubtasks] = useState<PlanSubtask[] | null>(null);
  const [events, setEvents] = useState<PlanEvent[]>([]);
  const [loading, setLoading] = useState(false);
  const [reviewVerdict, setReviewVerdict] = useState<ParsedReviewVerdict | null>(null);
  const [implComplete, setImplComplete] = useState(false);
  const [showDoc, setShowDoc] = useState(false);
  const [taskFileTitles, setTaskFileTitles] = useState<Record<number, string>>({});
  const statusCfg = PLAN_STATUS_CFG[plan.status];
  const phaseCfg = PLAN_PHASE_CFG[plan.phase] ?? PLAN_PHASE_CFG.drafting;

  const handlePlanUpdate = (update: Partial<Plan>) => {
    setPlan((prev) => ({ ...prev, ...update }));
    onPlanUpdated(plan.id, update);
  };

  // Build plan content for handoff — reused by both forward buttons and handoffSource
  const buildPlanContent = (tasks: PlanSubtask[] | null) => {
    const subtaskSummary = tasks
      ? tasks.map((st) => `- [${st.status}] ${st.title}${st.ownerAgent ? ` (${st.ownerAgent})` : ""}`).join("\n")
      : "";
    return `[Plan: ${plan.title}]\n${plan.description || ""}\n\nSubtasks:\n${subtaskSummary}`;
  };

  const handleToggle = async () => {
    let tasks = subtasks;
    if (!expanded && tasks === null) {
      setLoading(true);
      try {
        const projectKey = useChatStore.getState().selectedProjectKey;
        const result = await loadPlanExpandData(plan, projectKey, runningThreadIds);
        tasks = result.subtasks;
        setSubtasks(result.subtasks);
        const events = await planApi.listPlanEvents(plan.id);
        setEvents(events);
        if (result.implComplete) setImplComplete(true);
        if (result.reviewVerdict) setReviewVerdict(result.reviewVerdict);
        setTaskFileTitles(result.taskFileTitles);
      } catch (e) {
        console.error("[PlanCard] expand load failed:", e);
        tasks = [];
        setSubtasks([]);
      } finally {
        setLoading(false);
      }
    }
    const nextExpanded = !expanded;
    setExpanded(nextExpanded);
    // Set/clear handoff source
    if (nextExpanded) {
      setHandoffSource({ type: "plan", content: buildPlanContent(tasks) });
    } else {
      setHandoffSource(null);
    }
  };

  const handleSubtaskStatus = async (subtaskId: string, status: SubtaskStatus) => {
    try {
      await planApi.updateSubtaskStatus(subtaskId, status);
      setSubtasks((prev) =>
        prev ? prev.map((st) => (st.id === subtaskId ? { ...st, status } : st)) : prev
      );
    } catch (e) {
      console.error("[PlanCard] subtask status update failed:", e);
    }
  };

  const handleOwnerChange = async (subtaskId: string, owner: string | null) => {
    try {
      await planApi.setSubtaskOwner(subtaskId, owner);
      setSubtasks((prev) =>
        prev ? prev.map((st) => (st.id === subtaskId ? { ...st, ownerAgent: owner ?? undefined } : st)) : prev
      );
    } catch (e) {
      console.error("[PlanCard] subtask owner change failed:", e);
    }
  };

  const PLAN_STATUS_CYCLE: PlanStatus[] = ["draft", "active", "done", "abandoned"];
  const nextPlanStatus = PLAN_STATUS_CYCLE[
    (PLAN_STATUS_CYCLE.indexOf(plan.status) + 1) % PLAN_STATUS_CYCLE.length
  ];

  const allStatusActions: { status: PlanStatus; label: string; icon: React.ReactNode; destructive?: boolean }[] = [
    { status: "active", label: "Active로 변경", icon: <Play className={ctxMenuIcon} /> },
    { status: "done", label: "완료 처리", icon: <CheckCircle className={ctxMenuIcon} /> },
    { status: "draft", label: "Draft로 되돌리기", icon: <RotateCcw className={ctxMenuIcon} /> },
    { status: "abandoned", label: "Abandon", icon: <Ban className={ctxMenuIcon} />, destructive: true },
  ];
  const statusActions = allStatusActions.filter((a) => a.status !== plan.status);

  return (
    <ContextMenu.Root>
      <ContextMenu.Trigger asChild>
    <div className="rounded-lg border border-border bg-card transition-colors">
      <div
        className="flex items-start gap-2 p-2.5 cursor-pointer hover:bg-accent/40 rounded-lg transition-colors"
        onClick={handleToggle}
      >
        <span className="mt-0.5 text-muted-foreground shrink-0">
          {expanded ? <ChevronDown className="w-3 h-3" /> : <ChevronRight className="w-3 h-3" />}
        </span>
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-1.5">
            <p className="text-xs font-medium text-foreground leading-snug">{plan.title}</p>
            <button onClick={(e) => { e.stopPropagation(); setShowDoc(true); }} title="문서 보기" className="shrink-0 text-muted-foreground/30 hover:text-primary/60 transition-colors">
              <FileText className="w-3 h-3" />
            </button>
            {plan.phase !== "drafting" && (
              <span className={cn("text-[9px] font-semibold px-1.5 py-0 rounded-full border whitespace-nowrap", phaseCfg.cls)}>
                {phaseCfg.label}
              </span>
            )}
            {(plan.versionMajor > 1 || plan.versionMinor > 0) && (
              <span className="text-[9px] font-mono text-muted-foreground/50 px-1 py-0 rounded bg-accent/50" title={`Version ${plan.versionMajor}.${plan.versionMinor}`}>
                v{plan.versionMajor}.{plan.versionMinor}
              </span>
            )}
            {plan.branchId && (
              <span className="inline-flex items-center gap-0.5 text-[9px] font-medium text-primary bg-primary/10 border border-primary/20 px-1 py-0 rounded-full">
                <GitBranch className="w-2 h-2" />
                branch
              </span>
            )}
          </div>
          {plan.description && !expanded && (
            <p className="text-tf-xs text-muted-foreground mt-0.5 line-clamp-1">{plan.description}</p>
          )}
        </div>
        <button
          title={`Click to → ${nextPlanStatus}`}
          onClick={(e) => { e.stopPropagation(); onStatusChange(plan.id, nextPlanStatus); }}
          className={cn(
            "shrink-0 text-tf-xs font-semibold px-2 py-1 rounded-full border whitespace-nowrap hover:ring-1 hover:ring-primary/30 transition-all",
            statusCfg.cls
          )}
        >
          {statusCfg.label}
        </button>
      </div>

      {expanded && (
        <div className="px-2.5 pb-2.5">
          {plan.description && (
            <p className="text-tf-xs text-muted-foreground mb-2 leading-snug pl-5">{plan.description}</p>
          )}
          {plan.expectedOutcome && (
            <p className="text-tf-xs text-muted-foreground/70 italic mb-2 pl-5 line-clamp-2">
              Goal: {plan.expectedOutcome}
            </p>
          )}
          <div className="pl-5">
            {loading && <p className="text-tf-xs text-muted-foreground">Loading…</p>}
            {!loading && subtasks !== null && subtasks.length === 0 && (
              <AddSubtasksInline plan={plan} onAdded={(newTasks) => setSubtasks(newTasks)} />
            )}
            {!loading && subtasks && subtasks.map((st, idx) => {
              const linked = branches.find((b) => b.subtaskId === st.id);
              return (
                <SubtaskRow
                  key={st.id}
                  subtask={st}
                  planTitle={plan.title}
                  fileTitle={taskFileTitles[idx + 1]}
                  onStatusChange={handleSubtaskStatus}
                  onOwnerChange={handleOwnerChange}
                  onForwardSubtask={(engine, payload) => sendFollowup(engine, "plan", payload)}
                  linkedBranch={linked ? { id: linked.id, label: linked.label, customLabel: linked.customLabel, status: linked.status } : null}
                  onOpenThread={openThread}
                />
              );
            })}
          </div>
          {/* Phase-specific sections */}
          <div className="pl-5">
            {/* Drafting stage — detail design + start review */}
            {plan.phase === "drafting" && subtasks && (
              <DraftingActions plan={plan} subtasks={subtasks} onPlanUpdate={handlePlanUpdate} onSwitchToChat={onSwitchToChat} />
            )}

            {/* Subtask review stage — user reviews subtasks, then advances to approval */}
            {plan.phase === "subtask_review" && subtasks && (
              <div className="mt-2 pt-2 border-t border-border/20 flex items-center gap-2">
                <button
                  onClick={async () => {
                    await planApi.updatePlanPhase(plan.id, "approval");
                    await planApi.createPlanEvent(plan.id, "subtask_review_approved", "user");
                    handlePlanUpdate({ phase: "approval" });
                  }}
                  className="flex items-center gap-1 px-2.5 py-1 rounded-md text-[10px] font-medium bg-status-approved/10 text-status-approved hover:bg-status-approved/20 transition-colors"
                >
                  <Check className="w-3 h-3" />검토 완료 → Dev 승인
                </button>
                <button
                  onClick={async () => {
                    await planApi.updatePlanPhase(plan.id, "drafting");
                    await planApi.createPlanEvent(plan.id, "reverted_to_drafting", "user");
                    handlePlanUpdate({ phase: "drafting" });
                  }}
                  className="flex items-center gap-1 px-2.5 py-1 rounded-md text-[10px] font-medium bg-accent text-muted-foreground hover:text-foreground transition-colors"
                >
                  수정 필요 → Drafting
                </button>
              </div>
            )}

            {/* Approval gate (now simplified — Dev start only) */}
            {plan.phase === "approval" && (
              <ApprovalGate plan={plan} subtasks={subtasks} onPlanUpdate={handlePlanUpdate} />
            )}

            {/* Review branch merge button */}
            {plan.phase === "approval" && plan.reviewBranchId && (
              <div className="mt-1.5 flex items-center gap-2">
                <button onClick={() => openThread(plan.reviewBranchId!)} className="text-[9px] text-primary/60 hover:text-primary hover:underline flex items-center gap-0.5">
                  <GitBranch className="w-2.5 h-2.5" />Review Branch 열기
                </button>
                <MergeBranchButton plan={plan} branchId={plan.reviewBranchId} branchType="review" onPlanUpdate={handlePlanUpdate} />
              </div>
            )}

            {/* Implementation phase — impl-plan display */}
            {plan.phase === "implementation" && plan.implementationBranchId && (
              <>
                <div className="mt-1.5 flex items-center gap-2">
                  <button onClick={() => openThread(plan.implementationBranchId!)} className="text-[9px] text-primary/60 hover:text-primary hover:underline flex items-center gap-0.5">
                    <GitBranch className="w-2.5 h-2.5" />Implementation Branch 열기
                  </button>
                </div>
                {implComplete && !runningThreadIds.includes(`branch:${plan.implementationBranchId}`) && (
                  <div className="mt-2 rounded-md border border-status-approved/30 bg-status-approved/5 p-2 text-tf-xs text-status-approved flex items-center gap-1.5">
                    <Check className="w-3 h-3" />구현 완료 — Review 단계로 전환 가능
                    <button
                      onClick={async () => {
                        // 진입 게이트 + roleAssignments 해석으로 model 포함 전달
                        const { assertRoleReady, loadRoleAssignments, resolveRoleProfiles } = await import("@/lib/roleAssignments");
                        const allProfiles = useChatStore.getState().agentProfiles;
                        const gate = await assertRoleReady("reviewers", allProfiles);
                        if (!gate.ok) return;
                        const assignments = await loadRoleAssignments();
                        const reviewerProfiles = resolveRoleProfiles("reviewers", assignments, allProfiles);
                        const reviewers = reviewerProfiles.map((p) => ({ engine: p.engine, model: p.model, name: p.label }));

                        const implShadow = `branch:${plan.implementationBranchId}`;
                        const msgs = await invoke<Message[]>("list_messages", { conversationId: implShadow });
                        // Run project tests and pass results to Reviewer
                        let testOutput: string | undefined;
                        try {
                          const projectKey = useChatStore.getState().selectedProjectKey;
                          if (projectKey) {
                            const project = await invoke<{ path?: string }>("get_project", { key: projectKey });
                            if (project?.path) {
                              const { runProjectTests } = await import("@/lib/api/testRunner");
                              const result = await runProjectTests(project.path);
                              testOutput = result.output;
                            }
                          }
                        } catch (e) { console.debug("[test-before-review]", e); }
                        const { branch, participants, prompt, mode } = await startReviewRT(plan, msgs, testOutput, reviewers);
                        handlePlanUpdate({ phase: "review", reviewBranchId: branch.id });
                        await loadBranches(plan.conversationId);
                        await openThread(branch.id);
                        await sendThreadRoundtable(prompt, participants, mode, { autoSynthesize: true });
                      }}
                      className="ml-auto px-2 py-0.5 rounded text-[9px] font-medium bg-status-approved/20 hover:bg-status-approved/30 transition-colors"
                    >
                      Review RT 시작
                    </button>
                  </div>
                )}
              </>
            )}

            {/* Review phase — verdict display */}
            {plan.phase === "review" && (
              <>
                {plan.reviewBranchId && (
                  <div className="mt-1.5">
                    <button onClick={() => openThread(plan.reviewBranchId!)} className="text-[9px] text-primary/60 hover:text-primary hover:underline flex items-center gap-0.5">
                      <GitBranch className="w-2.5 h-2.5" />Review Branch 열기
                    </button>
                  </div>
                )}
                {reviewVerdict && (
                  <ReviewVerdictCard verdict={reviewVerdict} plan={plan} onPlanUpdate={handlePlanUpdate} />
                )}
                {/* Fallback: no verdict marker detected — let user manually decide */}
                {!reviewVerdict && !runningThreadIds.includes(`branch:${plan.reviewBranchId}`) && (
                  <div className="mt-2 rounded-md border border-muted/40 bg-muted/5 p-2.5 space-y-1.5">
                    <p className="text-[9px] text-muted-foreground/60">리뷰어 마커가 감지되지 않았습니다. 수동으로 판단하세요.</p>
                    <div className="flex items-center gap-2 flex-wrap">
                      <button
                        onClick={async () => {
                          // Review 진입 자체가 실패한 경우 — 한 단계 전(dev 완료 상태)으로 되돌려
                          // 사용자가 Review RT 를 다시 시작할 수 있게 한다. 기존 review 브랜치는
                          // archive (재사용되지 않고 다음 시작 시 새 브랜치 생성됨). s37
                          if (plan.reviewBranchId) {
                            await invoke("archive_branch", { id: plan.reviewBranchId }).catch(() => {});
                          }
                          await planApi.updatePlanPhase(plan.id, "implementation");
                          await planApi.createPlanEvent(
                            plan.id, "review_rolled_back", "user",
                            "Review 진입 실패로 Dev 단계 복귀",
                          );
                          handlePlanUpdate({ phase: "implementation" });
                        }}
                        className="px-2.5 py-1 rounded-md text-[10px] font-medium bg-primary/10 text-primary hover:bg-primary/20 transition-colors"
                        title="Review RT 진입이 실패했을 때 Dev 완료 상태로 되돌립니다. 기존 review 브랜치는 archive."
                      >
                        ← Dev 단계로 복귀 (Review 재시도)
                      </button>
                      <button
                        onClick={async () => {
                          await planApi.updatePlanPhase(plan.id, "rework");
                          await planApi.createPlanEvent(plan.id, "review_failed", "reviewer", JSON.stringify({ verdict: "fail", findings: [], recommendations: [] }));
                          handlePlanUpdate({ phase: "rework" });
                        }}
                        className="px-2.5 py-1 rounded-md text-[10px] font-medium bg-status-rejected/10 text-status-rejected hover:bg-status-rejected/20 transition-colors"
                      >
                        수정 필요 (Rework)
                      </button>
                      <button
                        onClick={async () => {
                          await planApi.updatePlanPhase(plan.id, "done");
                          await planApi.updatePlanStatus(plan.id, "done");
                          await planApi.createPlanEvent(plan.id, "review_passed", "reviewer", JSON.stringify({ verdict: "pass", findings: [], recommendations: [] }));
                          handlePlanUpdate({ phase: "done", status: "done" });
                        }}
                        className="px-2.5 py-1 rounded-md text-[10px] font-medium bg-status-approved/10 text-status-approved hover:bg-status-approved/20 transition-colors"
                      >
                        통과 처리 (Done)
                      </button>
                    </div>
                  </div>
                )}
              </>
            )}

            {/* Rework phase — send findings to Developer or return to Subtask */}
            {plan.phase === "rework" && plan.implementationBranchId && (
              <div className="mt-2 rounded-md border border-status-rejected/30 bg-status-rejected/5 p-2.5 text-tf-xs text-status-rejected space-y-2">
                <p>Rework 필요 — Review findings를 Developer에게 전달합니다.</p>
                {(() => {
                  const failCount = events.filter((e) => e.eventType === "review_failed").length;
                  return failCount >= 2 ? (
                    <p className="text-[9px] font-medium text-amber-500">
                      ⚠ Review 실패 {failCount}회 — {failCount >= 3 ? "설계 재검토가 필요합니다." : "다음 실패 시 설계 재검토로 에스컬레이션됩니다."}
                    </p>
                  ) : null;
                })()}
                {reviewVerdict && reviewVerdict.findings.length > 0 && (
                  <ul className="space-y-0.5 text-[9px] text-foreground/60 pl-2">
                    {reviewVerdict.findings.map((f, i) => <li key={i}>- {f}</li>)}
                  </ul>
                )}
                <div className="flex items-center gap-2">
                  <button
                    onClick={async () => {
                      await planApi.updatePlanPhase(plan.id, "implementation");
                      await planApi.createPlanEvent(plan.id, "rework_requested", "user");
                      handlePlanUpdate({ phase: "implementation" });
                      await openThread(plan.implementationBranchId!);
                      // Send review findings to Developer with budget pressure
                      const findings = reviewVerdict?.findings.join("\n- ") ?? "";
                      const failCount = events.filter((e) => e.eventType === "review_failed").length;
                      const pressure = failCount >= 2
                        ? `\n> ⚠️ 이전 ${failCount}회 Review 실패. ${failCount >= 3 ? "이번이 마지막 기회입니다." : "다음 실패 시 설계 재검토로 에스컬레이션됩니다."}`
                        : "";
                      const reworkPrompt = [
                        `[Rework] Review에서 다음 사항이 지적되었습니다:`,
                        findings ? `- ${findings}` : "(findings 없음)",
                        "",
                        `위 사항을 수정하고 완료되면 알려주세요.`,
                        pressure,
                      ].filter(Boolean).join("\n");
                      const shadowConvId = `branch:${plan.implementationBranchId}`;
                      const saved = useChatStore.getState().getConversationEngine(shadowConvId);
                      await useChatStore.getState().sendThreadMessage(reworkPrompt, saved?.engine ?? "claude", saved?.model ?? undefined);
                    }}
                    className="px-2.5 py-1 rounded-md text-tf-xs font-medium bg-primary/10 text-primary hover:bg-primary/20 transition-colors"
                  >
                    Developer에게 전달 + Rework
                  </button>
                  <button
                    onClick={async () => {
                      await planApi.updatePlanPhase(plan.id, "subtask_review");
                      await planApi.createPlanEvent(plan.id, "reverted_to_subtask_review", "user", "Design change needed");
                      handlePlanUpdate({ phase: "subtask_review" as any });
                    }}
                    className="px-2.5 py-1 rounded-md text-tf-xs font-medium bg-accent text-muted-foreground hover:text-foreground transition-colors"
                  >
                    설계 변경 → Subtask
                  </button>
                </div>
              </div>
            )}
          </div>


          {/* Unified timeline (events + branches, collapsed by default) */}
          {(() => {
            const planBranches = branches.filter(
              (b) => b.id === plan.implementationBranchId || b.id === plan.reviewBranchId
                || b.label?.startsWith("dev:") || b.label?.startsWith("Impl:") || b.label?.startsWith("Dev:") || b.label?.startsWith("review:") || b.label?.startsWith("Review:")
            ).filter((b) => b.conversationId === plan.conversationId);
            return (events.length > 0 || planBranches.length > 0) ? (
              <div className="pl-5">
                <EventTimeline events={events} branches={planBranches} onOpenBranch={openThread} />
              </div>
            ) : null;
          })()}
        </div>
      )}
      {showDoc && <PlanDocumentModal plan={plan} onClose={() => setShowDoc(false)} />}
    </div>
      </ContextMenu.Trigger>
      <ContextMenu.Portal>
        <ContextMenu.Content className={ctxMenuContent}>
          <ContextMenu.Item className={ctxMenuItem} onSelect={() => setShowDoc(true)}>
            <FileText className={ctxMenuIcon} /> 문서 보기
          </ContextMenu.Item>
          <ContextMenu.Item className={ctxMenuItem} onSelect={handleToggle}>
            {expanded ? <ChevronDown className={ctxMenuIcon} /> : <ChevronRight className={ctxMenuIcon} />}
            {expanded ? "접기" : "펼치기"}
          </ContextMenu.Item>
          <ContextMenu.Separator className={ctxMenuSeparator} />
          {statusActions.map((a) => (
            <ContextMenu.Item
              key={a.status}
              className={a.destructive ? ctxMenuDestructive : ctxMenuItem}
              onSelect={() => {
                onStatusChange(plan.id, a.status);
                if (a.status === "done") handlePlanUpdate({ status: "done", phase: "done" as PlanPhase });
              }}
            >
              {a.icon} {a.label}
            </ContextMenu.Item>
          ))}
          <ContextMenu.Separator className={ctxMenuSeparator} />
          <ContextMenu.Label className="px-2.5 py-1 text-[9px] text-muted-foreground/40 font-medium">Phase 전환</ContextMenu.Label>
          {(["drafting", "approval", "implementation", "review", "done"] as PlanPhase[])
            .filter((p) => p !== plan.phase)
            .map((phase) => (
              <ContextMenu.Item
                key={phase}
                className={ctxMenuItem}
                onSelect={async () => {
                  await planApi.updatePlanPhase(plan.id, phase);
                  await planApi.createPlanEvent(plan.id, "phase_manual_override", "user", `→ ${phase}`);
                  handlePlanUpdate({ phase });
                  if (phase === "done") {
                    onStatusChange(plan.id, "done");
                    handlePlanUpdate({ status: "done" });
                  }
                }}
              >
                <span className={cn("w-1.5 h-1.5 rounded-full shrink-0", PLAN_PHASE_CFG[phase]?.cls?.split(" ")[0] ?? "bg-muted")} />
                {PLAN_PHASE_CFG[phase]?.label ?? phase}
              </ContextMenu.Item>
            ))}
        </ContextMenu.Content>
      </ContextMenu.Portal>
    </ContextMenu.Root>
  );
}

// ─── AddSubtasksInline ─────────────────────────────────────────────────────

/** Extract first H1/H2 heading from markdown as the task title. */
function extractHeading(md: string): string | null {
  const m = md.match(/^#{1,2}\s+(.+)/m);
  return m ? m[1].replace(/^(Task\s+\d+[:.]\s*)/i, "").trim() : null;
}

function AddSubtasksInline({ plan, onAdded }: { plan: Plan; onAdded: (tasks: PlanSubtask[]) => void }) {
  const [open, setOpen] = useState(false);
  const [text, setText] = useState("");
  const [saving, setSaving] = useState(false);
  const [parsing, setParsing] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const selectedProjectKey = useChatStore((s) => s.selectedProjectKey);
  const projects = useChatStore((s) => s.projects);

  const handleParseFromDocs = async () => {
    const project = projects.find((p) => p.key === selectedProjectKey);
    if (!project?.path || !plan.slug) return;
    setParsing(true);
    try {
      // List docs/plans/ directory and find task files for this plan
      const entries = await invoke<{ name: string; path: string; isDir: boolean }[]>(
        "list_directory", { path: `${project.path}/docs/plans` }
      ).catch(() => [] as { name: string; path: string; isDir: boolean }[]);

      const taskFiles = entries
        .filter((e) => !e.isDir && e.name.match(new RegExp(`^${plan.slug}-task-\\d+\\.md$`)))
        .sort((a, b) => a.name.localeCompare(b.name));

      if (taskFiles.length === 0) {
        // Fall back to main plan doc for inline subtask list
        const mainDoc = await invoke<string>("read_file_content", {
          path: `${project.path}/docs/plans/${plan.slug}.md`
        }).catch(() => "");
        // Extract numbered list items as subtasks
        const items = [...mainDoc.matchAll(/^\d+\.\s+(.+)/gm)].map((m) => m[1].trim());
        if (items.length > 0) setText(items.join("\n"));
      } else {
        // Read each task file and extract its heading as the subtask title
        const titles: string[] = [];
        for (const f of taskFiles) {
          const content = await invoke<string>("read_file_content", { path: f.path }).catch(() => "");
          const title = extractHeading(content) ?? f.name.replace(/\.md$/, "");
          titles.push(title);
        }
        setText(titles.join("\n"));
      }
    } finally {
      setParsing(false);
      setTimeout(() => textareaRef.current?.focus(), 50);
    }
  };

  const handleSave = async () => {
    const lines = text.split("\n").map((l) => l.replace(/^[-*\d.)\s]+/, "").trim()).filter(Boolean);
    if (lines.length === 0) return;
    setSaving(true);
    try {
      const subtasks = await planApi.replacePlanSubtasks(plan.id, lines.map((title) => ({ title, details: undefined })));
      onAdded(subtasks);
    } catch (e) {
      console.error("[AddSubtasks] failed:", e);
    } finally {
      setSaving(false);
    }
  };

  if (!open) {
    return (
      <button
        onClick={() => { setOpen(true); setTimeout(() => textareaRef.current?.focus(), 50); }}
        className="flex items-center gap-1 text-tf-xs text-muted-foreground/40 hover:text-primary/60 transition-colors py-0.5"
      >
        <Plus className="w-3 h-3" /> 서브태스크 추가
      </button>
    );
  }

  return (
    <div className="space-y-1.5 mt-1">
      <div className="flex items-center justify-between">
        <p className="text-tf-xs text-muted-foreground/50">한 줄에 하나씩 (마크다운 리스트 형식 OK)</p>
        {plan.slug && (
          <button
            onClick={handleParseFromDocs}
            disabled={parsing}
            className="flex items-center gap-1 text-tf-xs text-primary/50 hover:text-primary transition-colors disabled:opacity-40"
          >
            {parsing ? <Loader2 className="w-3 h-3 animate-spin" /> : <FolderOpen className="w-3 h-3" />}
            docs에서 가져오기
          </button>
        )}
      </div>
      <textarea
        ref={textareaRef}
        value={text}
        onChange={(e) => setText(e.target.value)}
        placeholder={"1. 첫 번째 작업\n2. 두 번째 작업\n3. 세 번째 작업"}
        rows={5}
        className="w-full bg-input rounded-md px-2.5 py-1.5 text-tf-sm outline-none text-foreground placeholder:text-muted-foreground/30 border border-border/30 focus:border-ring/40 resize-none font-mono"
      />
      <div className="flex gap-1.5">
        <button
          onClick={handleSave}
          disabled={saving || !text.trim()}
          className="flex items-center gap-1 px-2.5 py-1 rounded-md text-tf-xs font-medium bg-primary/10 text-primary hover:bg-primary/20 disabled:opacity-40 transition-colors"
        >
          {saving ? <Loader2 className="w-3 h-3 animate-spin" /> : <Check className="w-3 h-3" />}
          저장
        </button>
        <button
          onClick={() => { setOpen(false); setText(""); }}
          className="px-2 py-1 rounded-md text-tf-xs text-muted-foreground hover:text-foreground transition-colors"
        >
          취소
        </button>
      </div>
    </div>
  );
}
