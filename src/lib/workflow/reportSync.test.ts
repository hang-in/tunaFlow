import { describe, it, expect } from "vitest";
import { truncateSafe, isResultMdEcho } from "./reportSync";

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

describe("isResultMdEcho", () => {
  it("returns true when both sentinel headers appear in head", () => {
    const content = "# Implementation Result: 2026-04-29 fix\n\n> Plan Revision: r2\n\nbody...";
    expect(isResultMdEcho(content)).toBe(true);
  });

  it("returns false when only Implementation Result header present", () => {
    const content = "# Implementation Result: only this header\n\nsome body without revision";
    expect(isResultMdEcho(content)).toBe(false);
  });

  it("returns false when only Plan Revision header present", () => {
    const content = "> Plan Revision: r1\n\nstandalone revision note";
    expect(isResultMdEcho(content)).toBe(false);
  });

  it("returns false for normal assistant message mentioning result.md plainly", () => {
    const content = "I edited the result.md file based on your request, see diff below.";
    expect(isResultMdEcho(content)).toBe(false);
  });

  it("returns false for code block that contains both phrases as quoted strings", () => {
    // 정상 코드 설명에 result.md 언급 + 별개 단어로 'Plan Revision' 이 있어도
    // 헤더 형식(`# ...:` / `> ...:`) 이 아니면 echo 아님
    const content = "Here we describe Implementation Result: process and Plan Revision: workflow as concepts.";
    expect(isResultMdEcho(content)).toBe(false);
  });

  it("requires headers within first 200 chars", () => {
    const padding = "x".repeat(250);
    const content = `${padding}\n# Implementation Result: late\n> Plan Revision: late`;
    expect(isResultMdEcho(content)).toBe(false);
  });

  it("returns false for empty / non-string input", () => {
    expect(isResultMdEcho("")).toBe(false);
    // @ts-expect-error testing runtime guard
    expect(isResultMdEcho(undefined)).toBe(false);
  });

  it("matches both headers even with extra whitespace", () => {
    const content = "#  Implementation Result:  spaced\n>  Plan Revision:  r3";
    expect(isResultMdEcho(content)).toBe(true);
  });
});
