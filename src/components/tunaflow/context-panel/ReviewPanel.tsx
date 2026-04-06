import { useState } from "react";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { FileSearch, Gavel, CheckCircle2, XCircle, X, ChevronDown, ChevronRight } from "lucide-react";
import type { Artifact } from "@/types";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

// ─── Verdict parser ─────────────────────────────────────────────────────────

interface ParsedVerdictCard {
  verdict: "PASS" | "FAIL" | "CONDITIONAL" | null;
  findings: string[];
  recommendations: string[];
  raw: string;
}

function parseVerdictContent(content: string): ParsedVerdictCard {
  const verdictMatch = content.match(/## Review Verdict:\s*(PASS|FAIL|CONDITIONAL)/i);
  const verdict = verdictMatch ? verdictMatch[1].toUpperCase() as ParsedVerdictCard["verdict"] : null;

  const findings: string[] = [];
  const recommendations: string[] = [];

  // Extract findings section
  const findingsMatch = content.match(/### Findings\n([\s\S]*?)(?=###|$)/);
  if (findingsMatch) {
    for (const line of findingsMatch[1].split("\n")) {
      const trimmed = line.replace(/^-\s*/, "").trim();
      if (trimmed && trimmed !== "\uC5C6\uC74C") findings.push(trimmed); // "없음" skip
    }
  }

  // Extract recommendations section
  const recMatch = content.match(/### Recommendations\n([\s\S]*?)(?=###|$)/);
  if (recMatch) {
    for (const line of recMatch[1].split("\n")) {
      const trimmed = line.replace(/^-\s*/, "").trim();
      if (trimmed) recommendations.push(trimmed);
    }
  }

  return { verdict, findings, recommendations, raw: content };
}

// ─── Decision parser ────────────────────────────────────────────────────────

interface ParsedDecisionCard {
  title: string;
  description: string;
  subtaskCount: number;
  raw: string;
}

function parseDecisionContent(content: string): ParsedDecisionCard {
  const titleMatch = content.match(/## Plan Approved:\s*(.+)/);
  const title = titleMatch ? titleMatch[1].trim() : "";

  // Count subtask lines
  const subtaskSection = content.match(/### Subtasks\n([\s\S]*?)(?=###|$)/);
  const subtaskCount = subtaskSection
    ? subtaskSection[1].split("\n").filter((l) => /^\d+\./.test(l.trim())).length
    : 0;

  // Description: everything between title and ### Subtasks (or end)
  const descMatch = content.match(/## Plan Approved:.*\n\n([\s\S]*?)(?=\n###|\n\*\*Expected|$)/);
  const description = descMatch ? descMatch[1].trim() : "";

  return { title, description, subtaskCount, raw: content };
}

// ─── Verdict colors ─────────────────────────────────────────────────────────

const VERDICT_CFG: Record<string, { cls: string; label: string; icon: React.ReactNode }> = {
  PASS: { cls: "text-status-approved border-status-approved/30 bg-status-approved/5", label: "PASS", icon: <CheckCircle2 className="w-3.5 h-3.5" /> },
  FAIL: { cls: "text-status-rejected border-status-rejected/30 bg-status-rejected/5", label: "FAIL", icon: <XCircle className="w-3.5 h-3.5" /> },
  CONDITIONAL: { cls: "text-agent-gemini border-agent-gemini/30 bg-agent-gemini/5", label: "CONDITIONAL", icon: <FileSearch className="w-3.5 h-3.5" /> },
};

// ─── Review Verdict Card ────────────────────────────────────────────────────

function VerdictCard({ artifact, onOpen }: { artifact: Artifact; onOpen: (a: Artifact) => void }) {
  const parsed = parseVerdictContent(artifact.content);
  const cfg = parsed.verdict ? VERDICT_CFG[parsed.verdict] : null;

  return (
    <div
      onClick={() => onOpen(artifact)}
      className={cn(
        "rounded-lg border p-3 cursor-pointer hover:ring-1 hover:ring-primary/20 transition-all",
        cfg?.cls ?? "border-border/30 bg-card/60",
      )}
    >
      {/* Header: verdict badge + title */}
      <div className="flex items-center gap-2 mb-2">
        {cfg && <span className="shrink-0">{cfg.icon}</span>}
        <span className="text-[12px] font-semibold">{cfg?.label ?? "REVIEW"}</span>
        <span className="text-[10px] text-muted-foreground/50 truncate flex-1">{artifact.title}</span>
        <span className="text-[8px] text-muted-foreground/30 font-mono shrink-0">
          {new Date(artifact.updatedAt * 1000).toLocaleDateString()}
        </span>
      </div>

      {/* Findings preview (max 3) */}
      {parsed.findings.length > 0 && (
        <div className="space-y-0.5 mb-2">
          {parsed.findings.slice(0, 3).map((f, i) => (
            <p key={i} className="text-[10px] text-foreground/70 leading-snug truncate pl-2 border-l-2 border-current/20">
              {f}
            </p>
          ))}
          {parsed.findings.length > 3 && (
            <p className="text-[9px] text-muted-foreground/40 pl-2">+{parsed.findings.length - 3} more</p>
          )}
        </div>
      )}
      {parsed.findings.length === 0 && parsed.verdict === "PASS" && (
        <p className="text-[10px] text-muted-foreground/40 mb-2">No findings</p>
      )}

      {/* Recommendations preview (1 line) */}
      {parsed.recommendations.length > 0 && (
        <p className="text-[9px] text-muted-foreground/50 truncate">
          Rec: {parsed.recommendations[0]}
          {parsed.recommendations.length > 1 && ` (+${parsed.recommendations.length - 1})`}
        </p>
      )}
    </div>
  );
}

// ─── Decision Card ──────────────────────────────────────────────────────────

function DecisionCard({ artifact, onOpen }: { artifact: Artifact; onOpen: (a: Artifact) => void }) {
  const parsed = parseDecisionContent(artifact.content);

  return (
    <div
      onClick={() => onOpen(artifact)}
      className="rounded-lg border border-status-approved/20 bg-status-approved/3 p-3 cursor-pointer hover:ring-1 hover:ring-primary/20 transition-all"
    >
      <div className="flex items-center gap-2 mb-1.5">
        <Gavel className="w-3.5 h-3.5 text-status-approved/60 shrink-0" />
        <span className="text-[12px] font-semibold text-status-approved/80">APPROVED</span>
        <span className="text-[8px] text-muted-foreground/30 font-mono ml-auto shrink-0">
          {new Date(artifact.updatedAt * 1000).toLocaleDateString()}
        </span>
      </div>
      <p className="text-[11px] font-medium text-foreground/80 truncate">{parsed.title || artifact.title}</p>
      {parsed.description && (
        <p className="text-[10px] text-muted-foreground/50 truncate mt-0.5">{parsed.description}</p>
      )}
      {parsed.subtaskCount > 0 && (
        <p className="text-[9px] text-muted-foreground/40 mt-1">{parsed.subtaskCount} subtasks</p>
      )}
    </div>
  );
}

// ─── Detail Modal ───────────────────────────────────────────────────────────

const PROSE_CLS = "prose prose-sm prose-invert max-w-none [&>*:first-child]:mt-0 [&>*:last-child]:mb-0 [&>hr]:border-sidebar-foreground/20";

function ReviewDetailModal({ artifact, onClose }: { artifact: Artifact; onClose: () => void }) {
  return (
    <div className="fixed inset-0 z-[70] flex items-center justify-center">
      <div className="absolute inset-0 bg-black/30" onClick={onClose} />
      <div className="relative bg-card border border-border/40 rounded-xl shadow-2xl w-[640px] max-h-[80vh] overflow-hidden flex flex-col">
        {/* Header */}
        <div className="flex items-center gap-2 px-5 pt-4 pb-3 shrink-0 border-b border-border/20">
          <span className="text-[13px] font-semibold text-foreground flex-1 truncate">{artifact.title}</span>
          <span className="text-[9px] text-muted-foreground/40 font-mono">
            {new Date(artifact.updatedAt * 1000).toLocaleString()}
          </span>
          <button onClick={onClose} className="p-1 rounded hover:bg-accent text-muted-foreground hover:text-foreground transition-colors shrink-0">
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Content — full markdown rendering */}
        <div className="flex-1 overflow-y-auto px-5 py-4">
          <div className={PROSE_CLS}>
            <ReactMarkdown remarkPlugins={[[remarkGfm, { singleTilde: false }]]}>
              {artifact.content}
            </ReactMarkdown>
          </div>
        </div>
      </div>
    </div>
  );
}

// ─── ReviewPanel ────────────────────────────────────────────────────────────

export function ReviewPanel() {
  const artifacts = useChatStore((s) => s.artifacts);
  const [detailArtifact, setDetailArtifact] = useState<Artifact | null>(null);
  const [findingsCollapsed, setFindingsCollapsed] = useState(false);
  const [decisionsCollapsed, setDecisionsCollapsed] = useState(false);

  const reviewFindings = artifacts
    .filter((a) => a.type === "review-findings")
    .sort((a, b) => b.updatedAt - a.updatedAt);
  const decisions = artifacts
    .filter((a) => a.type === "architect-decision")
    .sort((a, b) => b.updatedAt - a.updatedAt);

  const hasContent = reviewFindings.length > 0 || decisions.length > 0;

  return (
    <div className="space-y-4">
      {/* Summary strip */}
      {hasContent && (
        <div className="flex items-center gap-3 text-[9px] text-sidebar-foreground/50">
          {reviewFindings.length > 0 && (
            <span className="flex items-center gap-1">
              <FileSearch className="w-3 h-3 text-status-draft/60" />
              {reviewFindings.length} finding{reviewFindings.length > 1 ? "s" : ""}
            </span>
          )}
          {decisions.length > 0 && (
            <span className="flex items-center gap-1">
              <Gavel className="w-3 h-3 text-status-approved/60" />
              {decisions.length} decision{decisions.length > 1 ? "s" : ""}
            </span>
          )}
        </div>
      )}

      {/* Review findings */}
      {reviewFindings.length > 0 && (
        <div>
          <button
            onClick={() => setFindingsCollapsed(!findingsCollapsed)}
            className="flex items-center gap-1 text-[9px] font-semibold text-muted-foreground/50 uppercase tracking-widest mb-2 hover:text-foreground/60 transition-colors"
          >
            {findingsCollapsed ? <ChevronRight className="w-2.5 h-2.5" /> : <ChevronDown className="w-2.5 h-2.5" />}
            Findings
          </button>
          {!findingsCollapsed && (
            <div className="space-y-2">
              {reviewFindings.map((a) => (
                <VerdictCard key={a.id} artifact={a} onOpen={setDetailArtifact} />
              ))}
            </div>
          )}
        </div>
      )}

      {/* Decisions */}
      {decisions.length > 0 && (
        <div>
          <button
            onClick={() => setDecisionsCollapsed(!decisionsCollapsed)}
            className="flex items-center gap-1 text-[9px] font-semibold text-muted-foreground/50 uppercase tracking-widest mb-2 hover:text-foreground/60 transition-colors"
          >
            {decisionsCollapsed ? <ChevronRight className="w-2.5 h-2.5" /> : <ChevronDown className="w-2.5 h-2.5" />}
            Decisions
          </button>
          {!decisionsCollapsed && (
            <div className="space-y-2">
              {decisions.map((a) => (
                <DecisionCard key={a.id} artifact={a} onOpen={setDetailArtifact} />
              ))}
            </div>
          )}
        </div>
      )}

      {!hasContent && (
        <div className="text-center py-6">
          <FileSearch className="w-5 h-5 text-muted-foreground/20 mx-auto mb-2" />
          <p className="text-[11px] text-muted-foreground/40">No review findings or decisions yet</p>
        </div>
      )}

      {/* Detail modal */}
      {detailArtifact && (
        <ReviewDetailModal artifact={detailArtifact} onClose={() => setDetailArtifact(null)} />
      )}
    </div>
  );
}
