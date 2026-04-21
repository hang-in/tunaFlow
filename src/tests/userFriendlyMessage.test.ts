import { describe, it, expect } from "vitest";
import { formatError, formatErrorWithPrefix } from "@/lib/errors/userFriendlyMessage";

describe("formatError — AppError shape", () => {
  it("db_error code → korean sentence", () => {
    const err = { code: "db_error", message: "Database error: UNIQUE constraint failed" };
    expect(formatError(err)).toContain("데이터 저장 중 오류");
  });

  it("not_found pattern wins over generic code", () => {
    const err = { code: "agent_error", message: "plan p-123 not found" };
    expect(formatError(err)).toContain("찾을 수 없습니다");
  });

  it("empty_branch pattern maps regardless of code", () => {
    const err = { code: "agent_error", message: "empty_branch" };
    expect(formatError(err)).toContain("빈 브랜치");
  });

  it("unknown code falls back to the raw message", () => {
    const err = { code: "some_new_code", message: "some new issue" };
    expect(formatError(err)).toBe("some new issue");
  });
});

describe("formatError — plain Error / string / unknown", () => {
  it("Error.message with known pattern → mapped", () => {
    const err = new Error("request timeout after 30s");
    expect(formatError(err)).toContain("응답 대기");
  });

  it("plain string without pattern → returned as-is", () => {
    expect(formatError("boom")).toBe("boom");
  });

  it("rate limit pattern", () => {
    expect(formatError(new Error("rate-limit exceeded"))).toContain("요청 한도");
  });

  it("null / undefined → stringified", () => {
    expect(formatError(null)).toBe("null");
    expect(formatError(undefined)).toBe("undefined");
  });
});

describe("formatErrorWithPrefix", () => {
  it("composes prefix + mapped message", () => {
    const err = { code: "lock_error", message: "lock poisoned" };
    expect(formatErrorWithPrefix("전송 실패", err)).toBe("전송 실패: 잠시 지연이 발생했습니다. 다시 시도해주세요.");
  });
});
