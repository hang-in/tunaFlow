import { useEffect, useState } from "react";
import { toast } from "sonner";
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
      toast.success(next ? "Background insight 활성화" : "Background insight 비활성화 (큐 보존)");
    } catch (e) {
      toast.error(`토글 실패: ${e}`);
    }
  };

  const handleThresholdChange = async (next: number) => {
    setThreshold(next);
    try {
      await setIdentityAnalysisThreshold(next);
    } catch (e) {
      toast.error(`threshold 저장 실패: ${e}`);
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
          ? "분석 job enqueue — 30s 내 worker 가 실행"
          : `skip — ${decision.reason}`,
      );
    } catch (e) {
      toast.error(`실행 실패: ${e}`);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="space-y-5">
      <div>
        <h2 className="text-[14px] font-[550] text-foreground mb-1 flex items-center gap-2">
          <Brain className="w-4 h-4" />
          Identity Analysis
        </h2>
        <p className="text-[12px] text-muted-foreground leading-relaxed">
          Plan 완료 artifact 를 주기적으로 분석해 프로젝트 정체성 요약을 생성합니다.
          ContextPack 에 자동 주입되어 이후 agent 응답의 정합성을 높입니다.
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
        Background 분석 활성화 (OFF 시 worker 는 pending job 을 pick 하지 않음)
      </label>

      {/* Threshold slider */}
      <div className="space-y-1">
        <label className="text-[12px] font-medium text-foreground/80 flex items-center justify-between">
          <span>Eligible artifact threshold</span>
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
          Plan done 이 3의 배수이고, 이전 분석 이후 누적된 eligible artifact 수가 이 값 이상이면 자동 실행됩니다.
          기본 10 (범위 3~50).
        </p>
      </div>

      {/* TriggerStatus */}
      {projectKey && status && (
        <div className="text-[11px] bg-muted/30 rounded-md p-3 space-y-1">
          <div className="text-[10px] text-muted-foreground uppercase tracking-wide">현재 상태</div>
          <div>Plans done: <span className="font-mono">{status.donePlanCount}</span></div>
          <div>Eligible artifacts: <span className="font-mono">{status.eligibleArtifactCount} / {status.threshold}</span></div>
          <div className="text-muted-foreground">reason: <span className="font-mono">{status.reason}</span></div>
          <div className="text-muted-foreground/80">
            {status.shouldRun ? "✓ 다음 Plan 완료 시 자동 실행" : "⏸ 조건 미충족"}
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
          {busy ? "enqueue 중..." : "지금 실행 (threshold 무시)"}
        </button>
        <span className="text-[11px] text-muted-foreground">
          plan done %3 조건은 유지됩니다 (partial period 분석 품질 저하 방지)
        </span>
      </div>
    </div>
  );
}
