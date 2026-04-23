import { describe, it, expect } from "vitest";
import { extractErrorCode } from "@/lib/errors/extractErrorCode";

describe("extractErrorCode", () => {
  it("parses AppError JSON object with known code", () => {
    const err = { code: "not_found", context: "plan-42", message: "Not found: plan-42" };
    const out = extractErrorCode(err);
    expect(out.code).toBe("not_found");
    expect(out.context).toBe("plan-42");
    expect(out.rawMessage).toBe("Not found: plan-42");
  });

  it("handles all known error codes", () => {
    const codes = [
      "db_error",
      "not_found",
      "io_error",
      "json_error",
      "agent_error",
      "bad_request",
      "lock_error",
    ];
    for (const code of codes) {
      const out = extractErrorCode({ code, context: "x", message: "m" });
      expect(out.code).toBe(code);
    }
  });

  it("falls back to unknown_error for unrecognized code", () => {
    const err = { code: "weird_code", context: "c", message: "m" };
    const out = extractErrorCode(err);
    expect(out.code).toBe("unknown_error");
    // context 는 message 로 흡수
    expect(out.context).toBe("m");
  });

  it("handles string errors", () => {
    const out = extractErrorCode("plain text error");
    expect(out.code).toBe("unknown_error");
    expect(out.context).toBe("plain text error");
  });

  it("handles Error instances", () => {
    const err = new Error("boom");
    const out = extractErrorCode(err);
    expect(out.code).toBe("unknown_error");
    // Error 는 object — message 필드가 있으므로 rawMessage 는 "boom"
    expect(out.rawMessage).toBe("boom");
  });

  it("handles empty context gracefully", () => {
    const out = extractErrorCode({ code: "lock_error", context: "", message: "Lock poisoned" });
    expect(out.code).toBe("lock_error");
    expect(out.context).toBe("");
  });
});
