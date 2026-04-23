import { useState } from "react";
import { useTranslation } from "react-i18next";
import { ChevronDown, ChevronRight } from "lucide-react";
import type { InsightFinding } from "@/types";
import type { QuadrantKey } from "./insightConstants";
import { FindingRow } from "./InsightFindingCards";

export const QUADRANT_META: Record<QuadrantKey, { label: string; desc: string }> = {
  "quick-wins": { label: "Quick Wins", desc: "auto + high impact" },
  "strategic": { label: "Strategic", desc: "guided + high impact" },
  "fill-ins": { label: "Fill-ins", desc: "low impact" },
  "deprioritize": { label: "Deprioritize", desc: "manual" },
};

export function QuadrantSection({
  quadrant,
  findings,
  selectedIds,
  activeFindingId,
  onToggle,
  onSelect,
  onAutoFix,
}: {
  quadrant: QuadrantKey;
  findings: InsightFinding[];
  selectedIds: Set<string>;
  activeFindingId: string | null;
  onToggle: (id: string) => void;
  onSelect: (f: InsightFinding) => void;
  onAutoFix?: () => void;
}) {
  const { t } = useTranslation("insight");
  const [collapsed, setCollapsed] = useState(quadrant === "deprioritize");
  const meta = QUADRANT_META[quadrant];
  const open = findings.filter((f) => f.status !== "resolved" && f.status !== "dismissed");

  if (findings.length === 0) return null;

  return (
    <div className="space-y-1">
      <div className="flex items-center">
        <button
          onClick={() => setCollapsed(!collapsed)}
          className="flex items-center gap-1 text-tf-xs font-medium text-prose-muted hover:text-foreground flex-1"
        >
          {collapsed ? <ChevronRight className="w-3 h-3" /> : <ChevronDown className="w-3 h-3" />}
          {meta.label}
          <span className="text-tf-micro text-prose-disabled">— {meta.desc} ({open.length})</span>
        </button>
        {quadrant === "quick-wins" && open.length > 0 && (
          <span
            title={t("quadrant.auto_fix_tooltip")}
            className="text-tf-micro px-1.5 py-0.5 rounded bg-muted/30 text-prose-disabled cursor-not-allowed"
          >
            {t("quadrant.auto_fix_label")}
          </span>
        )}
      </div>
      {!collapsed && (
        <div className="space-y-0.5">
          {findings.map((f) => (
            <FindingRow
              key={f.id}
              finding={f}
              checked={selectedIds.has(f.id)}
              active={f.id === activeFindingId}
              onToggle={onToggle}
              onSelect={onSelect}
            />
          ))}
        </div>
      )}
    </div>
  );
}
