import { describe, it, expect } from "vitest";
import { stripTunaflowMarkers } from "./markerScrub";

describe("stripTunaflowMarkers", () => {
  it("removes an inline tunaflow marker and leaves surrounding text", () => {
    expect(stripTunaflowMarkers("hello <!-- tunaflow:plan-proposal --> world"))
      .toBe("hello  world");
  });

  it("normalizes multiple blank lines around a subtask-done marker", () => {
    expect(stripTunaflowMarkers("done\n\n\n\n<!-- subtask-done:3 -->\ndone"))
      .toBe("done\n\ndone");
  });

  it("passes empty string through unchanged", () => {
    expect(stripTunaflowMarkers("")).toBe("");
  });

  it("leaves marker-free text untouched (aside from trim)", () => {
    expect(stripTunaflowMarkers("plain report body"))
      .toBe("plain report body");
  });

  it("strips impl-complete and tunaflow payload markers together", () => {
    const input = "<!-- impl-complete -->\nresult\n<!-- tunaflow:insight-findings:12 -->";
    expect(stripTunaflowMarkers(input)).toBe("result");
  });

  it("preserves regular HTML comments that are not tunaflow markers", () => {
    // INV-5: regex must only match tunaflow markers, not general HTML comments
    expect(stripTunaflowMarkers("before <!-- user comment --> after"))
      .toBe("before <!-- user comment --> after");
  });
});
