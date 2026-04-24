import { describe, it, expect } from "vitest";
import { extractManualItems } from "./manualVerification";
import type { Message } from "@/types";

function msg(role: "user" | "assistant", content: string): Message {
  return {
    id: `test-${Math.random()}`,
    conversationId: "test",
    role,
    content,
    timestamp: Date.now(),
    status: "done",
  } as Message;
}

describe("extractManualItems", () => {
  it("returns empty array when no manual lines present", () => {
    const messages = [msg("assistant", "Regular response without manual flags.")];
    expect(extractManualItems(messages)).toEqual([]);
  });

  it("extracts a single manual item", () => {
    const messages = [msg("assistant", "Done.\n⚠️ Manual: Click the dropdown to verify it opens")];
    const items = extractManualItems(messages);
    expect(items).toHaveLength(1);
    expect(items[0].label).toBe("Click the dropdown to verify it opens");
    expect(items[0].source).toBe("developer");
  });

  it("extracts multiple manual items across lines", () => {
    const content = [
      "Implementation complete.",
      "⚠️ Manual: Open Settings and toggle Skip gate",
      "⚠️ Manual: Click File > New Project",
      "⚠️ Manual: Verify the toast disappears after 3 seconds",
    ].join("\n");
    const items = extractManualItems([msg("assistant", content)]);
    expect(items).toHaveLength(3);
    expect(items.map((i) => i.label)).toEqual([
      "Open Settings and toggle Skip gate",
      "Click File > New Project",
      "Verify the toast disappears after 3 seconds",
    ]);
  });

  it("scopes extraction to messages after the last Rework marker", () => {
    const messages = [
      msg("assistant", "⚠️ Manual: Old item from previous cycle"),
      msg("user", "### 🔄 Rework\n수정 필요"),
      msg("assistant", "Fixed.\n⚠️ Manual: New item only"),
    ];
    const items = extractManualItems(messages);
    expect(items).toHaveLength(1);
    expect(items[0].label).toBe("New item only");
  });

  it("preserves non-ASCII Unicode labels intact", () => {
    const content = "⚠️ Manual: 한글로 된 확인 항목 — 이모지 ✅ 와 기호 포함";
    const items = extractManualItems([msg("assistant", content)]);
    expect(items).toHaveLength(1);
    expect(items[0].label).toBe("한글로 된 확인 항목 — 이모지 ✅ 와 기호 포함");
  });

  it("ignores blank labels after prefix (e.g. `⚠️ Manual:   `)", () => {
    const content = "⚠️ Manual:    \n⚠️ Manual: valid item";
    const items = extractManualItems([msg("assistant", content)]);
    // Blank-label line doesn't match the `.+` quantifier so only the valid item survives.
    expect(items).toHaveLength(1);
    expect(items[0].label).toBe("valid item");
  });
});
