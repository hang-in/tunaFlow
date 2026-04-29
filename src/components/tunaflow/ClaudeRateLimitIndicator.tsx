/**
 * claudeTransportFlipHardeningPlan T4 — Claude rate_limit_event 의 status 를
 * RuntimeStatusBar 안에 dot indicator + 텍스트 hint 로 표시.
 *
 * 데이터 source: useClaudeRateLimitStore (ClaudeFallbackEvents 가 갱신).
 * 표시 우선순위: 현재 selected conv 의 정보 → 없으면 latest conv 의 정보 →
 * 없으면 미표시.
 *
 * 색상 규칙 (Anthropic 정의 status 기반):
 *  - "ok" / 미정의 → 초록 dot, 라벨 미표시 (조용)
 *  - "approaching_limit" → 노란 dot, "Approaching..." 라벨
 *  - "limit_reached" + overage_status="disabled" → 빨간 dot, "Overage disabled"
 *  - "limit_reached" → 주황 dot, "Limit reached"
 *
 * 클릭 시 detail tooltip 또는 settings 링크 — 본 minimal 구현은 title 속성만.
 *
 * 회귀 가드:
 *  - 본 컴포넌트는 store data 가 없으면 null 반환 → 기존 RuntimeStatusBar 영향 0.
 *  - 다른 indicator (rateLimit, gitStatus) 와 영역 분리.
 */
import { useTranslation } from "react-i18next";
import { useChatStore } from "@/stores/chatStore";
import { useClaudeRateLimitStore, type ClaudeRateLimitInfo } from "@/stores/claudeRateLimitStore";
import { cn } from "@/lib/utils";

type StatusKind = "ok" | "approaching" | "limitReached" | "overageDisabled";

function selectStatusKind(info: ClaudeRateLimitInfo): StatusKind {
  const status = info.status ?? null;
  const overageDisabled = info.overageStatus === "disabled";
  const isLimitReached = status === "limit_reached";

  if (isLimitReached && overageDisabled) return "overageDisabled";
  if (isLimitReached) return "limitReached";
  if (status === "approaching_limit") return "approaching";
  return "ok";
}

function dotClassFor(kind: StatusKind): string {
  switch (kind) {
    case "overageDisabled": return "bg-red-500";
    case "limitReached": return "bg-amber-500";
    case "approaching": return "bg-yellow-400";
    default: return "bg-status-approved";
  }
}

export function ClaudeRateLimitIndicator() {
  const { t } = useTranslation("runtime");
  const selectedConvId = useChatStore((s) => s.selectedConversationId);
  const byConversation = useClaudeRateLimitStore((s) => s.byConversation);
  const latestConvId = useClaudeRateLimitStore((s) => s.latestConversationId);

  const convId = selectedConvId && byConversation[selectedConvId] ? selectedConvId : latestConvId;
  const info = convId ? byConversation[convId] : null;
  if (!info) return null;

  const kind = selectStatusKind(info);
  // 정상 상태는 자리 차지 안 하게 hide.
  if (kind === "ok") return null;

  const labelText: string =
    kind === "approaching" ? t("claude.rateLimit.approaching")
    : kind === "limitReached" ? t("claude.rateLimit.limitReached")
    : t("claude.rateLimit.overageDisabled");

  const tooltipParts: string[] = [labelText];
  if (info.resetsAt) {
    tooltipParts.push(t("claude.rateLimit.resetsIn", { when: info.resetsAt }));
  }
  if (info.rateLimitType) {
    const typeLabel: string =
      info.rateLimitType === "5_hour" ? t("claude.rateLimit.fiveHour")
      : info.rateLimitType === "weekly" ? t("claude.rateLimit.weekly")
      : info.rateLimitType;
    tooltipParts.push(typeLabel);
  }
  const tooltip = tooltipParts.join(" · ");

  return (
    <>
      <span className="w-px h-3 bg-border/30" />
      <a
        href="https://claude.ai/settings/usage"
        target="_blank"
        rel="noopener noreferrer"
        title={tooltip}
        className="flex items-center gap-1 px-2 h-full text-muted-foreground/60 hover:text-muted-foreground transition-colors text-tf-micro"
      >
        <span className={cn("w-1.5 h-1.5 rounded-full shrink-0", dotClassFor(kind))} />
        <span className="truncate max-w-[140px]">{labelText}</span>
      </a>
    </>
  );
}
