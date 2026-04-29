/**
 * Claude transport flip hardening (T4) — UI 가시화.
 *
 * SSOT: docs/plans/claudeTransportFlipHardeningPlan_2026-04-29.md Task 04.
 *
 * Backend (T2+T3) 가 emit 하는 두 이벤트를 단일 컴포넌트에서 listen:
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
 * 회귀 가드:
 *  - 본 컴포넌트는 AppShell 안에 1회 mount → listener 가 앱 lifetime 한 번만 wire.
 *  - 기존 progress/chunk/completed/error listener (agentStreamHelper) 영향 0.
 *  - 다른 엔진의 같은 이름 이벤트 없음 (claude 한정 prefix) — 회귀 0.
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

/** sessionStorage key — 같은 conversation 의 fallback 토스트 재표시 차단. */
function fallbackToastKey(conversationId: string): string {
  return `tunaflow.claudeFreshFallbackShown.${conversationId}`;
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
    })().catch((err) => {
      console.warn("[claude-fallback] listener setup failed:", err);
    });

    return () => {
      cancelled = true;
      if (unlistenFallback) unlistenFallback();
      if (unlistenRateLimit) unlistenRateLimit();
    };
  }, [setRateLimit, t]);

  return null;
}
