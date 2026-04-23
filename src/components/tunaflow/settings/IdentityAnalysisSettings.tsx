import { useEffect, useState } from "react";
import { toast } from "sonner";
import { useTranslation } from "react-i18next";
import { Brain, Play } from "lucide-react";
import { useChatStore } from "@/stores/chatStore";
import {
  getBackgroundInsightEnabled,
  setBackgroundInsightEnabled,
  getIdentityAnalysisThreshold,
  setIdentityAnalysisThreshold,
  getIdentityTriggerStatus,
  triggerIdentityAnalysisNow,
  type IdentityTriggerDecision,
} from "@/lib/api/identityAnalysis";

/** projectIdentityAnalysisPlan subtask-04 follow-up — Settings > Identity.
 *
 *  - BACKGROUND_INSIGHT_ENABLED 토글 (INV-3, 기본 ON)
 *  - threshold 슬라이더 (3~50, 기본 10, env var 우선순위)
 *  - "지금 실행 (threshold 무시)" 버튼
 *  - TriggerStatus 배지 (Plans done / Eligible / reason)
 */
export function IdentityAnalysisSettings() {
  const { t } = useTranslation("settings");
  const projectKey = useChatStore((s) => s.selectedProjectKey);
  const [enabled, setEnabled] = useState(true);
  const [threshold, setThreshold] = useState(10);
  const [status, setStatus] = useState<IdentityTriggerDecision | null>(null);
  const [busy, setBusy] = useState(false);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let alive = true;
    (async () => {
      try {
        const [en, th] = await Promise.all([
          getBackgroundInsightEnabled(),
          getIdentityAnalysisThreshold(),
        ]);
        if (!alive) return;
        setEnabled(en);
        setThreshold(th);
        if (projectKey) {
          try {
            const st = await getIdentityTriggerStatus(projectKey);
            if (alive) setStatus(st);
          } catch { /* status 실패는 무시 */ }
        }
      } catch (e) {
        console.error("[identity-settings] load failed", e);
      } finally {
        if (alive) setLoading(false);
      }
    })();
    return () => {
      alive = false;
    };
  }, [projectKey]);

  const handleToggle = async (next: boolean) => {
    try {
      await setBackgroundInsightEnabled(next);
      setEnabled(next);
      toast.success(t(next ? "identity.toast.toggle_on" : "identity.toast.toggle_off"));
    } catch (e) {
      toast.error(t("identity.toast.toggle_failed", { error: String(e) }));
    }
  };

  const handleThresholdChange = async (next: number) => {
    setThreshold(next);
    try {
      await setIdentityAnalysisThreshold(next);
    } catch (e) {
      toast.error(t("identity.toast.threshold_save_failed", { error: String(e) }));
    }
  };

  const handleRunNow = async () => {
    if (!projectKey || busy) return;
    setBusy(true);
    try {
      const decision = await triggerIdentityAnalysisNow(projectKey, true);
      setStatus(decision);
      toast.success(
        decision.shouldRun
          ? t("identity.toast.enqueued")
          : t("identity.toast.skipped", { reason: decision.reason }),
      );
    } catch (e) {
      toast.error(t("identity.toast.run_failed", { error: String(e) }));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="space-y-5">
      <div>
        <h2 className="text-[14px] font-[550] text-foreground mb-1 flex items-center gap-2">
          <Brain className="w-4 h-4" />
          {t("identity.heading")}
        </h2>
        <p className="text-[12px] text-muted-foreground leading-relaxed">
          {t("identity.description")}
        </p>
      </div>

      {/* Toggle */}
      <label className="flex items-center gap-2 text-[12px] text-foreground/80 cursor-pointer">
        <input
          type="checkbox"
          checked={enabled}
          disabled={loading}
          onChange={(e) => handleToggle(e.target.checked)}
          className="accent-primary"
        />
        {t("identity.toggle_label")}
      </label>

      {/* Threshold slider */}
      <div className="space-y-1">
        <label className="text-[12px] font-medium text-foreground/80 flex items-center justify-between">
          <span>{t("identity.threshold_label")}</span>
          <span className="text-[11px] text-muted-foreground font-mono">{threshold}</span>
        </label>
        <input
          type="range"
          min={3}
          max={50}
          step={1}
          value={threshold}
          disabled={loading}
          onChange={(e) => handleThresholdChange(Number(e.target.value))}
          className="w-full accent-primary"
        />
        <p className="text-[11px] text-muted-foreground/70">
          {t("identity.threshold_hint")}
        </p>
      </div>

      {/* TriggerStatus */}
      {projectKey && status && (
        <div className="text-[11px] bg-muted/30 rounded-md p-3 space-y-1">
          <div className="text-[10px] text-muted-foreground uppercase tracking-wide">{t("identity.status.heading")}</div>
          <div>{t("identity.status.plans_done")} <span className="font-mono">{status.donePlanCount}</span></div>
          <div>{t("identity.status.eligible")} <span className="font-mono">{status.eligibleArtifactCount} / {status.threshold}</span></div>
          <div className="text-muted-foreground">{t("identity.status.reason")} <span className="font-mono">{status.reason}</span></div>
          <div className="text-muted-foreground/80">
            {status.shouldRun ? t("identity.status.will_run") : t("identity.status.wont_run")}
          </div>
        </div>
      )}

      {/* Manual run */}
      <div className="flex items-center gap-2">
        <button
          onClick={handleRunNow}
          disabled={!projectKey || busy}
          className="flex items-center gap-1.5 px-3 py-1.5 text-[12px] font-medium rounded-md bg-primary text-primary-foreground hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
          <Play className="w-3 h-3" />
          {busy ? t("identity.button.enqueuing") : t("identity.button.run_now")}
        </button>
        <span className="text-[11px] text-muted-foreground">
          {t("identity.force_hint")}
        </span>
      </div>
    </div>
  );
}
