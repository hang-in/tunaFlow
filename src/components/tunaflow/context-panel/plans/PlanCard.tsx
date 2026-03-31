import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { ChevronDown, ChevronRight, GitBranch, Check, FileText } from "lucide-react";
import type { Plan, PlanEvent, PlanPhase, PlanSubtask, PlanStatus, SubtaskStatus, Message } from "@/types";
import * as planApi from "@/lib/api/plans";
import { scanMessagesForMarkers, startReviewRT } from "@/lib/workflowOrchestration";
import type { ParsedReviewVerdict } from "@/lib/planProposalParser";
import { PLAN_STATUS_CFG, PLAN_PHASE_CFG, OWNER_OPTIONS } from "./constants";
import { PlanDocumentModal } from "../PlanDocumentModal";
import { SubtaskRow } from "./SubtaskRow";
import { EventTimeline } from "./EventTimeline";
import { DraftingActions } from "./DraftingActions";
import { ApprovalGate } from "./ApprovalGate";
import { ReviewVerdictCard } from "./ReviewVerdictCard";
import { MergeBranchButton } from "./MergeBranchButton";

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
      } catch {
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
    } catch {
      // silent
    }
  };

  const handleOwnerChange = async (subtaskId: string, owner: string | null) => {
    try {
      await planApi.setSubtaskOwner(subtaskId, owner);
      setSubtasks((prev) =>
        prev ? prev.map((st) => (st.id === subtaskId ? { ...st, ownerAgent: owner ?? undefined } : st)) : prev
      );
    } catch {
      // silent
    }
  };

  const PLAN_STATUS_CYCLE: PlanStatus[] = ["draft", "active", "done", "abandoned"];
  const nextPlanStatus = PLAN_STATUS_CYCLE[
    (PLAN_STATUS_CYCLE.indexOf(plan.status) + 1) % PLAN_STATUS_CYCLE.length
  ];

  return (
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
            {plan.revision > 0 && (
              <span className="text-[8px] font-mono text-muted-foreground/50 px-1 py-0 rounded bg-accent/50" title={`Revision ${plan.revision}`}>
                rev.{plan.revision}
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
            "shrink-0 text-[9px] font-semibold px-1.5 py-0.5 rounded-full border whitespace-nowrap",
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
            {!loading && subtasks && subtasks.map((st) => {
              const linked = branches.find((b) => b.subtaskId === st.id);
              return (
                <SubtaskRow
                  key={st.id}
                  subtask={st}
                  planTitle={plan.title}
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

            {/* Rework phase — back to implementation */}
            {plan.phase === "rework" && plan.implementationBranchId && (
              <div className="mt-2 rounded-md border border-status-rejected/30 bg-status-rejected/5 p-2 text-[10px] text-status-rejected">
                <p className="mb-1.5">Rework 필요 — Review findings를 반영하세요.</p>
                <button
                  onClick={async () => {
                    await planApi.updatePlanPhase(plan.id, "implementation");
                    await planApi.createPlanEvent(plan.id, "rework_requested", "user");
                    handlePlanUpdate({ phase: "implementation" });
                    await openThread(plan.implementationBranchId!);
                  }}
                  className="px-2.5 py-1 rounded-md text-[10px] font-medium bg-primary/10 text-primary hover:bg-primary/20 transition-colors"
                >
                  Implementation Branch로 돌아가기
                </button>
              </div>
            )}
          </div>

          {/* Plan forward — send plan context to an agent */}
          <div className="flex items-center gap-1.5 pl-5 pt-1.5 mt-1 border-t border-border/20">
            <span className="text-[9px] text-muted-foreground/50">Forward plan:</span>
            {OWNER_OPTIONS.map((eng) => (
              <button
                key={eng}
                onClick={(e) => {
                  e.stopPropagation();
                  sendFollowup(eng, "plan", buildPlanContent(subtasks));
                }}
                className="text-[9px] text-primary/60 hover:text-primary hover:underline"
              >
                → {eng}
              </button>
            ))}
          </div>

          {/* Event timeline */}
          {events.length > 0 && (
            <div className="pl-5">
              <EventTimeline events={events} />
            </div>
          )}
        </div>
      )}
      {showDoc && <PlanDocumentModal plan={plan} onClose={() => setShowDoc(false)} />}
    </div>
  );
}
