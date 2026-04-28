import remarkGfm from "remark-gfm";
import remarkBreaks from "remark-breaks";

/**
 * 채팅/로그 친화 마크다운 플러그인 셋 — SSOT.
 *
 * - `remarkGfm`: GitHub Flavored Markdown (table / strikethrough / task list / autolink).
 *   `singleTilde: false` 로 단일 `~` 를 strikethrough 로 해석하지 않음.
 * - `remarkBreaks`: paragraph 안 single newline 을 `<br>` 로 변환. 채팅/로그 컨텍스트 친화.
 *   code block / list 등 block 구조는 영향 없음 (mdast 단계 paragraph 자식만 처리).
 *
 * 사용처는 모두 본 SSOT 를 통해 import — `<ReactMarkdown remarkPlugins={REMARK_PLUGINS}>`.
 * 직접 인라인 array 정의 금지 (위치별 표시 차이 방지).
 */
export const REMARK_PLUGINS: any[] = [
  [remarkGfm, { singleTilde: false }],
  remarkBreaks,
];
