import { describe, it, expect } from "vitest";
import { truncateSafe } from "./reportSync";

describe("truncateSafe", () => {
  it("returns original string when within limit", () => {
    expect(truncateSafe("hello", 10)).toBe("hello");
  });

  it("returns original empty string unchanged", () => {
    expect(truncateSafe("", 10)).toBe("");
  });

  it("returns original when code-point count equals limit", () => {
    const s = "abcde";
    expect(truncateSafe(s, 5)).toBe(s);
  });

  it("truncates ASCII text and appends marker", () => {
    const s = "abcdefghij"; // 10 chars
    const result = truncateSafe(s, 5);
    expect(result.startsWith("abcde")).toBe(true);
    expect(result).toContain("[…truncated, original 10 chars]");
  });

  it("respects UTF-8 boundary for Korean (Hangul)", () => {
    // 가나다라마 = 5 code points
    const result = truncateSafe("가나다라마", 3);
    expect(result.startsWith("가나다")).toBe(true);
    expect(result).toContain("[…truncated, original 5 chars]");
    // 보장: 잘려도 깨진 자모 없음
    expect(result.includes("�")).toBe(false);
  });

  it("respects UTF-8 boundary for emoji (surrogate pair)", () => {
    // 😀😃😄 = 3 code points, but 6 UTF-16 code units
    const s = "😀😃😄😁😆";
    const result = truncateSafe(s, 2);
    expect(result.startsWith("😀😃")).toBe(true);
    expect(result).toContain("[…truncated, original 5 chars]");
  });

  it("does not truncate when limit equals char length", () => {
    expect(truncateSafe("가나다", 3)).toBe("가나다");
  });

  it("preserves marker format for downstream parser visibility", () => {
    const result = truncateSafe("0123456789", 4);
    // marker 는 newline 두 개로 시작
    expect(result).toMatch(/0123\n\n\[…truncated, original 10 chars\]/);
  });
});
