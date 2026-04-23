/**
 * Rust AppError JSON 에서 `code` / `context` 추출. i18nPlan Phase 4A-1 신규 계약:
 * Rust 는 `{ code, context, message }` 객체로 직렬화 — FE 는 `code` 로 i18n 키
 * 선택, `context` 를 interpolation variable 로 사용.
 *
 * Tauri invoke catch 블록 사용법:
 * ```ts
 * import { useTranslation } from "react-i18next";
 * const { t } = useTranslation("error");
 * try { await invoke(...); }
 * catch (e) {
 *   const { code, context } = extractErrorCode(e);
 *   const key = context ? `${code}_with_context` : code;
 *   toast.error(t(key, { context }));
 * }
 * ```
 *
 * `error.json` 의 키 구조:
 * - `{code}` — context 없을 때 폴백
 * - `{code}_with_context` — context 가 interpolate 될 상세 메시지
 */

export type AppErrorCode =
  | "db_error"
  | "not_found"
  | "io_error"
  | "json_error"
  | "agent_error"
  | "bad_request"
  | "lock_error"
  | "unknown_error";

export interface ExtractedError {
  code: AppErrorCode;
  context: string;
  /** 원본 `message` (영어). 최후 fallback 용. */
  rawMessage: string;
}

const KNOWN_CODES: readonly string[] = [
  "db_error",
  "not_found",
  "io_error",
  "json_error",
  "agent_error",
  "bad_request",
  "lock_error",
];

function isKnownCode(v: unknown): v is AppErrorCode {
  return typeof v === "string" && KNOWN_CODES.includes(v);
}

export function extractErrorCode(e: unknown): ExtractedError {
  // Rust Tauri 에러는 보통 plain object 로 올라옴
  if (e && typeof e === "object") {
    const obj = e as Record<string, unknown>;
    const rawCode = obj.code;
    const rawContext = obj.context;
    const rawMessage = typeof obj.message === "string" ? obj.message : String(e);
    if (isKnownCode(rawCode)) {
      return {
        code: rawCode,
        context: typeof rawContext === "string" ? rawContext : "",
        rawMessage,
      };
    }
    // 구조는 object 지만 code 가 unknown — 전체 message 를 context 로 흡수
    return {
      code: "unknown_error",
      context: rawMessage,
      rawMessage,
    };
  }
  // 문자열/기타 — 전체를 unknown context 로
  const str = typeof e === "string" ? e : String(e);
  return {
    code: "unknown_error",
    context: str,
    rawMessage: str,
  };
}
