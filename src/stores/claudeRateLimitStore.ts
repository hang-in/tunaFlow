/**
 * claudeTransportFlipHardeningPlan T4 — Claude rate_limit 상태 저장소.
 *
 * Backend (claude.rs T1) 가 stream-json 안의 `rate_limit_event` 를 parse 해
 * `claude:rate_limit` 이벤트로 emit. ClaudeFallbackEvents 컴포넌트가 받아 본
 * store 에 저장 → RuntimeStatusBar 의 ClaudeRateLimitIndicator 가 read.
 *
 * 데이터는 conversation 별로 마지막 1건만 보관. 이전 plan 의 `ccusage` 산출
 * 기반 rateLimit (5h/7d 비율) 과는 다른 source. 둘 다 RuntimeStatusBar 에
 * 표시 가능 (영역 분리).
 */
import { create } from "zustand";

/** Backend RateLimitInfo struct 의 camelCase JSON. */
export interface ClaudeRateLimitInfo {
  status?: string | null;
  resetsAt?: string | null;
  rateLimitType?: string | null;
  overageStatus?: string | null;
  overageDisabledReason?: string | null;
  isUsingOverage?: boolean | null;
}

interface ClaudeRateLimitState {
  /** conversationId → 가장 최근 rate_limit_event payload. */
  byConversation: Record<string, ClaudeRateLimitInfo>;
  /** 가장 최근에 갱신된 conversationId. RuntimeStatusBar 가 selected conv 우선
   *  사용하다가 없으면 fallback 으로 본 값을 사용. */
  latestConversationId: string | null;
  setRateLimit: (conversationId: string, info: ClaudeRateLimitInfo) => void;
  clear: (conversationId: string) => void;
}

export const useClaudeRateLimitStore = create<ClaudeRateLimitState>((set) => ({
  byConversation: {},
  latestConversationId: null,
  setRateLimit: (conversationId, info) =>
    set((s) => ({
      byConversation: { ...s.byConversation, [conversationId]: info },
      latestConversationId: conversationId,
    })),
  clear: (conversationId) =>
    set((s) => {
      const next = { ...s.byConversation };
      delete next[conversationId];
      return { byConversation: next };
    }),
}));
