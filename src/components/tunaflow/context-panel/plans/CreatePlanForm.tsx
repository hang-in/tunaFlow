import { useState } from "react";
import { cn } from "@/lib/utils";
import { Plus, X, GitBranch } from "lucide-react";
import type { Plan, SubtaskInput } from "@/types";
import * as planApi from "@/lib/api/plans";
import { INPUT_CLS } from "./constants";

type PlanScope = "conversation" | "branch";

export function CreatePlanForm({
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
