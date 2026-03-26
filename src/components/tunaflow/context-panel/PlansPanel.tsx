import { useState, useEffect } from "react";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { ChevronDown, ChevronRight, Plus, ClipboardList, X, GitBranch } from "lucide-react";
import type { Plan, PlanSubtask, PlanStatus, SubtaskStatus, SubtaskInput } from "@/types";
import * as planApi from "@/lib/api/plans";

// ─── Status configs ──────────────────────────────────────────────────────────

const PLAN_STATUS_CFG: Record<PlanStatus, { label: string; cls: string }> = {
  draft:     { label: "draft",     cls: "text-muted-foreground bg-accent border-border" },
  active:    { label: "active",    cls: "text-primary bg-primary/10 border-primary/20" },
  done:      { label: "done",      cls: "text-status-approved bg-status-approved/10 border-status-approved/20" },
  abandoned: { label: "abandoned", cls: "text-status-rejected bg-status-rejected/10 border-status-rejected/20" },
};

const SUBTASK_STATUS_CFG: Record<SubtaskStatus, { label: string; next: SubtaskStatus; cls: string }> = {
  todo:        { label: "todo",        next: "approved",     cls: "text-muted-foreground bg-accent border-border" },
  approved:    { label: "approved",    next: "in_progress",  cls: "text-agent-gemini bg-agent-gemini/10 border-agent-gemini/20" },
  in_progress: { label: "in progress", next: "done",         cls: "text-primary bg-primary/10 border-primary/20" },
  done:        { label: "done",        next: "todo",         cls: "text-status-approved bg-status-approved/10 border-status-approved/20" },
  abandoned:   { label: "abandoned",   next: "todo",         cls: "text-status-rejected bg-status-rejected/10 border-status-rejected/20" },
};

const INPUT_CLS =
  "w-full bg-input rounded-md px-2.5 py-1.5 text-xs outline-none text-foreground " +
  "placeholder:text-muted-foreground border border-border focus:border-ring/50";

// ─── CreatePlanForm ──────────────────────────────────────────────────────────

type PlanScope = "conversation" | "branch";

function CreatePlanForm({
  conversationId,
  activeBranchId,
  onCreated,
  onCancel,
}: {
  conversationId: string;
  activeBranchId: string | null;
  onCreated: (plan: Plan) => void;
  onCancel: () => void;
}) {
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [expectedOutcome, setExpectedOutcome] = useState("");
  const [subtasks, setSubtasks] = useState<SubtaskInput[]>([]);
  const [newSubtask, setNewSubtask] = useState("");
  const [saving, setSaving] = useState(false);
  const [scope, setScope] = useState<PlanScope>(activeBranchId ? "branch" : "conversation");

  const addSubtask = () => {
    const t = newSubtask.trim();
    if (!t) return;
    setSubtasks((prev) => [...prev, { title: t }]);
    setNewSubtask("");
  };

  const removeSubtask = (idx: number) => {
    setSubtasks((prev) => prev.filter((_, i) => i !== idx));
  };

  const handleCreate = async () => {
    if (!title.trim()) return;
    setSaving(true);
    try {
      const plan = await planApi.createPlan({
        conversationId,
        branchId: scope === "branch" && activeBranchId ? activeBranchId : undefined,
        title: title.trim(),
        description: description.trim() || undefined,
        expectedOutcome: expectedOutcome.trim() || undefined,
        subtasks,
      });
      onCreated(plan);
    } catch {
      // silent — user can retry
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="rounded-lg border border-border bg-card p-3 space-y-2">
      {/* Scope toggle — only shown in branch stream */}
      {activeBranchId && (
        <div className="flex items-center gap-1 p-0.5 rounded-md bg-accent/50">
          {(["conversation", "branch"] as PlanScope[]).map((s) => (
            <button
              key={s}
              onClick={() => setScope(s)}
              className={cn(
                "flex-1 flex items-center justify-center gap-1 px-2 py-1 rounded text-[10px] font-medium transition-colors",
                scope === s ? "bg-card text-foreground shadow-sm" : "text-muted-foreground"
              )}
            >
              {s === "branch" && <GitBranch className="w-2.5 h-2.5" />}
              {s === "conversation" ? "Conversation" : "This Branch"}
            </button>
          ))}
        </div>
      )}
      <input
        placeholder="Plan title *"
        value={title}
        onChange={(e) => setTitle(e.target.value)}
        className={INPUT_CLS}
        autoFocus
      />
      <textarea
        placeholder="Description (optional)"
        value={description}
        onChange={(e) => setDescription(e.target.value)}
        rows={2}
        className={`${INPUT_CLS} resize-none`}
      />
      <textarea
        placeholder="Expected outcome (optional)"
        value={expectedOutcome}
        onChange={(e) => setExpectedOutcome(e.target.value)}
        rows={2}
        className={`${INPUT_CLS} resize-none`}
      />

      {subtasks.length > 0 && (
        <div className="space-y-1">
          {subtasks.map((st, i) => (
            <div key={i} className="flex items-center gap-1.5">
              <span className="text-[10px] text-muted-foreground shrink-0">{i + 1}.</span>
              <span className="flex-1 text-[11px] text-foreground truncate">{st.title}</span>
              <button
                onClick={() => removeSubtask(i)}
                className="shrink-0 text-muted-foreground hover:text-destructive transition-colors"
              >
                <X className="w-3 h-3" />
              </button>
            </div>
          ))}
        </div>
      )}

      <div className="flex gap-1.5">
        <input
          placeholder="Add subtask…"
          value={newSubtask}
          onChange={(e) => setNewSubtask(e.target.value)}
          onKeyDown={(e) => { if (e.key === "Enter") { e.preventDefault(); addSubtask(); } }}
          className={`${INPUT_CLS} flex-1`}
        />
        <button
          onClick={addSubtask}
          className="shrink-0 px-2 py-1.5 rounded-md bg-accent text-muted-foreground hover:text-foreground text-xs transition-colors border border-border"
        >
          <Plus className="w-3.5 h-3.5" />
        </button>
      </div>

      <div className="flex gap-2 pt-1">
        <button
          onClick={handleCreate}
          disabled={saving || !title.trim()}
          className="flex-1 px-2 py-1.5 rounded-md bg-primary/15 text-primary text-xs hover:bg-primary/25 transition-colors disabled:opacity-40"
        >
          {saving ? "Creating…" : "Create"}
        </button>
        <button
          onClick={onCancel}
          className="px-2 py-1.5 rounded-md text-muted-foreground text-xs hover:bg-accent transition-colors"
        >
          Cancel
        </button>
      </div>
    </div>
  );
}

const OWNER_OPTIONS = ["claude", "codex", "gemini", "opencode"];

// ─── SubtaskRow ──────────────────────────────────────────────────────────────

function SubtaskRow({
  subtask,
  onStatusChange,
  onOwnerChange,
  linkedBranch,
  onOpenThread,
}: {
  subtask: PlanSubtask;
  onStatusChange: (id: string, status: SubtaskStatus) => void;
  onOwnerChange: (id: string, owner: string | null) => void;
  linkedBranch?: { id: string; label: string; customLabel?: string; status: string } | null;
  onOpenThread?: (branchId: string) => void;
}) {
  const cfg = SUBTASK_STATUS_CFG[subtask.status];
  return (
    <div className="flex items-start gap-2 py-1.5 border-b border-border/30 last:border-0">
      <button
        title={`Click to → ${cfg.next}`}
        onClick={() => onStatusChange(subtask.id, cfg.next)}
        className={cn(
          "shrink-0 mt-0.5 text-[9px] font-semibold px-1.5 py-0.5 rounded-full border whitespace-nowrap",
          cfg.cls
        )}
      >
        {cfg.label}
      </button>
      <div className="flex-1 min-w-0">
        <p className="text-[11px] text-foreground leading-snug">{subtask.title}</p>
        {subtask.details && (
          <p className="text-[10px] text-muted-foreground leading-snug mt-0.5 line-clamp-2">{subtask.details}</p>
        )}
        <div className="flex items-center gap-1.5 mt-0.5 flex-wrap">
          <select
            value={subtask.ownerAgent || ""}
            onChange={(e) => onOwnerChange(subtask.id, e.target.value || null)}
            className="text-[9px] bg-transparent border border-border/30 rounded px-1 py-0 text-muted-foreground/70 outline-none"
            title="Owner agent"
          >
            <option value="">no owner</option>
            {OWNER_OPTIONS.map((o) => <option key={o} value={o}>{o}</option>)}
          </select>
          {subtask.lastUpdatedBy && (
            <span className="text-[9px] text-muted-foreground/50">by: {subtask.lastUpdatedBy}</span>
          )}
          {linkedBranch && (
            <button
              onClick={() => onOpenThread?.(linkedBranch.id)}
              className="inline-flex items-center gap-0.5 text-[8px] font-medium text-primary/60 bg-primary/6 hover:bg-primary/12 px-1 py-0 rounded transition-colors"
              title={`Open branch: ${linkedBranch.customLabel ?? linkedBranch.label}`}
            >
              <GitBranch className="w-2 h-2" />
              {linkedBranch.customLabel ?? linkedBranch.label}
              <span className="text-muted-foreground/40 ml-0.5">{linkedBranch.status}</span>
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

// ─── PlanCard ────────────────────────────────────────────────────────────────

function PlanCard({
  plan,
  onStatusChange,
  defaultExpanded = false,
}: {
  plan: Plan;
  onStatusChange: (id: string, status: PlanStatus) => void;
  defaultExpanded?: boolean;
}) {
  const { sendFollowup, setHandoffSource, branches, openThread } = useChatStore();
  const [expanded, setExpanded] = useState(defaultExpanded);
  const [subtasks, setSubtasks] = useState<PlanSubtask[] | null>(null);
  const [loading, setLoading] = useState(false);
  const statusCfg = PLAN_STATUS_CFG[plan.status];

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
        tasks = await planApi.listSubtasks(plan.id);
        setSubtasks(tasks);
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
                  onStatusChange={handleSubtaskStatus}
                  onOwnerChange={handleOwnerChange}
                  linkedBranch={linked ? { id: linked.id, label: linked.label, customLabel: linked.customLabel, status: linked.status } : null}
                  onOpenThread={openThread}
                />
              );
            })}
          </div>
          {/* Plan forward — send plan context to an agent */}
          <div className="flex items-center gap-1.5 pl-5 pt-1.5 mt-1 border-t border-border/20">
            <span className="text-[9px] text-muted-foreground/50">Forward:</span>
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
        </div>
      )}
    </div>
  );
}

// ─── PlansPanel (main export) ────────────────────────────────────────────────

export function PlansPanel() {
  const { selectedConversationId, activeBranchId, parentConversationId } = useChatStore();
  const [plans, setPlans] = useState<Plan[]>([]);
  const [showForm, setShowForm] = useState(false);
  const [expandedNewId, setExpandedNewId] = useState<string | null>(null);

  // In branch stream: selectedConversationId = "branch:xxx", canonical = parentConversationId
  const canonicalConvId = activeBranchId && parentConversationId
    ? parentConversationId
    : selectedConversationId;

  useEffect(() => {
    if (!canonicalConvId) return;
    planApi.listPlansByConversation(canonicalConvId)
      .then(setPlans)
      .catch(() => setPlans([]));
    setShowForm(false);
  }, [canonicalConvId]);

  const handlePlanStatus = async (planId: string, status: PlanStatus) => {
    try {
      await planApi.updatePlanStatus(planId, status);
      setPlans((prev) => prev.map((p) => (p.id === planId ? { ...p, status } : p)));
    } catch {
      // silent
    }
  };

  const handleCreated = (newPlan: Plan) => {
    setPlans((prev) => [newPlan, ...prev]);
    setShowForm(false);
    setExpandedNewId(newPlan.id);
  };

  if (!canonicalConvId) {
    return <p className="text-xs text-muted-foreground px-2">No conversation selected.</p>;
  }

  return (
    <div className="space-y-2">
      {plans.length === 0 && !showForm && (
        <div className="text-center py-4">
          <ClipboardList className="w-5 h-5 text-muted-foreground/40 mx-auto mb-2" />
          <p className="text-xs text-muted-foreground">No plans yet.</p>
        </div>
      )}

      {plans.map((plan) => (
        <PlanCard
          key={plan.id}
          plan={plan}
          onStatusChange={handlePlanStatus}
          defaultExpanded={plan.id === expandedNewId}
        />
      ))}

      {showForm && (
        <CreatePlanForm
          conversationId={canonicalConvId}
          activeBranchId={activeBranchId}
          onCreated={handleCreated}
          onCancel={() => setShowForm(false)}
        />
      )}

      {!showForm && (
        <button
          onClick={() => setShowForm(true)}
          className="w-full flex items-center gap-2 px-2 py-1.5 rounded-md text-xs text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
        >
          <Plus className="w-3.5 h-3.5" />
          New plan
        </button>
      )}
    </div>
  );
}
