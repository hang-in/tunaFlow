import { useTranslation } from "react-i18next";
import { cn } from "@/lib/utils";
import { CheckSquare, Square, CheckCircle2, Clock, GitBranch } from "lucide-react";
import type { InsightFinding, InsightSeverity } from "@/types";
import { CATEGORY_META, SEVERITY_META } from "./insightConstants";

// ── Summary bar ──────────────────────────────────────────────

export function SummaryBar({ findings }: { findings: InsightFinding[] }) {
  const severityCounts = { critical: 0, major: 0, minor: 0, info: 0 };
  const difficultyCounts = { auto: 0, guided: 0, manual: 0 };
  let resolved = 0;

  for (const f of findings) {
    severityCounts[f.severity] = (severityCounts[f.severity] || 0) + 1;
    difficultyCounts[f.fixDifficulty] = (difficultyCounts[f.fixDifficulty] || 0) + 1;
    if (f.status === "resolved") resolved++;
  }

  return (
    <div className="rounded-md border border-border/30 bg-card/40 p-2 text-[10px] space-y-1">
      <div className="flex gap-3 flex-wrap">
        {Object.entries(severityCounts).map(([k, v]) => v > 0 && (
          <span key={k} className={cn("flex items-center gap-0.5", SEVERITY_META[k as InsightSeverity]?.cls)}>
            {SEVERITY_META[k as InsightSeverity]?.icon} {k}: {v}
          </span>
        ))}
      </div>
      <div className="flex gap-3 text-muted-foreground/60">
        <span>Auto: {difficultyCounts.auto}</span>
        <span>Guided: {difficultyCounts.guided}</span>
        <span>Manual: {difficultyCounts.manual}</span>
        <span className="text-status-approved/60">Resolved: {resolved}</span>
      </div>
    </div>
  );
}

// ── Finding card ─────────────────────────────────────────────

export function FindingRow({
  finding,
  checked,
  active,
  onToggle,
  onSelect,
}: {
  finding: InsightFinding;
  checked: boolean;
  active: boolean;
  onToggle: (id: string) => void;
  onSelect: (f: InsightFinding) => void;
}) {
  const catMeta = CATEGORY_META[finding.category];
  const sevMeta = SEVERITY_META[finding.severity];

  return (
    <div
      onClick={() => onSelect(finding)}
      className={cn(
        "rounded-md border px-2 py-1.5 cursor-pointer transition-colors flex items-center gap-1.5",
        active ? "border-primary/40 bg-primary/5" :
        finding.status === "resolved" ? "border-status-approved/20 bg-status-approved/5 opacity-60" :
        finding.status === "dismissed" ? "border-border/10 opacity-40" :
        "border-border/30 bg-card/60 hover:border-border/50",
      )}
    >
      {(finding.status === "open" || finding.status === "selected") && (
        <button
          onClick={(e) => { e.stopPropagation(); onToggle(finding.id); }}
          className="shrink-0 p-1 -m-1 rounded hover:bg-accent/40 transition-colors"
        >
          {checked ? <CheckSquare className="w-3 h-3 text-accent" /> : <Square className="w-3 h-3 text-muted-foreground/40" />}
        </button>
      )}
      {finding.status === "resolved" && <CheckCircle2 className="w-3 h-3 text-status-approved/60 shrink-0" />}
      {finding.status === "in_progress" && <Clock className="w-3 h-3 text-yellow-500/60 shrink-0" />}

      <span className={cn("shrink-0", catMeta?.color)}>{catMeta?.icon}</span>
      <span className={cn("text-[9px] px-1 py-0.5 rounded shrink-0", sevMeta?.cls)}>{finding.severity}</span>
      <span className="text-tf-sm font-medium text-foreground truncate flex-1">{finding.title}</span>
      {finding.filePath && (
        <span className="text-tf-micro text-prose-disabled font-mono truncate max-w-[120px] shrink-0">
          {finding.filePath.split("/").pop()}
        </span>
      )}
    </div>
  );
}

// ── Detail panel ─────────────────────────────────────────────

export function FindingDetail({ finding, onSendToArchitect }: { finding: InsightFinding; onSendToArchitect?: (f: InsightFinding) => void }) {
  const { t } = useTranslation("insight");
  const catMeta = CATEGORY_META[finding.category];
  const sevMeta = SEVERITY_META[finding.severity];

  return (
    <div className="p-4 space-y-4 overflow-y-auto h-full">
      {/* Header */}
      <div>
        <div className="flex items-center gap-2 mb-2">
          <span className={cn("shrink-0", catMeta?.color)}>{catMeta?.icon}</span>
          <span className={cn("text-tf-xs px-1.5 py-0.5 rounded inline-flex items-center gap-0.5", sevMeta?.cls)}>
            {sevMeta?.icon} {finding.severity}
          </span>
          <span className="text-tf-xs text-prose-faint px-1.5 py-0.5 rounded bg-muted/40">
            {finding.fixDifficulty}
          </span>
          <span className="text-tf-xs text-prose-disabled">
            {finding.status}
          </span>
        </div>
        <h3 className="text-tf-base font-semibold text-foreground">{finding.title}</h3>
      </div>

      {/* File path */}
      {finding.filePath && (
        <div className="text-tf-sm text-prose-muted font-mono bg-muted/20 rounded px-2 py-1">
          {finding.filePath}{finding.lineNumber ? `:${finding.lineNumber}` : ""}
        </div>
      )}

      {/* Description */}
      <div className="text-tf-sm text-prose-base leading-relaxed whitespace-pre-wrap">
        {finding.description}
      </div>

      {/* Code snippet */}
      {finding.snippet && (
        <div>
          <p className="text-tf-xs text-prose-faint mb-1">{t("finding_card.code_label")}</p>
          <pre className="text-tf-sm bg-[#1e1e1e] text-[#d4d4d4] rounded-md p-3 overflow-x-auto font-mono leading-relaxed">
            {finding.snippet}
          </pre>
        </div>
      )}

      {/* Resolution (resolved/done findings) */}
      {finding.resolution && (
        <div>
          <p className="text-tf-xs text-status-approved/70 mb-1 font-medium">{t("finding_card.solution_label")}</p>
          <div className="text-tf-sm text-prose-base bg-status-approved/5 border border-status-approved/20 rounded-md p-3 whitespace-pre-wrap">
            {finding.resolution}
          </div>
        </div>
      )}

      {/* Meta */}
      {finding.estimatedFiles && finding.estimatedFiles > 1 && (
        <p className="text-tf-xs text-prose-disabled">
          {t("finding_card.estimated_files", { count: finding.estimatedFiles })}
        </p>
      )}

      {/* Actions */}
      {(finding.status === "open" || finding.status === "selected") && onSendToArchitect && (
        <button
          onClick={() => onSendToArchitect(finding)}
          className="flex items-center gap-1 text-tf-xs px-2 py-1 rounded font-medium bg-primary/10 text-primary hover:bg-primary/20 transition-colors"
        >
          <GitBranch className="w-3 h-3" />
          {t("finding_card.architect_review_button")}
        </button>
      )}
    </div>
  );
}
