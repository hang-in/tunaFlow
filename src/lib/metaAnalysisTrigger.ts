/**
 * Meta Analysis Tier 2 — 트리거 체크 + 분석 실행 헬퍼.
 *
 * 호출 시점: review_passed / review_failed / artifact 생성 직후 등. 카운트 체크 후
 * 임계값 도달 시 저비용 엔진(Haiku/Flash)에 분석 요청 → 결과를 Meta 알림으로 등록.
 *
 * **원칙** (meta.md):
 * - 읽기 전용 (DB 쓰기 금지, 조언만)
 * - 사용자 승인 없이 plan/subtask 변경 불가
 */
import { invoke } from "@tauri-apps/api/core";
import { loadMetaConfig, resolveAutoEngine, toBackendEngine, type MetaAnalysisConfig } from "./metaAnalysis";
import { dispatchMetaNotification } from "./metaNotifications";
import type { MetaNotificationKind } from "./metaNotifications";

type TriggerKind = "review_passed" | "review_failed" | "artifact_created" | "idle_check";

/** 누적 카운트 체크 — localStorage 기반 단순 counter (프로젝트/트리거 key 별).
 *  DB 쪽 정확한 카운트는 비용이 더 크니 1차 필터로만 사용. */
const COUNTER_KEY = "meta-trigger-counters-v1";

function loadCounters(): Record<string, number> {
  try {
    const raw = localStorage.getItem(COUNTER_KEY);
    if (raw) return JSON.parse(raw);
  } catch { /* ignore */ }
  return {};
}
function saveCounters(c: Record<string, number>) {
  try { localStorage.setItem(COUNTER_KEY, JSON.stringify(c)); } catch { /* ignore */ }
}

function counterKey(projectKey: string | undefined, trigger: TriggerKind): string {
  return `${projectKey ?? "global"}:${trigger}`;
}

function bump(projectKey: string | undefined, trigger: TriggerKind): number {
  const counters = loadCounters();
  const k = counterKey(projectKey, trigger);
  counters[k] = (counters[k] ?? 0) + 1;
  saveCounters(counters);
  return counters[k];
}

function reset(projectKey: string | undefined, trigger: TriggerKind): void {
  const counters = loadCounters();
  delete counters[counterKey(projectKey, trigger)];
  saveCounters(counters);
}

/** 엔진 결정 — config.engine 이 "auto" 면 stackKeywords 로 자동. "off" 면 null. */
function resolveEngine(config: MetaAnalysisConfig, stackKeywords: string[] = []): { engine: string; model: string } | null {
  if (config.engine === "off") return null;
  const resolved = config.engine === "auto" ? resolveAutoEngine(stackKeywords) : config.engine;
  return toBackendEngine(resolved);
}

/** Tier 2 분석 실행 — 저비용 엔진으로 짧은 분석을 돌려 결과를 Meta 알림으로 등록.
 *  run_eval_agent 커맨드 재사용 (session 저장 없는 일회성 분석). */
async function runTier2Analysis(
  projectKey: string,
  prompt: string,
  config: MetaAnalysisConfig,
  notificationPayload: {
    kind: MetaNotificationKind;
    title: string;
    summaryFallback: string;
  },
): Promise<void> {
  const resolved = resolveEngine(config);
  if (!resolved) return;
  try {
    const result = await invoke<{ content: string }>("run_eval_agent", {
      engine: resolved.engine,
      prompt,
      model: resolved.model,
      projectPath: null,
    });
    const summary = (result.content ?? "").trim().slice(0, 600)
      || notificationPayload.summaryFallback;
    dispatchMetaNotification({
      kind: notificationPayload.kind,
      title: notificationPayload.title,
      summary,
      projectKey,
      route: { tab: "chat" }, // 메타 채팅으로 이동 유도 (상세 대화)
    });
  } catch (e) {
    console.warn("[meta-tier2] analysis failed:", e);
    dispatchMetaNotification({
      kind: notificationPayload.kind,
      title: notificationPayload.title,
      summary: notificationPayload.summaryFallback,
      projectKey,
    });
  }
}

/** 외부 호출용 — 이벤트 발생 지점에서 fire-and-forget 으로 호출.
 *  임계값 도달 시 분석 실행, 아니면 no-op. */
export async function maybeTriggerMetaAnalysis(
  projectKey: string,
  trigger: TriggerKind,
  context: { planTitle?: string; failCount?: number; findings?: string[] } = {},
): Promise<void> {
  const config = await loadMetaConfig();
  if (!config.autoTrigger || config.engine === "off") return;

  const count = bump(projectKey, trigger);
  const t = config.thresholds;

  if (trigger === "review_passed" && count >= t.reviewPassedCount) {
    reset(projectKey, trigger);
    await runTier2Analysis(
      projectKey,
      `최근 ${t.reviewPassedCount}건의 Plan 이 리뷰 통과되었습니다. 프로젝트 전반에서 반복되는 잘된 패턴과 다음 우선순위 작업을 간단히 요약해주세요. 300자 이내.`,
      config,
      {
        kind: "tier2_brief",
        title: `📊 ${t.reviewPassedCount}건 Plan 통과 — 주간 요약`,
        summaryFallback: "분석이 완료되지 않았습니다. 메타 채팅에서 직접 질문해보세요.",
      },
    );
  } else if (trigger === "review_failed" && count >= t.reviewFailedCount) {
    reset(projectKey, trigger);
    const findingsContext = context.findings?.slice(0, 5).join("\n") ?? "";
    await runTier2Analysis(
      projectKey,
      `최근 ${t.reviewFailedCount}건의 Review 가 실패했습니다. 반복되는 실패 패턴(동일 파일/동일 findings)이 있는지 확인하고, 설계 재검토가 필요한 부분을 제안해주세요. 구체적 findings:\n${findingsContext}\n\n300자 이내 한국어 요약.`,
      config,
      {
        kind: "tier2_brief",
        title: `⚠️ ${t.reviewFailedCount}건 Review 실패 — 패턴 분석`,
        summaryFallback: "반복되는 실패 패턴이 있을 수 있습니다. 메타 채팅에서 확인하세요.",
      },
    );
  } else if (trigger === "artifact_created" && count >= t.artifactCount) {
    reset(projectKey, trigger);
    await runTier2Analysis(
      projectKey,
      `프로젝트에 artifacts 가 ${t.artifactCount}건 누적되었습니다. 타입별 분포와 눈에 띄는 설계 결정/회고를 간단히 요약해주세요. 300자 이내.`,
      config,
      {
        kind: "plan_completed",
        title: `📦 Artifacts ${t.artifactCount}건 도달`,
        summaryFallback: "artifacts 가 누적되었습니다. 메타 채팅에서 요약을 요청하세요.",
      },
    );
  }
  // idle_check 는 별도 스케줄러 (향후 작업)
}
