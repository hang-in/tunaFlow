import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import ReactMarkdown from "react-markdown";
import { REMARK_PLUGINS } from "@/lib/markdownPlugins";
import { RefreshCw, Clock, AlertTriangle, Check } from "lucide-react";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import {
  listIdentitySummaries,
  triggerIdentityAnalysisNow,
  getIdentityTriggerStatus,
  type IdentityTriggerDecision,
} from "@/lib/api/identityAnalysis";
import {
  parseIdentitySummary,
  type ParsedIdentity,
  type InflectionPoint,
} from "@/lib/parseIdentitySummary";
import type { Artifact } from "@/types";

/** projectIdentityAnalysisPlan subtask-04 — Insight 탭 Identity 뷰.
 *
 *  - 최신 summary 렌더 + 과거 summary 스위처
 *  - "강제 실행" 버튼 (trigger_identity_analysis_now force=true)
 *  - TriggerStatus 배지 (plan done / eligible / threshold / reason)
 *  - Empty state — summary 없을 때 조건 안내
 */
export function IdentityView() {
  const { t } = useTranslation("insight");
  const projectKey = useChatStore((s) => s.selectedProjectKey);
  const [summaries, setSummaries] = useState<Artifact[]>([]);
  const [selected, setSelected] = useState<Artifact | null>(null);
  const [parsed, setParsed] = useState<ParsedIdentity | null>(null);
  const [status, setStatus] = useState<IdentityTriggerDecision | null>(null);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    if (!projectKey) return;
    let alive = true;
    Promise.all([
      listIdentitySummaries(projectKey),
      getIdentityTriggerStatus(projectKey).catch(() => null),
    ])
      .then(([list, st]) => {
        if (!alive) return;
        setSummaries(list);
        setSelected(list[0] ?? null);
        if (st) setStatus(st);
      })
      .catch((e) => {
        if (alive) setErr(String(e));
      });
    return () => {
      alive = false;
    };
  }, [projectKey]);

  useEffect(() => {
    setParsed(selected ? parseIdentitySummary(selected.content) : null);
  }, [selected]);

  const handleRun = async (force: boolean) => {
    if (!projectKey || busy) return;
    setBusy(true);
    setErr(null);
    try {
      const decision = await triggerIdentityAnalysisNow(projectKey, force);
      setStatus(decision);
      // 분석 완료는 worker tick 이후라 즉시 반영 안 됨. 20s 뒤 list 재조회.
      setTimeout(async () => {
        try {
          const list = await listIdentitySummaries(projectKey);
          setSummaries(list);
          if (list[0]) setSelected(list[0]);
        } catch (e) {
          console.warn("[identity] reload after trigger failed", e);
        }
      }, 20_000);
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  };

  if (!projectKey) {
    return <div className="p-4 text-[11px] text-muted-foreground">{t("identity.empty_project")}</div>;
  }

  return (
    <div className="flex flex-col h-full overflow-hidden">
      <Toolbar
        summaries={summaries}
        selected={selected}
        onSelect={setSelected}
        onRun={handleRun}
        busy={busy}
      />
      {status && <TriggerStatusBar status={status} />}
      {err && <ErrorBanner msg={err} />}
      <div className="flex-1 overflow-y-auto">
        {selected && parsed ? (
          <SummaryBody summary={selected} parsed={parsed} />
        ) : (
          <EmptyState status={status} onForce={() => handleRun(true)} busy={busy} />
        )}
      </div>
    </div>
  );
}

function Toolbar({
  summaries,
  selected,
  onSelect,
  onRun,
  busy,
}: {
  summaries: Artifact[];
  selected: Artifact | null;
  onSelect: (a: Artifact) => void;
  onRun: (force: boolean) => void;
  busy: boolean;
}) {
  const { t } = useTranslation("insight");
  return (
    <div className="flex items-center gap-2 px-3 py-2 border-b border-border/20 shrink-0">
      <button
        onClick={() => onRun(true)}
        disabled={busy}
        className={cn(
          "flex items-center gap-1 text-[10px] px-2 py-1 rounded font-medium transition-colors",
          busy
            ? "bg-muted text-muted-foreground cursor-not-allowed"
            : "bg-accent text-accent-foreground hover:bg-accent/80",
        )}
      >
        {busy ? <RefreshCw className="w-3 h-3 animate-spin" /> : <Check className="w-3 h-3" />}
        {busy ? t("identity.force_busy") : t("identity.force_button")}
      </button>
      {summaries.length > 0 && (
        <select
          value={selected?.id ?? ""}
          onChange={(e) => {
            const next = summaries.find((s) => s.id === e.target.value);
            if (next) onSelect(next);
          }}
          className="text-[10px] bg-input border border-border/40 rounded px-2 py-1 outline-none"
        >
          {summaries.map((s) => (
            <option key={s.id} value={s.id}>
              {fmtTs(s.createdAt)} — {s.title}
            </option>
          ))}
        </select>
      )}
      <span className="text-[10px] text-muted-foreground ml-auto">
        total {summaries.length}
      </span>
    </div>
  );
}

function TriggerStatusBar({ status }: { status: IdentityTriggerDecision }) {
  const done3 = status.donePlanCount % 3 === 0 && status.donePlanCount > 0;
  const volumeOk = status.eligibleArtifactCount >= status.threshold;
  return (
    <div className="flex items-center gap-3 px-3 py-1.5 text-[10px] text-muted-foreground border-b border-border/10 shrink-0">
      <span className={cn(done3 ? "text-status-approved" : "")}>
        Plans done: {status.donePlanCount}{" "}
        {done3 ? "(OK)" : `(need ${3 - (status.donePlanCount % 3)} more)`}
      </span>
      <span className={cn(volumeOk ? "text-status-approved" : "")}>
        Eligible: {status.eligibleArtifactCount}/{status.threshold}
      </span>
      <span className="ml-auto">reason: {status.reason}</span>
    </div>
  );
}

function ErrorBanner({ msg }: { msg: string }) {
  return (
    <div className="flex items-center gap-2 px-3 py-2 text-[10px] text-status-rejected bg-status-rejected/5">
      <AlertTriangle className="w-3 h-3" />
      {msg}
    </div>
  );
}

function EmptyState({
  status,
  onForce,
  busy,
}: {
  status: IdentityTriggerDecision | null;
  onForce: () => void;
  busy: boolean;
}) {
  const { t } = useTranslation("insight");
  return (
    <div className="flex flex-col items-center justify-center h-full p-6 gap-3 text-center">
      <p className="text-[12px] text-foreground">{t("identity.empty_title")}</p>
      <p className="text-[10px] text-muted-foreground max-w-[360px] leading-relaxed">
        {t("identity.empty_hint")}
        {status && (
          <>
            {" "}
            {t("identity.status_summary", {
              done: status.donePlanCount,
              eligible: status.eligibleArtifactCount,
              required: status.threshold,
            })}
          </>
        )}
      </p>
      <button
        onClick={onForce}
        disabled={busy}
        className="text-[10px] px-2.5 py-1 rounded bg-accent text-accent-foreground hover:bg-accent/80 disabled:opacity-50"
      >
        {t("identity.force_tooltip")}
      </button>
    </div>
  );
}

function SummaryBody({ summary, parsed }: { summary: Artifact; parsed: ParsedIdentity }) {
  return (
    <div className="p-4 space-y-4">
      <div className="text-[10px] text-muted-foreground flex items-center gap-2">
        <Clock className="w-3 h-3" />
        {fmtTs(summary.createdAt)} — {summary.title}
      </div>

      <Section title="Project identity" content={parsed.sections.projectIdentity} />
      <Section
        title="User working preference"
        content={parsed.sections.userWorkingPreference}
      />
      <Section
        title="Agent operating preference"
        content={parsed.sections.agentOperatingPreference}
      />
      <InflectionTimeline points={parsed.sections.inflectionPoints} />
      <DoAvoidLists items={parsed.sections.doAvoid} />

      {parsed.frontmatter && (
        <div className="text-[9px] text-muted-foreground pt-3 border-t border-border/10">
          <div>artifact_refs: {parsed.frontmatter.artifactRefs.length} items</div>
          {parsed.frontmatter.supersedes && <div>supersedes: {parsed.frontmatter.supersedes}</div>}
        </div>
      )}
    </div>
  );
}

function Section({ title, content }: { title: string; content: string }) {
  const { t } = useTranslation("insight");
  if (!content.trim()) {
    return (
      <div>
        <h4 className="text-[11px] font-semibold text-foreground/80 mb-1">{title}</h4>
        <p className="text-[10px] text-muted-foreground italic">{t("identity.no_data")}</p>
      </div>
    );
  }
  return (
    <div>
      <h4 className="text-[11px] font-semibold text-foreground/80 mb-1">{title}</h4>
      <div className="text-[11px] leading-relaxed prose-tf prose-invert">
        <ReactMarkdown remarkPlugins={REMARK_PLUGINS}>{content}</ReactMarkdown>
      </div>
    </div>
  );
}

function InflectionTimeline({ points }: { points: InflectionPoint[] }) {
  const { t } = useTranslation("insight");
  if (points.length === 0) {
    return (
      <div>
        <h4 className="text-[11px] font-semibold text-foreground/80 mb-1">
          Recent inflection points
        </h4>
        <p className="text-[10px] text-muted-foreground italic">{t("identity.no_data")}</p>
      </div>
    );
  }
  return (
    <div>
      <h4 className="text-[11px] font-semibold text-foreground/80 mb-2">
        Recent inflection points
      </h4>
      <ol className="space-y-2">
        {points.map((p, i) => (
          <li
            key={i}
            className="text-[10px] border-l-2 border-primary/40 pl-2 space-y-0.5"
          >
            <div className="text-foreground/90">{p.what || "(what missing)"}</div>
            {p.why && <div className="text-muted-foreground">why: {p.why}</div>}
            {p.when && <div className="text-muted-foreground">when: {p.when}</div>}
          </li>
        ))}
      </ol>
    </div>
  );
}

function DoAvoidLists({ items }: { items: { do: string[]; avoid: string[] } }) {
  const { t } = useTranslation("insight");
  if (items.do.length === 0 && items.avoid.length === 0) {
    return (
      <div>
        <h4 className="text-[11px] font-semibold text-foreground/80 mb-1">Do / Avoid</h4>
        <p className="text-[10px] text-muted-foreground italic">{t("identity.no_data")}</p>
      </div>
    );
  }
  return (
    <div className="grid grid-cols-2 gap-3">
      <div>
        <h5 className="text-[10px] font-semibold text-status-approved mb-1">Do</h5>
        <ul className="text-[10px] space-y-0.5">
          {items.do.map((d, i) => (
            <li key={i}>• {d}</li>
          ))}
        </ul>
      </div>
      <div>
        <h5 className="text-[10px] font-semibold text-status-rejected mb-1">Avoid</h5>
        <ul className="text-[10px] space-y-0.5">
          {items.avoid.map((a, i) => (
            <li key={i}>• {a}</li>
          ))}
        </ul>
      </div>
    </div>
  );
}

function fmtTs(ms: number): string {
  if (!ms) return "—";
  return new Date(ms).toLocaleString([], {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}
