import { useState } from "react";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import {
  FileText, Clock, CheckCircle2, XCircle, Plus,
  ClipboardCheck, FileSearch, Gavel, TestTube,
} from "lucide-react";
import type { Artifact } from "@/types";

// ─── Constants ───────────────────────────────────────────────────────────────

const FORWARD_ENGINES = [
  { id: "claude", label: "Claude" },
  { id: "codex", label: "Codex" },
  { id: "gemini", label: "Gemini" },
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

// ─── ArtifactCard ────────────────────────────────────────────────────────────

function ArtifactCard({ artifact }: { artifact: Artifact }) {
  const { updateArtifactStatus, deleteArtifact, sendFollowup, setHandoffSource } = useChatStore();
  const status = STATUS_CONFIG[artifact.status];
  const [expanded, setExpanded] = useState(false);
  const isHarness = HARNESS_TYPES.has(artifact.type);
  const harnessConfig = isHarness ? HARNESS_TYPE_CONFIG[artifact.type] : null;

  return (
    <div
      className={cn(
        "rounded-md border p-2.5 hover:border-border/60 transition-colors cursor-pointer group",
        isHarness ? "border-border/40 bg-card/80" : "border-border/30 bg-card/50"
      )}
      onClick={() => {
        const next = !expanded;
        setExpanded(next);
        setHandoffSource(next ? { type: "artifact", content: `[${artifact.title}] ${artifact.content}` } : null);
      }}
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
      {!expanded && (
        <p className="text-[10px] text-muted-foreground/60 leading-snug line-clamp-2 ml-6">
          {artifact.content.slice(0, 100)}
        </p>
      )}
      {expanded && (
        <div className="ml-6 mt-2 space-y-2">
          <p className="text-[11px] text-foreground leading-relaxed whitespace-pre-wrap">{artifact.content}</p>
          <div className="flex gap-2 pt-1 border-t border-border/20">
            {artifact.status !== "approved" && (
              <button
                onClick={(e) => { e.stopPropagation(); updateArtifactStatus(artifact.id, "approved"); }}
                className="text-[9px] text-status-approved/70 hover:underline"
              >
                Approve
              </button>
            )}
            {artifact.status !== "rejected" && (
              <button
                onClick={(e) => { e.stopPropagation(); updateArtifactStatus(artifact.id, "rejected"); }}
                className="text-[9px] text-status-rejected/70 hover:underline"
              >
                Reject
              </button>
            )}
            {artifact.status !== "draft" && (
              <button
                onClick={(e) => { e.stopPropagation(); updateArtifactStatus(artifact.id, "draft"); }}
                className="text-[9px] text-muted-foreground hover:underline"
              >
                Draft
              </button>
            )}
            <span className="ml-auto flex items-center gap-2">
              {FORWARD_ENGINES.map((eng) => (
                <button
                  key={eng.id}
                  onClick={(e) => { e.stopPropagation(); sendFollowup(eng.id, "artifact", `[${artifact.title}] ${artifact.content}`); }}
                  className="text-[9px] text-primary/60 hover:text-primary hover:underline"
                >
                  → {eng.label}
                </button>
              ))}
              <button
                onClick={(e) => { e.stopPropagation(); deleteArtifact(artifact.id); }}
                className="text-[9px] text-destructive/70 hover:underline"
              >
                Delete
              </button>
            </span>
          </div>
        </div>
      )}
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

// ─── ArtifactsPanel (main export) ────────────────────────────────────────────

export function ArtifactsPanel() {
  const { artifacts, selectedConversationId, createArtifact } = useChatStore();
  const [showForm, setShowForm] = useState(false);
  const [title, setTitle] = useState("");
  const [content, setContent] = useState("");
  const [artType, setArtType] = useState("note");

  const handleCreate = async () => {
    if (!title.trim() || !content.trim() || !selectedConversationId) return;
    await createArtifact({ conversationId: selectedConversationId, type: artType, title: title.trim(), content: content.trim() });
    setTitle(""); setContent(""); setShowForm(false);
  };

  // Split artifacts into harness vs other
  const harnessArtifacts = artifacts.filter((a) => HARNESS_TYPES.has(a.type));
  const otherArtifacts = artifacts.filter((a) => !HARNESS_TYPES.has(a.type));

  return (
    <div className="space-y-3">
      {/* Harness section */}
      {harnessArtifacts.length > 0 && (
        <div>
          <div className="flex items-center gap-1.5 mb-1.5">
            <h4 className="text-[9px] font-semibold text-muted-foreground/50 uppercase tracking-widest">Harness</h4>
          </div>
          <HarnessStrip artifacts={harnessArtifacts} />
          <div className="space-y-1.5">
            {harnessArtifacts.map((a) => <ArtifactCard key={a.id} artifact={a} />)}
          </div>
        </div>
      )}

      {/* Other artifacts section */}
      {otherArtifacts.length > 0 && (
        <div>
          {harnessArtifacts.length > 0 && (
            <h4 className="text-[9px] font-semibold text-muted-foreground/50 uppercase tracking-widest mb-1.5 mt-1">
              Other
            </h4>
          )}
          <div className="space-y-1.5">
            {otherArtifacts.map((a) => <ArtifactCard key={a.id} artifact={a} />)}
          </div>
        </div>
      )}

      {/* Empty state */}
      {artifacts.length === 0 && !showForm && (
        <div className="text-center py-4">
          <FileText className="w-5 h-5 text-muted-foreground/30 mx-auto mb-2" />
          <p className="text-[11px] text-muted-foreground/50">No artifacts yet</p>
        </div>
      )}

      {/* Create form */}
      {showForm && (
        <div className="rounded-md border border-border/40 bg-card p-2.5 space-y-2">
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
            >
              Create
            </button>
            <button
              onClick={() => setShowForm(false)}
              className="px-2 py-1 rounded text-muted-foreground text-[11px] hover:bg-accent transition-colors"
            >
              Cancel
            </button>
          </div>
        </div>
      )}
      {!showForm && (
        <button
          onClick={() => setShowForm(true)}
          className="w-full flex items-center gap-2 px-2 py-1 rounded text-[11px] text-muted-foreground/60 hover:text-foreground hover:bg-accent/50 transition-colors"
        >
          <Plus className="w-3 h-3" />
          New artifact
        </button>
      )}
    </div>
  );
}
