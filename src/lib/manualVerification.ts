/**
 * Manual verification gate (B-19 / Issue #176).
 *
 * Developer 가 `⚠️ Manual:` prefix 로 자기 응답에 열거한 "shell 로 확인 불가한
 * 항목" 을 파싱해 UI 다이얼로그 입력으로 넘긴다. pass/skip/fail 판정은
 * `ManualVerificationResult` 로 돌아와 reviewWorkflow 가 review/rework 분기.
 *
 * 이 파일은 DOM/React 의존 없음 — 순수 TS 유틸. 테스트 mock 쉬움.
 */
import type { Message } from "@/types";

export interface ManualVerificationItem {
  /** "⚠️ Manual:" prefix 제거 후 본문. 사용자에게 그대로 표시된다. */
  label: string;
  /** 항목의 출처. 향후 확장 (plan 선언 manual 등) 을 위한 discriminator. */
  source: "developer";
}

export interface ManualVerificationResult {
  status: "pass" | "skip" | "fail";
  /** fail 일 때 사용자가 선택적으로 입력한 실패 사유. 없으면 undefined. */
  reason?: string;
}

export interface ManualVerificationReport {
  items: ManualVerificationItem[];
  /** `items` 와 동일 길이 · 순서. i18next 무관 — UI 가 결과 수집 후 전달. */
  results: ManualVerificationResult[];
}

/**
 * Developer 의 마지막 assistant 메시지에서 `⚠️ Manual: ...` 라인을 추출.
 *
 * Rework 가 있었으면 마지막 Rework 이후 범위만 본다 (syncResultReport 와 동일
 * 필터링 — 이전 싸이클 항목이 다시 튀어나오지 않도록).
 *
 * ⚠️ 는 U+26A0 U+FE0F (variation selector 포함). 정규식에 UTF-8 리터럴로 그대로.
 */
export function extractManualItems(implMessages: Message[]): ManualVerificationItem[] {
  let lastReworkIdx = -1;
  for (let i = implMessages.length - 1; i >= 0; i--) {
    if (implMessages[i].role === "user" && implMessages[i].content.includes("### 🔄 Rework")) {
      lastReworkIdx = i;
      break;
    }
  }
  const relevant = lastReworkIdx >= 0 ? implMessages.slice(lastReworkIdx + 1) : implMessages;
  const lastAssistant = [...relevant].reverse().find((m) => m.role === "assistant");
  if (!lastAssistant) return [];

  const items: ManualVerificationItem[] = [];
  // `\s` 는 newline 을 포함하므로 `\s*` 가 multi-line 을 가로질러 인접 항목까지
  // 한 매치에 삼킬 수 있다. 수평 공백만 허용하는 `[ \t]*` 로 좁혀 라인 단위
  // 매칭을 보장 (⚠️ 는 U+26A0 U+FE0F 그대로 유지).
  const re = /^[ \t]*⚠️[ \t]*Manual:[ \t]*(.+)$/gm;
  let match: RegExpExecArray | null;
  while ((match = re.exec(lastAssistant.content)) !== null) {
    const label = match[1].trim();
    if (label.length > 0) items.push({ label, source: "developer" });
  }
  return items;
}
