/**
 * architectDispatch — pass / fail / doom-escalate 분기에서 main conv 로 prompt
 * 자동 dispatch 가 동작하는지 검증.
 *
 * Plan: docs/plans/reviewerVerdictDirectArchitectPlan_2026-05-04.md (T07)
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import type { Plan } from "@/types";
import type { ParsedReviewVerdict } from "../planProposalParser";

// useChatStore mock — sendWithEngine / getConversationEngine 호출만 검증
const sendWithEngine = vi.fn<(engine: string, prompt: string) => Promise<void>>(
  () => Promise.resolve(),
);
const getConversationEngine = vi.fn<(convId: string) => { engine: string; model?: string } | undefined>(
  () => undefined,
);
vi.mock("@/stores/chatStore", () => ({
  useChatStore: { getState: () => ({ sendWithEngine, getConversationEngine }) },
}));

// i18next mock — t() 가 key + interpolated args 로 prompt 를 만들어 sendWithEngine
// 인자에 key 가 포함되어 있는지 단언할 수 있게 함
vi.mock("i18next", () => ({
  default: {
    t: (key: string, opts?: Record<string, unknown>) => {
      if (!opts) return `[${key}]`;
      const ns = opts.ns ?? "";
      const rest = { ...opts };
      delete rest.ns;
      const args = Object.entries(rest)
        .map(([k, v]) => `${k}=${v}`)
        .join(",");
      return `[${key}|${ns}|${args}]`;
    },
  },
}));

import { dispatchArchitectNextPriority, dispatchArchitectRedesign } from "./architectDispatch";

const mockPlan: Plan = {
  id: "p-1",
  conversationId: "conv-1",
  title: "Auth refactor",
  description: "",
  status: "active",
  phase: "review",
  revision: 2,
  versionMajor: 1,
  versionMinor: 0,
  createdAt: 0,
  updatedAt: 0,
};

const mockVerdict: ParsedReviewVerdict = {
  verdict: "fail",
  findings: ["bug A", "bug B"],
  recommendations: ["fix 1"],
  failedSubtaskIds: [1],
  raw: "",
};

beforeEach(() => {
  vi.clearAllMocks();
});

describe("dispatchArchitectNextPriority", () => {
  it("calls sendWithEngine with next_priority_prompt + plan title", async () => {
    await dispatchArchitectNextPriority(mockPlan);
    expect(sendWithEngine).toHaveBeenCalledTimes(1);
    const [engine, prompt] = sendWithEngine.mock.calls[0];
    expect(engine).toBe("claude"); // default fallback
    expect(prompt as string).toContain("review.verdict.next_priority_prompt");
    expect(prompt as string).toContain("title=Auth refactor");
  });

  it("uses saved engine when getConversationEngine returns one", async () => {
    getConversationEngine.mockReturnValueOnce({ engine: "codex", model: "gpt-5" } as any);
    await dispatchArchitectNextPriority(mockPlan);
    const [engine] = sendWithEngine.mock.calls[0];
    expect(engine).toBe("codex");
  });

  it("does not throw when sendWithEngine rejects", async () => {
    sendWithEngine.mockRejectedValueOnce(new Error("network"));
    await expect(dispatchArchitectNextPriority(mockPlan)).resolves.toBeUndefined();
  });
});

describe("dispatchArchitectRedesign", () => {
  it("user-redesign: calls sendWithEngine with redesign_prompt and findings", async () => {
    await dispatchArchitectRedesign(mockPlan, mockVerdict, { reason: "user-redesign" });
    expect(sendWithEngine).toHaveBeenCalledTimes(1);
    const [, prompt] = sendWithEngine.mock.calls[0];
    expect(prompt as string).toContain("review.verdict.redesign_prompt");
    expect(prompt as string).toContain("verdict=FAIL");
    expect(prompt as string).toContain("nextRevision=3");
    // user-redesign 은 reasonNote 없음
    expect(prompt as string).not.toContain("redesign_reason_doom_escalate");
  });

  it("doom-escalate with failCount: prefixes redesign_reason_doom_escalate", async () => {
    await dispatchArchitectRedesign(mockPlan, mockVerdict, { reason: "doom-escalate", failCount: 5 });
    const [, prompt] = sendWithEngine.mock.calls[0];
    expect(prompt as string).toContain("review.verdict.redesign_reason_doom_escalate");
    expect(prompt as string).toContain("failCount=5");
    expect(prompt as string).toContain("review.verdict.redesign_prompt");
  });

  it("uses findings_empty_redesign when verdict.findings is empty", async () => {
    await dispatchArchitectRedesign(
      mockPlan,
      { ...mockVerdict, findings: [] },
      { reason: "user-redesign" },
    );
    const [, prompt] = sendWithEngine.mock.calls[0];
    expect(prompt as string).toContain("review.verdict.findings_empty_redesign");
  });

  it("does not throw when sendWithEngine rejects", async () => {
    sendWithEngine.mockRejectedValueOnce(new Error("network"));
    await expect(
      dispatchArchitectRedesign(mockPlan, mockVerdict, { reason: "user-redesign" }),
    ).resolves.toBeUndefined();
  });
});
