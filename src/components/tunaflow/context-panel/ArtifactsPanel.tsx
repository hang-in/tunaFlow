import { useState, useEffect, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { copyToClipboard } from "@/lib/clipboard";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import {
  FileText, Clock, CheckCircle2, XCircle, Plus, X,
  ClipboardCheck, FileSearch, Gavel, TestTube, ChevronDown, ChevronRight,
} from "lucide-react";
import type { Artifact, Plan } from "@/types";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

// ─── Constants ───────────────────────────────────────────────────────────────

const FORWARD_ENGINES = [
  { id: "claude", label: "Claude" },
  { id: "codex", label: "Codex" },
  { id: "gemini", label: "Gemini" },
  { id: "ollama", label: "Ollama" },
];

const HARNESS_TYPES = new Set(["task-brief", "test-report", "review-findings", "architect-decision"]);

const HARNESS_TYPE_CONFIG: Record<string, { icon: React.ReactNode; label: string; cls: string }> = {
  "task-brief":          { icon: <ClipboardCheck className="w-2.5 h-2.5" />, label: "Brief",    cls: "text-primary/60 bg-primary/6" },
  "review-findings":     { icon: <FileSearch className="w-2.5 h-2.5" />,     label: "Review",   cls: "text-status-draft/70 bg-status-draft/8" },
  "architect-decision":  { icon: <Gavel className="w-2.5 h-2.5" />,          label: "Decision", cls: "text-status-approved/70 bg-status-approved/8" },
  "test-report":         { icon: <TestTube className="w-2.5 h-2.5" />,       label: "Test",     cls: "text-agent-codex/60 bg-agent-codex/6" },
};

type ArtifactStatus = "draft" | "approved" | "rejected";

const STATUS_CONFIG: Record<ArtifactStatus, { icon: React.ReactNode; class: string; label: string }> = {
  draft:    { icon: <Clock className="w-2.5 h-2.5" />,        class: "text-muted-foreground/60 bg-muted",               label: "draft" },
  approved: { icon: <CheckCircle2 className="w-2.5 h-2.5" />, class: "text-status-approved/70 bg-status-approved/8",    label: "approved" },
  rejected: { icon: <XCircle className="w-2.5 h-2.5" />,      class: "text-status-rejected/70 bg-status-rejected/8",    label: "rejected" },
};

// Navigate to artifact source conversation/branch
function jumpToSource(artifact: Artifact, store: any) {
  if (artifact.branchId) {
    store.openThread(artifact.branchId);
  } else if (artifact.conversationId) {
    store.selectConversation(artifact.conversationId);
  }
}

// ─── ArtifactCard ────────────────────────────────────────────────────────────

function ArtifactCard({
  artifact, active, onOpen,
}: {
  artifact: Artifact; active: boolean; onOpen: (a: Artifact) => void;
}) {
  const status = STATUS_CONFIG[artifact.status];
  const isHarness = HARNESS_TYPES.has(artifact.type);
  const harnessConfig = isHarness ? HARNESS_TYPE_CONFIG[artifact.type] : null;
  const conversations = useChatStore((s) => s.conversations);
  const branches = useChatStore((s) => s.branches);
  const sourceConv = artifact.conversationId ? conversations.find((c) => c.id === artifact.conversationId) : null;
  const sourceBranch = artifact.branchId ? branches.find((b) => b.id === artifact.branchId) : null;
  const provenanceHint = sourceBranch
    ? (sourceBranch.customLabel ?? sourceBranch.label) + (sourceBranch.mode === "roundtable" ? " · RT" : "")
    : sourceConv ? (sourceConv.customLabel ?? sourceConv.label) : null;

  return (
    <div
      className={cn(
        "rounded-md border p-2.5 hover:border-border/60 transition-colors cursor-pointer group",
        active
          ? "border-primary/40 bg-primary/5 ring-1 ring-primary/20"
          : isHarness ? "border-border/40 bg-card/80" : "border-border/30 bg-card/50"
      )}
      onClick={() => onOpen(artifact)}
    >
      <div className="flex items-start gap-2 mb-1">
        {harnessConfig ? (
          <span className={cn("shrink-0 mt-0.5 p-0.5 rounded", harnessConfig.cls)}>
            {harnessConfig.icon}
          </span>
        ) : (
          <FileText className="w-3.5 h-3.5 text-muted-foreground/40 shrink-0 mt-0.5" />
        )}
        <div className="flex-1 min-w-0">
          <span className="text-[11px] font-medium text-foreground leading-snug">{artifact.title}</span>
          {harnessConfig && (
            <span className={cn("ml-1.5 text-[7px] font-medium px-1 py-0 rounded inline-block", harnessConfig.cls)}>
              {harnessConfig.label}
            </span>
          )}
        </div>
        <span className={cn("inline-flex items-center gap-0.5 text-[9px] px-1.5 py-0.5 rounded shrink-0", status.class)}>
          {status.icon}
          {status.label}
        </span>
      </div>
      <p className="text-[10px] text-muted-foreground/60 leading-snug line-clamp-2 ml-6">
        {artifact.content.slice(0, 100)}
      </p>
      <div className="flex items-center gap-2 ml-6 mt-0.5 text-[9px] text-muted-foreground/30 font-mono">
        <span>{new Date(artifact.updatedAt * 1000).toLocaleDateString()}</span>
        {provenanceHint && (
          <button
            onClick={(e) => { e.stopPropagation(); jumpToSource(artifact, useChatStore.getState()); }}
            className="hover:text-primary/60 hover:underline transition-colors"
          >
            · {provenanceHint}
          </button>
        )}
        {artifact.subtaskId && <span>· subtask</span>}
      </div>
    </div>
  );
}

// ─── Harness Summary Strip ───────────────────────────────────────────────────

function HarnessStrip({ artifacts }: { artifacts: Artifact[] }) {
  const briefs = artifacts.filter((a) => a.type === "task-brief");
  const reviews = artifacts.filter((a) => a.type === "review-findings");
  const decisions = artifacts.filter((a) => a.type === "architect-decision");
  const tests = artifacts.filter((a) => a.type === "test-report");

  if (briefs.length + reviews.length + decisions.length + tests.length === 0) return null;

  return (
    <div className="flex items-center gap-2 flex-wrap text-[8px] text-muted-foreground/50 mb-2">
      {briefs.length > 0 && (
        <span className="flex items-center gap-0.5">
          <ClipboardCheck className="w-2.5 h-2.5 text-primary/40" />
          {briefs.length} brief
        </span>
      )}
      {reviews.length > 0 && (
        <span className="flex items-center gap-0.5">
          <FileSearch className="w-2.5 h-2.5 text-status-draft/50" />
          {reviews.length} review
        </span>
      )}
      {decisions.length > 0 && (
        <span className="flex items-center gap-0.5">
          <Gavel className="w-2.5 h-2.5 text-status-approved/50" />
          {decisions.length} decision
        </span>
      )}
      {tests.length > 0 && (
        <span className="flex items-center gap-0.5">
          <TestTube className="w-2.5 h-2.5 text-agent-codex/40" />
          {tests.length} test
        </span>
      )}
    </div>
  );
}

// ─── Plan Group ──────────────────────────────────────────────────────────────

function PlanGroup({
  planId, planTitle, artifacts, activeId, onOpen,
}: {
  planId: string | null; planTitle?: string;
  artifacts: Artifact[]; activeId: string | null;
  onOpen: (a: Artifact) => void;
}) {
  const [collapsed, setCollapsed] = useState(false);
  const label = planId ? (planTitle ?? "Plan...") : "Ungrouped";

  return (
    <div className="rounded-md border border-border/20 bg-card/30">
      <button
        onClick={() => setCollapsed(!collapsed)}
        className="w-full flex items-center gap-1.5 px-2.5 py-1.5 text-left hover:bg-accent/30 transition-colors rounded-t-md"
      >
        {collapsed
          ? <ChevronRight className="w-3 h-3 text-muted-foreground/40 shrink-0" />
          : <ChevronDown className="w-3 h-3 text-muted-foreground/40 shrink-0" />}
        <span className="text-[11px] font-medium text-foreground/80 truncate flex-1">{label}</span>
        <span className="text-[9px] text-muted-foreground/40 font-mono shrink-0">{artifacts.length}</span>
      </button>
      {!collapsed && (
        <div className="px-1.5 pb-1.5 space-y-1">
          {artifacts.map((a) => (
            <ArtifactCard key={a.id} artifact={a} active={a.id === activeId} onOpen={onOpen} />
          ))}
        </div>
      )}
    </div>
  );
}

// ─── Detail Panel ────────────────────────────────────────────────────────────

function ArtifactDetailPanel({
  artifact, onClose,
}: {
  artifact: Artifact; onClose: () => void;
}) {
  const { updateArtifactStatus, deleteArtifact, sendFollowup, conversations, branches } = useChatStore();
  const status = STATUS_CONFIG[artifact.status];
  const harnessConfig = HARNESS_TYPES.has(artifact.type) ? HARNESS_TYPE_CONFIG[artifact.type] : null;

  const sourceConv = artifact.conversationId ? conversations.find((c) => c.id === artifact.conversationId) : null;
  const sourceBranch = artifact.branchId ? branches.find((b) => b.id === artifact.branchId) : null;
  const sourceLabel = sourceBranch
    ? `Branch: ${sourceBranch.customLabel ?? sourceBranch.label}${sourceBranch.mode === "roundtable" ? " (RT)" : ""}`
    : sourceConv ? `${sourceConv.customLabel ?? sourceConv.label}` : null;

  return (
    <div className="flex-1 min-w-0 flex flex-col border-l border-border/20 overflow-hidden">
      {/* Header */}
      <div className="flex items-start gap-2 px-4 pt-3 pb-2.5 shrink-0 border-b border-border/20">
        {harnessConfig ? (
          <span className={cn("shrink-0 mt-1 p-0.5 rounded", harnessConfig.cls)}>{harnessConfig.icon}</span>
        ) : (
          <FileText className="w-3.5 h-3.5 text-muted-foreground/40 shrink-0 mt-1" />
        )}
        <div className="flex-1 min-w-0">
          <h3 className="text-[13px] font-[550] text-foreground leading-snug">{artifact.title}</h3>
          <div className="flex items-center gap-1.5 mt-0.5 flex-wrap">
            <span className={cn("inline-flex items-center gap-0.5 text-[9px] px-1.5 py-0.5 rounded", status.class)}>
              {status.icon} {status.label}
            </span>
            {harnessConfig && (
              <span className={cn("text-[7px] font-medium px-1.5 py-0.5 rounded", harnessConfig.cls)}>
                {harnessConfig.label}
              </span>
            )}
            <span className="text-[9px] text-muted-foreground/30 font-mono">
              {new Date(artifact.updatedAt * 1000).toLocaleString()}
            </span>
          </div>
          {(sourceLabel || artifact.subtaskId) && (
            <div className="flex items-center gap-2 mt-1 text-[9px] text-muted-foreground/40">
              {sourceLabel && (
                <button
                  onClick={() => jumpToSource(artifact, useChatStore.getState())}
                  className="hover:text-primary/60 hover:underline transition-colors"
                >
                  {sourceLabel}
                </button>
              )}
              {artifact.subtaskId && <span>· Subtask linked</span>}
            </div>
          )}
        </div>
        <button
          onClick={onClose}
          className="p-1 rounded hover:bg-accent text-muted-foreground hover:text-foreground transition-colors shrink-0"
        >
          <X className="w-3.5 h-3.5" />
        </button>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto px-4 py-3">
        <div className="prose prose-sm prose-invert max-w-none text-[12px] leading-relaxed [&>*:first-child]:mt-0 [&>*:last-child]:mb-0 [&>h2]:text-[14px] [&>h2]:font-semibold [&>h2]:mt-4 [&>h2]:mb-2 [&>h3]:text-[12px] [&>h3]:font-semibold [&>h3]:mt-3 [&>h3]:mb-1 [&>ul]:space-y-0.5 [&>ul>li]:text-[11px] [&>p]:text-foreground/85">
          <ReactMarkdown remarkPlugins={[[remarkGfm, { singleTilde: false }]]}>
            {artifact.content}
          </ReactMarkdown>
        </div>
      </div>

      {/* Actions */}
      <div className="flex items-center gap-2 px-4 py-2 border-t border-border/20 shrink-0 text-[10px] flex-wrap">
        {artifact.status !== "approved" && (
          <button
            onClick={() => updateArtifactStatus(artifact.id, "approved")}
            className="text-status-approved/70 hover:underline"
          >Approve</button>
        )}
        {artifact.status !== "rejected" && (
          <button
            onClick={() => updateArtifactStatus(artifact.id, "rejected")}
            className="text-status-rejected/70 hover:underline"
          >Reject</button>
        )}
        {artifact.status !== "draft" && (
          <button
            onClick={() => updateArtifactStatus(artifact.id, "draft")}
            className="text-muted-foreground hover:underline"
          >Draft</button>
        )}
        <span className="flex-1" />
        <button
          onClick={() => copyToClipboard(artifact.content)}
          className="text-muted-foreground/50 hover:text-foreground hover:underline"
        >Copy</button>
        {FORWARD_ENGINES.map((eng) => (
          <button
            key={eng.id}
            onClick={() => sendFollowup(eng.id, "artifact", `[${artifact.title}] ${artifact.content}`)}
            className="text-primary/60 hover:text-primary hover:underline"
          >→ {eng.label}</button>
        ))}
        <button
          onClick={() => { deleteArtifact(artifact.id); onClose(); }}
          className="text-destructive/70 hover:underline"
        >Delete</button>
      </div>
    </div>
  );
}

// ─── Filter / Sort options ────────────────────────────────────────────────────

const FILTER_TABS = [
  { id: "all", label: "All" },
  { id: "note", label: "Notes" },
  { id: "code", label: "Code" },
  { id: "spec", label: "Specs" },
  { id: "harness", label: "Harness" },
];

const SORT_OPTIONS = [
  { id: "newest", label: "Newest" },
  { id: "oldest", label: "Oldest" },
  { id: "title", label: "Title" },
];

// ─── ArtifactsPanel ──────────────────────────────────────────────────────────

export function ArtifactsPanel() {
  const { artifacts, selectedConversationId, createArtifact } = useChatStore();
  const [showForm, setShowForm] = useState(false);
  const [title, setTitle] = useState("");
  const [content, setContent] = useState("");
  const [artType, setArtType] = useState("note");
  const [filter, setFilter] = useState("all");
  const [sort, setSort] = useState("newest");
  const [groupByPlan, setGroupByPlan] = useState(true);
  const [detailArtifact, setDetailArtifact] = useState<Artifact | null>(null);

  // Load plan titles for grouping
  const [planTitles, setPlanTitles] = useState<Record<string, string>>({});
  const planIds = useMemo(
    () => [...new Set(artifacts.map((a) => a.planId).filter(Boolean))] as string[],
    [artifacts],
  );
  useEffect(() => {
    if (planIds.length === 0) return;
    const newIds = planIds.filter((id) => !planTitles[id]);
    if (newIds.length === 0) return;
    Promise.all(
      newIds.map((id) =>
        invoke<Plan>("get_plan", { id }).then((p) => [id, p.title] as const).catch(() => null),
      ),
    ).then((results) => {
      const map = { ...planTitles };
      for (const r of results) { if (r) map[r[0]] = r[1]; }
      setPlanTitles(map);
    });
  }, [planIds]); // eslint-disable-line react-hooks/exhaustive-deps

  const handleCreate = async () => {
    if (!title.trim() || !content.trim() || !selectedConversationId) return;
    await createArtifact({ conversationId: selectedConversationId, type: artType, title: title.trim(), content: content.trim() });
    setTitle(""); setContent(""); setShowForm(false);
  };

  // Filter
  const filtered = artifacts.filter((a) => {
    if (filter === "all") return true;
    if (filter === "harness") return HARNESS_TYPES.has(a.type);
    return a.type === filter;
  });

  // Sort
  const sorted = [...filtered].sort((a, b) => {
    if (sort === "newest") return b.updatedAt - a.updatedAt;
    if (sort === "oldest") return a.updatedAt - b.updatedAt;
    return a.title.localeCompare(b.title);
  });

  // Group by plan
  const groups = useMemo(() => {
    if (!groupByPlan || planIds.length === 0) return null;
    const map = new Map<string | null, Artifact[]>();
    for (const a of sorted) {
      const key = a.planId ?? null;
      const list = map.get(key) ?? [];
      list.push(a);
      map.set(key, list);
    }
    const entries = [...map.entries()].sort((a, b) => {
      if (a[0] === null) return 1;
      if (b[0] === null) return -1;
      const aTime = Math.max(...a[1].map((x) => x.updatedAt));
      const bTime = Math.max(...b[1].map((x) => x.updatedAt));
      return bTime - aTime;
    });
    return entries;
  }, [sorted, groupByPlan, planIds.length]);

  return (
    <div className="flex flex-col h-full overflow-hidden">
      {/* Toolbar */}
      <div className="flex items-center gap-2 px-3 py-2 border-b border-border/20 shrink-0 flex-wrap">
        <div className="flex items-center gap-0.5">
          {FILTER_TABS.map((tab) => (
            <button key={tab.id} onClick={() => setFilter(tab.id)}
              className={cn("px-2 py-0.5 rounded text-[11px] font-medium transition-colors",
                filter === tab.id ? "bg-accent text-foreground" : "text-muted-foreground/50 hover:text-foreground hover:bg-accent/50"
              )}>
              {tab.label}
            </button>
          ))}
        </div>
        <span className="flex-1" />
        {planIds.length > 0 && (
          <button
            onClick={() => setGroupByPlan(!groupByPlan)}
            className={cn("px-1.5 py-0.5 rounded text-[10px] transition-colors",
              groupByPlan ? "bg-accent text-foreground" : "text-muted-foreground/40 hover:text-foreground"
            )}
          >Plan</button>
        )}
        <select value={sort} onChange={(e) => setSort(e.target.value)}
          className="bg-transparent text-[10px] text-muted-foreground/50 outline-none cursor-pointer">
          {SORT_OPTIONS.map((o) => <option key={o.id} value={o.id}>{o.label}</option>)}
        </select>
        <button
          onClick={() => setShowForm(!showForm)}
          className="flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] text-muted-foreground/60 hover:text-foreground hover:bg-accent/50 transition-colors"
        >
          <Plus className="w-3 h-3" />
          New
        </button>
      </div>

      {/* Create form (inline below toolbar) */}
      {showForm && (
        <div className="px-3 py-2 border-b border-border/20 shrink-0 space-y-1.5">
          <select
            value={artType}
            onChange={(e) => setArtType(e.target.value)}
            className="w-full bg-input rounded px-2 py-1 text-[11px] outline-none text-foreground border border-border/40 focus:border-ring/50"
          >
            <option value="note">Note</option>
            <option value="code">Code</option>
            <option value="spec">Spec</option>
            <option value="plan">Plan</option>
            <optgroup label="Harness">
              <option value="task-brief">Task Brief</option>
              <option value="test-report">Test Report</option>
              <option value="review-findings">Review Findings</option>
              <option value="architect-decision">Architect Decision</option>
            </optgroup>
          </select>
          <input
            placeholder="Title"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            className="w-full bg-input rounded px-2 py-1 text-[11px] outline-none text-foreground placeholder:text-muted-foreground/50 border border-border/40 focus:border-ring/50"
          />
          <textarea
            placeholder="Content"
            value={content}
            onChange={(e) => setContent(e.target.value)}
            rows={3}
            className="w-full bg-input rounded px-2 py-1 text-[11px] outline-none text-foreground placeholder:text-muted-foreground/50 border border-border/40 focus:border-ring/50 resize-none"
          />
          <div className="flex gap-2">
            <button
              onClick={handleCreate}
              className="flex-1 px-2 py-1 rounded bg-primary/12 text-primary text-[11px] hover:bg-primary/20 transition-colors"
            >Create</button>
            <button
              onClick={() => setShowForm(false)}
              className="px-2 py-1 rounded text-muted-foreground text-[11px] hover:bg-accent transition-colors"
            >Cancel</button>
          </div>
        </div>
      )}

      {/* Master-detail content */}
      <div className="flex-1 flex min-h-0">
        {/* Left: list */}
        <div className={cn(
          "overflow-y-auto p-3 space-y-2",
          detailArtifact ? "w-[42%] shrink-0" : "flex-1",
        )}>
          {artifacts.length > 0 && <HarnessStrip artifacts={artifacts} />}

          {sorted.length > 0 ? (
            groups ? (
              <div className="space-y-2">
                {groups.map(([planId, items]) => (
                  <PlanGroup
                    key={planId ?? "__ungrouped"}
                    planId={planId}
                    planTitle={planId ? planTitles[planId] : undefined}
                    artifacts={items}
                    activeId={detailArtifact?.id ?? null}
                    onOpen={setDetailArtifact}
                  />
                ))}
              </div>
            ) : (
              <div className="space-y-1.5">
                {sorted.map((a) => (
                  <ArtifactCard key={a.id} artifact={a} active={a.id === detailArtifact?.id} onOpen={setDetailArtifact} />
                ))}
              </div>
            )
          ) : artifacts.length > 0 ? (
            <div className="text-center py-4 text-[12px] text-muted-foreground/40">
              No artifacts match this filter
            </div>
          ) : (
            <div className="text-center py-6">
              <FileText className="w-5 h-5 text-muted-foreground/30 mx-auto mb-2" />
              <p className="text-[11px] text-muted-foreground/50">No artifacts yet</p>
            </div>
          )}
        </div>

        {/* Right: detail */}
        {detailArtifact && (
          <ArtifactDetailPanel
            artifact={detailArtifact}
            onClose={() => setDetailArtifact(null)}
          />
        )}
      </div>
    </div>
  );
}
