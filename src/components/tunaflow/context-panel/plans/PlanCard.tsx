import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import * as ContextMenu from "@radix-ui/react-context-menu";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { ChevronDown, ChevronRight, GitBranch, Check, FileText, Ban, CheckCircle, Play, RotateCcw } from "lucide-react";
import type { Plan, PlanEvent, PlanPhase, PlanSubtask, PlanStatus, SubtaskStatus, Message } from "@/types";
import * as planApi from "@/lib/api/plans";
import { getPlanSlug, scanMessagesForMarkers, startReviewRT } from "@/lib/workflowOrchestration";
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
const ctxMenuItem = "flex items-center gap-2 px-2.5 py-1.5 text-[12px] rounded-md cursor-pointer outline-none transition-colors text-foreground/80 data-[highlighted]:bg-accent data-[highlighted]:text-foreground";
const ctxMenuDestructive = "flex items-center gap-2 px-2.5 py-1.5 text-[12px] rounded-md cursor-pointer outline-none transition-colors text-destructive/70 data-[highlighted]:bg-destructive/10 data-[highlighted]:text-destructive";
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
  const { sendFollowup, setHandoffSource, branches, openThread, loadBranches } = useChatStore();
  const [plan, setPlan] = useState(initialPlan);
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
        const [t, e] = await Promise.all([
          planApi.listSubtasks(plan.id),
          planApi.listPlanEvents(plan.id),
        ]);
        tasks = t;
        setSubtasks(tasks);
        setEvents(e);

        // Scan branch messages for workflow markers
        if (plan.implementationBranchId && (plan.phase === "implementation" || plan.phase === "review")) {
          try {
            const shadowConvId = `branch:${plan.implementationBranchId}`;
            const msgs = await invoke<Message[]>("list_messages", { conversationId: shadowConvId });
            const markers = scanMessagesForMarkers(msgs);
            if (markers.implComplete) setImplComplete(true);
          } catch { /* branch may not exist yet */ }
        }
        if (plan.reviewBranchId && plan.phase === "review") {
          try {
            const shadowConvId = `branch:${plan.reviewBranchId}`;
            const msgs = await invoke<Message[]>("list_messages", { conversationId: shadowConvId });
            const markers = scanMessagesForMarkers(msgs);
            if (markers.reviewVerdict) setReviewVerdict(markers.reviewVerdict);
          } catch { /* branch may not exist yet */ }
        }
        // Load task file titles
        try {
          const projectKey = useChatStore.getState().selectedProjectKey;
          if (projectKey) {
            const project = await invoke("get_project", { key: projectKey }) as { path?: string };
            if (project?.path) {
              const slug = getPlanSlug(plan);
              const titles: Record<number, string> = {};
              for (let i = 1; i <= (tasks?.length ?? 0); i++) {
                const taskPath = `${project.path}/docs/plans/${slug}-task-${String(i).padStart(2, "0")}.md`;
                try {
                  const content = await invoke<{ content: string }>("read_text_file", { filePath: taskPath, projectPath: project.path });
                  const m = content.content.match(/^#\s+(.+)$/m);
                  if (m) titles[i] = m[1].trim();
                } catch { /* file doesn't exist */ }
              }
              setTaskFileTitles(titles);
            }
          }
        } catch (e) {
          console.warn("[PlanCard] task file title load failed:", e);
        }
      } catch (e) {
        console.error("[PlanCard] subtask/event load failed:", e);
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
              <span className={cn("text-[8px] font-semibold px-1.5 py-0 rounded-full border whitespace-nowrap", phaseCfg.cls)}>
                {phaseCfg.label}
              </span>
            )}
            {(plan.versionMajor > 1 || plan.versionMinor > 0) && (
              <span className="text-[8px] font-mono text-muted-foreground/50 px-1 py-0 rounded bg-accent/50" title={`Version ${plan.versionMajor}.${plan.versionMinor}`}>
                v{plan.versionMajor}.{plan.versionMinor}
              </span>
            )}
            {plan.branchId && (
              <span className="inline-flex items-center gap-0.5 text-[8px] font-medium text-primary bg-primary/10 border border-primary/20 px-1 py-0 rounded-full">
                <GitBranch className="w-2 h-2" />
                branch
              </span>
            )}
          </div>
          {plan.description && !expanded && (
            <p className="text-[10px] text-muted-foreground mt-0.5 line-clamp-1">{plan.description}</p>
          )}
        </div>
        <button
          title={`Click to → ${nextPlanStatus}`}
          onClick={(e) => { e.stopPropagation(); onStatusChange(plan.id, nextPlanStatus); }}
          className={cn(
            "shrink-0 text-[10px] font-semibold px-2 py-1 rounded-full border whitespace-nowrap hover:ring-1 hover:ring-primary/30 transition-all",
            statusCfg.cls
          )}
        >
          {statusCfg.label}
        </button>
      </div>

      {expanded && (
        <div className="px-2.5 pb-2.5">
          {plan.description && (
            <p className="text-[10px] text-muted-foreground mb-2 leading-snug pl-5">{plan.description}</p>
          )}
          {plan.expectedOutcome && (
            <p className="text-[10px] text-muted-foreground/70 italic mb-2 pl-5 line-clamp-2">
              Goal: {plan.expectedOutcome}
            </p>
          )}
          <div className="pl-5">
            {loading && <p className="text-[10px] text-muted-foreground">Loading…</p>}
            {!loading && subtasks !== null && subtasks.length === 0 && (
              <p className="text-[10px] text-muted-foreground">No subtasks.</p>
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
                {implComplete && (
                  <div className="mt-2 rounded-md border border-status-approved/30 bg-status-approved/5 p-2 text-[10px] text-status-approved flex items-center gap-1.5">
                    <Check className="w-3 h-3" />구현 완료 — Review 단계로 전환 가능
                    <button
                      onClick={async () => {
                        const implShadow = `branch:${plan.implementationBranchId}`;
                        const msgs = await invoke<Message[]>("list_messages", { conversationId: implShadow });
                        const { branch } = await startReviewRT(plan, msgs);
                        handlePlanUpdate({ phase: "review", reviewBranchId: branch.id });
                        await loadBranches(plan.conversationId);
                        await openThread(branch.id);
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
              </>
            )}

            {/* Rework phase — send findings to Developer or return to Subtask */}
            {plan.phase === "rework" && plan.implementationBranchId && (
              <div className="mt-2 rounded-md border border-status-rejected/30 bg-status-rejected/5 p-2.5 text-[10px] text-status-rejected space-y-2">
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
                    className="px-2.5 py-1 rounded-md text-[10px] font-medium bg-primary/10 text-primary hover:bg-primary/20 transition-colors"
                  >
                    Developer에게 전달 + Rework
                  </button>
                  <button
                    onClick={async () => {
                      await planApi.updatePlanPhase(plan.id, "subtask_review");
                      await planApi.createPlanEvent(plan.id, "reverted_to_subtask_review", "user", "Design change needed");
                      handlePlanUpdate({ phase: "subtask_review" as any });
                    }}
                    className="px-2.5 py-1 rounded-md text-[10px] font-medium bg-accent text-muted-foreground hover:text-foreground transition-colors"
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
                || b.label?.startsWith("Impl:") || b.label?.startsWith("Review:") || b.label?.startsWith("Re-review:")
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
              onSelect={() => onStatusChange(plan.id, a.status)}
            >
              {a.icon} {a.label}
            </ContextMenu.Item>
          ))}
        </ContextMenu.Content>
      </ContextMenu.Portal>
    </ContextMenu.Root>
  );
}
