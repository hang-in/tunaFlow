import { useState } from "react";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { FileSearch, Gavel, CheckCircle2, XCircle, X } from "lucide-react";
import type { Artifact } from "@/types";
import ReactMarkdown from "react-markdown";
import { REMARK_PLUGINS } from "@/lib/markdownPlugins";

// ─── Verdict parser ──────────────────────────────────────────────────────────

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

  const findingsMatch = content.match(/### Findings\n([\s\S]*?)(?=###|$)/);
  if (findingsMatch) {
    for (const line of findingsMatch[1].split("\n")) {
      const trimmed = line.replace(/^-\s*/, "").trim();
      if (trimmed && trimmed !== "\uC5C6\uC74C") findings.push(trimmed);
    }
  }

  const recMatch = content.match(/### Recommendations\n([\s\S]*?)(?=###|$)/);
  if (recMatch) {
    for (const line of recMatch[1].split("\n")) {
      const trimmed = line.replace(/^-\s*/, "").trim();
      if (trimmed) recommendations.push(trimmed);
    }
  }

  return { verdict, findings, recommendations, raw: content };
}

// ─── Decision parser ─────────────────────────────────────────────────────────

interface ParsedDecisionCard {
  title: string;
  description: string;
  subtaskCount: number;
  raw: string;
}

function parseDecisionContent(content: string): ParsedDecisionCard {
  const titleMatch = content.match(/## Plan Approved:\s*(.+)/);
  const title = titleMatch ? titleMatch[1].trim() : "";

  const subtaskSection = content.match(/### Subtasks\n([\s\S]*?)(?=###|$)/);
  const subtaskCount = subtaskSection
    ? subtaskSection[1].split("\n").filter((l) => /^\d+\./.test(l.trim())).length
    : 0;

  const descMatch = content.match(/## Plan Approved:.*\n\n([\s\S]*?)(?=\n###|\n\*\*Expected|$)/);
  const description = descMatch ? descMatch[1].trim() : "";

  return { title, description, subtaskCount, raw: content };
}

// ─── Verdict colors ──────────────────────────────────────────────────────────

const VERDICT_CFG: Record<string, { cls: string; label: string; icon: React.ReactNode }> = {
  PASS:        { cls: "text-status-approved border-status-approved/30 bg-status-approved/5", label: "PASS",        icon: <CheckCircle2 className="w-3.5 h-3.5" /> },
  FAIL:        { cls: "text-status-rejected border-status-rejected/30 bg-status-rejected/5", label: "FAIL",        icon: <XCircle className="w-3.5 h-3.5" /> },
  CONDITIONAL: { cls: "text-agent-gemini border-agent-gemini/30 bg-agent-gemini/5",          label: "CONDITIONAL", icon: <FileSearch className="w-3.5 h-3.5" /> },
};

// ─── VerdictCard ─────────────────────────────────────────────────────────────

function VerdictCard({
  artifact, active, onOpen,
}: {
  artifact: Artifact; active: boolean; onOpen: (a: Artifact) => void;
}) {
  const parsed = parseVerdictContent(artifact.content);
  const cfg = parsed.verdict ? VERDICT_CFG[parsed.verdict] : null;

  return (
    <div
      onClick={() => onOpen(artifact)}
      className={cn(
        "rounded-lg border p-3 cursor-pointer hover:ring-1 hover:ring-primary/20 transition-all",
        active ? "ring-1 ring-primary/30" : "",
        cfg?.cls ?? "border-border/30 bg-card/60",
      )}
    >
      <div className="flex items-center gap-2 mb-2">
        {cfg && <span className="shrink-0">{cfg.icon}</span>}
        <span className="text-[12px] font-semibold">{cfg?.label ?? "REVIEW"}</span>
        <span className="text-[10px] text-muted-foreground/50 truncate flex-1">{artifact.title}</span>
        <span className="text-[8px] text-muted-foreground/30 font-mono shrink-0">
          {new Date(artifact.updatedAt * 1000).toLocaleDateString()}
        </span>
      </div>
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
      {parsed.recommendations.length > 0 && (
        <p className="text-[9px] text-muted-foreground/50 truncate">
          Rec: {parsed.recommendations[0]}
          {parsed.recommendations.length > 1 && ` (+${parsed.recommendations.length - 1})`}
        </p>
      )}
    </div>
  );
}

// ─── DecisionCard ─────────────────────────────────────────────────────────────

function DecisionCard({
  artifact, active, onOpen,
}: {
  artifact: Artifact; active: boolean; onOpen: (a: Artifact) => void;
}) {
  const parsed = parseDecisionContent(artifact.content);

  return (
    <div
      onClick={() => onOpen(artifact)}
      className={cn(
        "rounded-lg border border-status-approved/20 bg-status-approved/3 p-3 cursor-pointer hover:ring-1 hover:ring-primary/20 transition-all",
        active ? "ring-1 ring-primary/30" : "",
      )}
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

// ─── Detail Panel ─────────────────────────────────────────────────────────────

const PROSE_CLS = "prose prose-sm prose-invert max-w-none text-[13px] leading-relaxed [&>*:first-child]:mt-0 [&>*:last-child]:mb-0 [&>hr]:border-sidebar-foreground/20 [&>h2]:text-[15px] [&>h2]:font-semibold [&>h2]:mt-4 [&>h2]:mb-2 [&>h3]:text-[13px] [&>h3]:font-semibold [&>h3]:mt-3 [&>h3]:mb-1.5 [&>ul]:space-y-1 [&>ul>li]:text-[12px]";

function ReviewDetailPanel({
  artifact, onClose,
}: {
  artifact: Artifact; onClose: () => void;
}) {
  const isVerdict = artifact.type === "review-findings";
  const parsed = isVerdict ? parseVerdictContent(artifact.content) : null;
  const cfg = parsed?.verdict ? VERDICT_CFG[parsed.verdict] : null;

  return (
    <div className="flex-1 min-w-0 flex flex-col border-l border-border/20 overflow-hidden">
      {/* Header */}
      <div className="flex items-center gap-2 px-4 pt-3 pb-2.5 shrink-0 border-b border-border/20">
        {cfg
          ? <span className={cn("shrink-0", cfg.cls.split(" ")[0])}>{cfg.icon}</span>
          : <Gavel className="w-3.5 h-3.5 text-status-approved/60 shrink-0" />
        }
        <span className="text-[13px] font-[550] text-foreground flex-1 truncate">{artifact.title}</span>
        <span className="text-[9px] text-muted-foreground/40 font-mono shrink-0">
          {new Date(artifact.updatedAt * 1000).toLocaleString()}
        </span>
        <button
          onClick={onClose}
          className="p-1 rounded hover:bg-accent text-muted-foreground hover:text-foreground transition-colors shrink-0"
        >
          <X className="w-3.5 h-3.5" />
        </button>
      </div>

      {/* Content */}
      <div className="flex-1 min-h-0 overflow-y-auto px-4 py-4 space-y-4">
        {parsed && cfg ? (
          <>
            <div className={cn("inline-flex items-center gap-2 px-3 py-1.5 rounded-lg border text-[13px] font-semibold", cfg.cls)}>
              {cfg.icon} {cfg.label}
            </div>

            {parsed.findings.length > 0 && (
              <div>
                <h3 className="text-[11px] font-semibold text-muted-foreground/60 uppercase tracking-wider mb-2">Findings</h3>
                <div className="space-y-1.5">
                  {parsed.findings.map((f, i) => (
                    <div key={i} className="flex gap-2 text-[12px] text-foreground/80 leading-snug">
                      <span className="text-muted-foreground/40 shrink-0 font-mono text-[10px] pt-0.5">{i + 1}.</span>
                      <span>{f}</span>
                    </div>
                  ))}
                </div>
              </div>
            )}
            {parsed.findings.length === 0 && (
              <p className="text-[12px] text-muted-foreground/40">No findings</p>
            )}

            {parsed.recommendations.length > 0 && (
              <div>
                <h3 className="text-[11px] font-semibold text-muted-foreground/60 uppercase tracking-wider mb-2">Recommendations</h3>
                <div className="space-y-1.5">
                  {parsed.recommendations.map((r, i) => (
                    <div key={i} className="flex gap-2 text-[12px] text-foreground/60 leading-snug">
                      <span className="text-muted-foreground/30 shrink-0">•</span>
                      <span>{r}</span>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </>
        ) : (
          <div className={PROSE_CLS}>
            <ReactMarkdown remarkPlugins={REMARK_PLUGINS}>
              {artifact.content}
            </ReactMarkdown>
          </div>
        )}
      </div>
    </div>
  );
}

// ─── ReviewPanel ──────────────────────────────────────────────────────────────

export function ReviewPanel() {
  const artifacts = useChatStore((s) => s.artifacts);
  const [detailArtifact, setDetailArtifact] = useState<Artifact | null>(null);

  const VERDICT_ORDER: Record<string, number> = { FAIL: 0, CONDITIONAL: 1, PASS: 2 };
  const reviewFindings = artifacts
    .filter((a) => a.type === "review-findings")
    .sort((a, b) => {
      const va = parseVerdictContent(a.content).verdict ?? "PASS";
      const vb = parseVerdictContent(b.content).verdict ?? "PASS";
      const orderDiff = (VERDICT_ORDER[va] ?? 9) - (VERDICT_ORDER[vb] ?? 9);
      return orderDiff !== 0 ? orderDiff : b.updatedAt - a.updatedAt;
    });

  return (
    <div className="flex min-h-0">
      {/* Left: findings list */}
      <div className={cn(
        "overflow-y-auto p-3 space-y-2",
        detailArtifact ? "w-[42%] shrink-0" : "flex-1",
      )}>
        {reviewFindings.length === 0 ? (
          <div className="text-center py-6">
            <FileSearch className="w-5 h-5 text-muted-foreground/20 mx-auto mb-2" />
            <p className="text-[11px] text-muted-foreground/40">No review findings yet</p>
          </div>
        ) : (
          reviewFindings.map((a) => (
            <VerdictCard key={a.id} artifact={a} active={a.id === detailArtifact?.id} onOpen={setDetailArtifact} />
          ))
        )}
      </div>

      {/* Right: detail */}
      {detailArtifact && (
        <ReviewDetailPanel
          artifact={detailArtifact}
          onClose={() => setDetailArtifact(null)}
        />
      )}
    </div>
  );
}
