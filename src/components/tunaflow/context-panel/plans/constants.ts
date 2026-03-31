import type { PlanPhase, PlanStatus, SubtaskStatus } from "@/types";

export const PLAN_STATUS_CFG: Record<PlanStatus, { label: string; cls: string }> = {
  draft:     { label: "draft",     cls: "text-muted-foreground bg-accent border-border" },
  active:    { label: "active",    cls: "text-primary bg-primary/10 border-primary/20" },
  done:      { label: "done",      cls: "text-status-approved bg-status-approved/10 border-status-approved/20" },
  abandoned: { label: "abandoned", cls: "text-status-rejected bg-status-rejected/10 border-status-rejected/20" },
};

export const SUBTASK_STATUS_CFG: Record<SubtaskStatus, { label: string; next: SubtaskStatus; cls: string }> = {
  todo:        { label: "todo",        next: "approved",     cls: "text-muted-foreground bg-accent border-border" },
  approved:    { label: "approved",    next: "in_progress",  cls: "text-agent-gemini bg-agent-gemini/10 border-agent-gemini/20" },
  in_progress: { label: "in progress", next: "done",         cls: "text-primary bg-primary/10 border-primary/20" },
  done:        { label: "done",        next: "todo",         cls: "text-status-approved bg-status-approved/10 border-status-approved/20" },
  abandoned:   { label: "abandoned",   next: "todo",         cls: "text-status-rejected bg-status-rejected/10 border-status-rejected/20" },
};

export const PLAN_PHASE_CFG: Record<PlanPhase, { label: string; cls: string }> = {
  drafting:        { label: "drafting",    cls: "text-muted-foreground bg-accent border-border" },
  subtask_review:  { label: "subtask",    cls: "text-amber-600 bg-amber-500/10 border-amber-500/20" },
  approval:        { label: "approved",   cls: "text-agent-gemini bg-agent-gemini/10 border-agent-gemini/20" },
  implementation:  { label: "dev",        cls: "text-primary bg-primary/10 border-primary/20" },
  review:          { label: "review",     cls: "text-agent-codex bg-agent-codex/10 border-agent-codex/20" },
  done:            { label: "done",       cls: "text-status-approved bg-status-approved/10 border-status-approved/20" },
  rework:          { label: "rework",     cls: "text-status-rejected bg-status-rejected/10 border-status-rejected/20" },
};

export const OWNER_OPTIONS = ["claude", "codex", "gemini", "opencode"];

export const INPUT_CLS =
  "w-full bg-input rounded-md px-2.5 py-1.5 text-xs outline-none text-foreground " +
  "placeholder:text-muted-foreground border border-border focus:border-ring/50";
