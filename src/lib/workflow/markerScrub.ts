/**
 * tunaFlow 내부 마커 제거. 사용자 가시 산출물 (docs/plans/*.md, docs/insight/*.md)
 * 에 새지 않아야 하는 모든 write 경로에서 이 함수를 통과시킬 것.
 *
 * 대상 마커:
 *   <!-- tunaflow:TOKEN -->            (plan-proposal / insight-findings / etc.)
 *   <!-- tunaflow:TOKEN:NUM -->        (subtask-ref 등 payload 번호가 붙는 경우)
 *   <!-- subtask-done:N -->
 *   <!-- impl-complete -->
 *
 * 추가로 연속 공백 라인(3+ \n)을 2개로 정규화하고 앞뒤 공백 trim.
 *
 * 이 파일은 DOM/React 의존 없음 — 워커/CLI/서비스 워커에서도 재사용 가능.
 */
const MARKER_PATTERNS: RegExp[] = [
  /<!--\s*tunaflow:[a-z_-]+(?::\d+)?\s*-->/g,
  /<!--\s*subtask-done:\d+\s*-->/g,
  /<!--\s*impl-complete\s*-->/g,
];

export function stripTunaflowMarkers(text: string): string {
  if (!text) return text;
  let out = text;
  for (const re of MARKER_PATTERNS) {
    out = out.replace(re, "");
  }
  return out.replace(/\n{3,}/g, "\n\n").trim();
}
