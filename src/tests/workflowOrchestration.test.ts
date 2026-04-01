import { describe, it, expect, vi, beforeEach } from "vitest";

// Mock Tauri invoke
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve()),
}));

// Mock plan API
vi.mock("@/lib/api/plans", () => ({
  updatePlanPhase: vi.fn(() => Promise.resolve()),
  updatePlanStatus: vi.fn(() => Promise.resolve()),
  createPlanEvent: vi.fn(() => Promise.resolve({ id: "ev-1" })),
  assignPlanEngines: vi.fn(() => Promise.resolve()),
  linkPlanBranch: vi.fn(() => Promise.resolve()),
  listSubtasks: vi.fn(() => Promise.resolve([
    { id: "st-1", planId: "p-1", idx: 0, title: "Task 1", details: "Do X", status: "todo", createdAt: 0, updatedAt: 0 },
    { id: "st-2", planId: "p-1", idx: 1, title: "Task 2", details: null, status: "todo", createdAt: 0, updatedAt: 0 },
  ])),
  replacePlanSubtasks: vi.fn(() => Promise.resolve([])),
}));

import { invoke } from "@tauri-apps/api/core";
import * as planApi from "@/lib/api/plans";
import {
  approveAndStartImplementation,
  approveImplPlan,
  processReviewVerdict,
  requestPlanRevision,
  scanMessagesForMarkers,
} from "@/lib/workflowOrchestration";
import type { Plan, Message } from "@/types";

const mockPlan: Plan = {
  id: "p-1",
  conversationId: "conv-1",
  title: "Test Plan",
  description: "A test plan",
  status: "active",
  phase: "approval",
  revision: 0,
  versionMajor: 1,
  versionMinor: 0,
  createdAt: 0,
  updatedAt: 0,
};

beforeEach(() => {
  vi.clearAllMocks();
  (invoke as any).mockImplementation((cmd: string) => {
    if (cmd === "create_branch") return Promise.resolve({ id: "br-1", conversationId: "conv-1", label: "Impl", status: "active", createdAt: Date.now() });
    if (cmd === "open_branch_stream") return Promise.resolve("branch:br-1");
    if (cmd === "save_rt_config") return Promise.resolve();
    if (cmd === "create_user_message") return Promise.resolve({ id: "msg-1" });
    return Promise.resolve();
  });
});

describe("approveAndStartImplementation", () => {
  it("transitions phase and creates branch", async () => {
    const result = await approveAndStartImplementation(mockPlan, "claude");

    expect(planApi.updatePlanPhase).toHaveBeenCalledWith("p-1", "implementation");
    expect(planApi.updatePlanStatus).toHaveBeenCalledWith("p-1", "active");
    expect(planApi.createPlanEvent).toHaveBeenCalledWith("p-1", "approved", "user");
    expect(planApi.assignPlanEngines).toHaveBeenCalledWith("p-1", { developer: "claude" });
    expect(invoke).toHaveBeenCalledWith("create_branch", expect.any(Object));
    expect(result.branch.id).toBe("br-1");
    expect(result.prompt).toContain("순서대로");
  });
});

describe("approveImplPlan", () => {
  it("creates event and returns prompt", async () => {
    const prompt = await approveImplPlan(mockPlan);

    expect(planApi.createPlanEvent).toHaveBeenCalledWith("p-1", "impl_approved", "user");
    expect(prompt).toContain("구현을 시작하세요");
  });
});

describe("processReviewVerdict", () => {
  it("pass → phase done", async () => {
    await processReviewVerdict(mockPlan, {
      verdict: "pass",
      findings: ["All good"],
      recommendations: [],
      raw: "",
    });

    expect(planApi.updatePlanPhase).toHaveBeenCalledWith("p-1", "done");
    expect(planApi.updatePlanStatus).toHaveBeenCalledWith("p-1", "done");
    expect(planApi.createPlanEvent).toHaveBeenCalledWith("p-1", "review_passed", "reviewer", expect.any(String));
  });

  it("fail → phase rework", async () => {
    await processReviewVerdict(mockPlan, {
      verdict: "fail",
      findings: ["Bug found"],
      recommendations: ["Fix it"],
      raw: "",
    });

    expect(planApi.updatePlanPhase).toHaveBeenCalledWith("p-1", "rework");
    expect(planApi.createPlanEvent).toHaveBeenCalledWith("p-1", "review_failed", "reviewer", expect.any(String));
  });

  it("conditional → event only, no phase change", async () => {
    await processReviewVerdict(mockPlan, {
      verdict: "conditional",
      findings: ["Maybe ok"],
      recommendations: [],
      raw: "",
    });

    expect(planApi.updatePlanPhase).not.toHaveBeenCalled();
    expect(planApi.createPlanEvent).toHaveBeenCalledWith("p-1", "review_failed", "reviewer", expect.any(String));
  });
});

describe("requestPlanRevision", () => {
  it("calls sendToArchitect callback with prompt", async () => {
    const sendFn = vi.fn(() => Promise.resolve());
    const msgs: Message[] = [
      { id: "m1", conversationId: "c1", role: "user", content: "Hello", timestamp: 0, status: "done" },
      { id: "m2", conversationId: "c1", role: "assistant", content: "World", timestamp: 0, status: "done" },
    ];

    await requestPlanRevision(mockPlan, msgs, "claude", sendFn);

    expect(sendFn).toHaveBeenCalledTimes(1);
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const [engine, prompt, systemPrompt] = sendFn.mock.calls[0] as any;
    expect(engine).toBe("claude");
    expect(prompt).toContain("계획 수정 요청");
    expect(systemPrompt).toContain("Implementation Branch");
    expect(planApi.createPlanEvent).toHaveBeenCalledWith("p-1", "revision_requested", "user", expect.any(String));
  });

  it("does not import store", async () => {
    // Verify no dynamic import of chatStore
    const sendFn = vi.fn(() => Promise.resolve());
    await requestPlanRevision(mockPlan, [], "claude", sendFn);
    // If it tried to import chatStore, it would fail or call sendWithEngine
    expect(sendFn).toHaveBeenCalledTimes(1);
  });
});

describe("scanMessagesForMarkers", () => {
  it("detects impl-complete", () => {
    const msgs: Message[] = [
      { id: "m1", conversationId: "c1", role: "assistant", content: "done <!-- tunaflow:impl-complete -->", timestamp: 0, status: "done" },
    ];
    const result = scanMessagesForMarkers(msgs);
    expect(result.implComplete).toBe(true);
    expect(result.implPlan).toBeNull();
    expect(result.reviewVerdict).toBeNull();
  });

  it("returns empty for no markers", () => {
    const msgs: Message[] = [
      { id: "m1", conversationId: "c1", role: "assistant", content: "just text", timestamp: 0, status: "done" },
    ];
    const result = scanMessagesForMarkers(msgs);
    expect(result.implComplete).toBe(false);
  });
});
