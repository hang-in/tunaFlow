/**
 * Claude transport flip hardening (T4) + SDK window guard (Task 02) — UI 가시화.
 *
 * SSOT:
 * - docs/plans/claudeTransportFlipHardeningPlan_2026-04-29.md Task 04
 * - docs/plans/claudeSdkSessionWindowGuardPlan_2026-05-09.md Task 02
 *
 * Backend 가 emit 하는 세 이벤트를 단일 컴포넌트에서 listen:
 *
 *  - `claude:fresh_fallback` — stale resume_token detect 후 자동 fresh session
 *    fallback 발생. 사용자에게 "다음 응답부터 ContextPack revival" 안내 토스트
 *    1회 (conversation 별 sessionStorage flag 로 spam 차단).
 *
 *  - `claude:rate_limit` — Anthropic 측 rate_limit_event payload 도착. 가장
 *    최근 값을 RuntimeStatusBar 의 indicator (별도 컴포넌트) 가 읽도록 store
 *    상태로 보관. (status: ok/approaching/limit_reached + reset 시각 + overage
 *    상태)
 *
 *  - `tunaflow:sdk-session-window-rotated` — SDK 누적 input_tokens 가 임계
 *    (180K default / 900K `[1m]`) 도달 시 자동 fresh-rotate 발생. 사용자에게
 *    "세션 컨텍스트 한계 도달, 새 세션으로 자동 전환" 안내 토스트 (info 레벨,
 *    5초 자동 dismiss). conversation 별 sessionStorage flag 로 spam 차단 —
 *    같은 conv 의 반복 rotate 마다 토스트 한 번만.
 *
 * 회귀 가드:
 *  - 본 컴포넌트는 AppShell 안에 1회 mount → listener 가 앱 lifetime 한 번만 wire.
 *  - 기존 progress/chunk/completed/error listener (agentStreamHelper) 영향 0.
 *  - 다른 엔진의 같은 이름 이벤트 없음 (claude 한정 prefix + tunaflow: prefix) — 회귀 0.
 */
import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";
import { useTranslation } from "react-i18next";
import { useClaudeRateLimitStore, type ClaudeRateLimitInfo } from "@/stores/claudeRateLimitStore";

interface FreshFallbackPayload {
  messageId: string;
  conversationId: string;
  engine: string;
}

interface RateLimitPayload {
  conversationId: string;
  engine: string;
  rateLimit: ClaudeRateLimitInfo;
}

/**
 * (claudeSdkSessionWindowGuardPlan Task 02) SDK 누적 window guard fresh-rotate
 * 발생 시 backend → frontend payload. backend 의 `RunOutput.window_rotated` 가
 * `Some(WindowRotatedInfo)` 일 때만 발행.
 */
interface WindowRotatedPayload {
  messageId: string;
  conversationId: string;
  engine: string;
  priorTokens: number;
  threshold: number;
}

/** sessionStorage key — 같은 conversation 의 fallback 토스트 재표시 차단. */
function fallbackToastKey(conversationId: string): string {
  return `tunaflow.claudeFreshFallbackShown.${conversationId}`;
}

/** sessionStorage key — 같은 conversation 의 window-rotated 토스트 재표시 차단. */
function windowRotatedToastKey(conversationId: string): string {
  return `tunaflow.sdkWindowRotatedShown.${conversationId}`;
}

/**
 * Claude fallback / rate_limit event listener — AppShell 에 1회 mount.
 * 시각 출력 없음 (토스트는 sonner, indicator 는 별도 컴포넌트). null 반환.
 */
export function ClaudeFallbackEvents(): null {
  const { t } = useTranslation("runtime");
  const setRateLimit = useClaudeRateLimitStore((s) => s.setRateLimit);

  useEffect(() => {
    let unlistenFallback: (() => void) | null = null;
    let unlistenRateLimit: (() => void) | null = null;
    let unlistenWindowRotated: (() => void) | null = null;
    let cancelled = false;

    (async () => {
      unlistenFallback = await listen<FreshFallbackPayload>(
        "claude:fresh_fallback",
        (e) => {
          const { conversationId } = e.payload;
          // Spam 차단 — 같은 conversation 의 첫 fallback 만 토스트.
          // sessionStorage 라 앱 재시작 후 reset (다음 세션에서 한 번 더).
          let alreadyShown = false;
          try {
            alreadyShown = sessionStorage.getItem(fallbackToastKey(conversationId)) === "1";
          } catch {
            // sessionStorage unavailable (private mode 등) — 무시, 매번 토스트 OK
          }
          if (alreadyShown) return;
          try {
            sessionStorage.setItem(fallbackToastKey(conversationId), "1");
          } catch {
            /* ignore */
          }

          toast.info(t("claude.freshFallback.title"), {
            description: t("claude.freshFallback.body"),
            duration: 8000,
            action: {
              label: t("claude.freshFallback.dismiss"),
              onClick: () => {
                /* sonner auto-dismiss — explicit 확인 액션만 */
              },
            },
          });
        },
      );

      unlistenRateLimit = await listen<RateLimitPayload>(
        "claude:rate_limit",
        (e) => {
          if (cancelled) return;
          setRateLimit(e.payload.conversationId, e.payload.rateLimit);
        },
      );

      // (claudeSdkSessionWindowGuardPlan Task 02) SDK 누적 window guard
      // fresh-rotate 발생 알림.
      unlistenWindowRotated = await listen<WindowRotatedPayload>(
        "tunaflow:sdk-session-window-rotated",
        (e) => {
          if (cancelled) return;
          const { conversationId } = e.payload;
          // Spam 차단 — 같은 conversation 의 첫 rotate 만 토스트.
          let alreadyShown = false;
          try {
            alreadyShown =
              sessionStorage.getItem(windowRotatedToastKey(conversationId)) === "1";
          } catch {
            /* sessionStorage unavailable */
          }
          if (alreadyShown) return;
          try {
            sessionStorage.setItem(windowRotatedToastKey(conversationId), "1");
          } catch {
            /* ignore */
          }

          toast.info(t("claude.windowRotated.title"), {
            description: t("claude.windowRotated.body"),
            duration: 5000,
            action: {
              label: t("claude.windowRotated.dismiss"),
              onClick: () => {
                /* sonner auto-dismiss — explicit 확인 액션만 */
              },
            },
          });
        },
      );
    })().catch((err) => {
      console.warn("[claude-fallback] listener setup failed:", err);
    });

    return () => {
      cancelled = true;
      if (unlistenFallback) unlistenFallback();
      if (unlistenRateLimit) unlistenRateLimit();
      if (unlistenWindowRotated) unlistenWindowRotated();
    };
  }, [setRateLimit, t]);

  return null;
}
