/**
 * User-friendly error message mapping layer (Phase 3 Finding 3-1).
 *
 * Tauri commands return `AppError` serialized as `{ code, message }`
 * where `code` is one of seven variants (`db_error`, `not_found`,
 * `io_error`, `json_error`, `agent_error`, `bad_request`, `lock_error`)
 * and `message` is an English developer-oriented sentence. Surfacing
 * that raw `message` in a toast has two problems:
 *
 *   1. Users can't act on "Database error: UNIQUE constraint failed:
 *      plans.slug". They need a sentence in Korean they can understand.
 *   2. SQL identifiers, stack traces, and internal paths leak into
 *      the UI.
 *
 * This module mediates the two. `formatError(err)` returns a Korean
 * sentence for the 20+ variants that actually surface during normal
 * operation. Unknown shapes fall through to the previous behavior
 * (stringified message), so the failure mode when we miss a case is
 * "same as today" — not worse.
 */
import { errorMessage } from "@/lib/utils";

/** Shape of `AppError` after the serde `Serialize` impl in `src-tauri/src/errors.rs`. */
interface AppErrorShape {
  code: string;
  message: string;
}

function isAppErrorShape(e: unknown): e is AppErrorShape {
  return (
    typeof e === "object" &&
    e !== null &&
    "code" in e &&
    "message" in e &&
    typeof (e as { code: unknown }).code === "string" &&
    typeof (e as { message: unknown }).message === "string"
  );
}

/** Known `agent_error` / `bad_request` messages — Rust embeds the string
 *  literal in the variant, so string matching against the tail of the
 *  message is the only way to discriminate. */
const MESSAGE_PATTERNS: Array<{ match: RegExp; text: string }> = [
  { match: /empty_branch/i, text: "빈 브랜치라 적용할 내용이 없습니다." },
  { match: /not found/i, text: "요청한 항목을 찾을 수 없습니다." },
  { match: /rate.?limit/i, text: "요청 한도에 도달했습니다. 잠시 후 다시 시도해주세요." },
  { match: /timeout/i, text: "응답 대기 시간이 초과됐습니다. 네트워크 상태를 확인해주세요." },
  { match: /unauthorized|invalid token/i, text: "인증에 실패했습니다. 세션을 다시 설정해주세요." },
  { match: /bind failed/i, text: "서버 포트를 확보하지 못했습니다. 이미 실행 중인 tunaFlow 가 있는지 확인해주세요." },
  { match: /persist.*failed/i, text: "저장 중 오류가 발생했습니다. 잠시 후 다시 시도해주세요." },
];

/** Mapping of `AppError.code` → Korean fallback sentence. Applied after
 *  `MESSAGE_PATTERNS` — patterns win when both could match. */
const CODE_MESSAGES: Record<string, string> = {
  db_error: "데이터 저장 중 오류가 발생했습니다. 잠시 후 다시 시도해주세요.",
  not_found: "요청한 항목을 찾을 수 없습니다.",
  io_error: "파일을 읽거나 쓰는 중 오류가 발생했습니다.",
  json_error: "데이터 형식이 올바르지 않습니다.",
  agent_error: "에이전트 실행 중 문제가 발생했습니다. 에이전트 상태를 확인해주세요.",
  bad_request: "요청이 유효하지 않습니다.",
  lock_error: "잠시 지연이 발생했습니다. 다시 시도해주세요.",
};

/**
 * Convert an unknown error into a Korean, user-facing sentence.
 *
 * Resolution order:
 *   1. `AppError` shape (`{ code, message }`) → run `MESSAGE_PATTERNS`
 *      against `.message`, fall back to `CODE_MESSAGES[code]`, fall
 *      back to `errorMessage(err)`.
 *   2. Raw `Error` / primitive → run `MESSAGE_PATTERNS` against its
 *      string form, fall back to `errorMessage(err)`.
 *
 * Returns a single sentence — call sites usually prepend context like
 * "분석 실패:" themselves, so avoid stacking on our end.
 */
export function formatError(err: unknown): string {
  if (isAppErrorShape(err)) {
    const pattern = MESSAGE_PATTERNS.find((p) => p.match.test(err.message));
    if (pattern) return pattern.text;
    const byCode = CODE_MESSAGES[err.code];
    if (byCode) return byCode;
    return errorMessage(err);
  }
  const raw = errorMessage(err);
  const pattern = MESSAGE_PATTERNS.find((p) => p.match.test(raw));
  if (pattern) return pattern.text;
  return raw;
}

/**
 * Helper for the very common `toast.error(\`…: ${err}\`)` pattern —
 * composes the prefix with the mapped sentence so callers don't have
 * to remember to call `formatError` on every site.
 */
export function formatErrorWithPrefix(prefix: string, err: unknown): string {
  return `${prefix}: ${formatError(err)}`;
}
