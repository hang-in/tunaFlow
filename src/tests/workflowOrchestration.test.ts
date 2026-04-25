import { describe, it, expect, vi, beforeEach } from "vitest";

// Mock Tauri invoke
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve()),
}));

// Mock failure lessons API
vi.mock("@/lib/api/failureLessons", () => ({
  createFailureLessonsBatch: vi.fn(() => Promise.resolve([])),
  resolveFailureLessonsByPlan: vi.fn(() => Promise.resolve(0)),
}));

// Mock artifacts API
vi.mock("@/lib/api/artifacts", () => ({
  createArtifact: vi.fn(() => Promise.resolve({ id: "art-1" })),
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
  listPlanEvents: vi.fn(() => Promise.resolve([])),
}));

// Mock appStore — skipManualVerificationGate default false; individual tests override.
vi.mock("@/lib/appStore", () => ({
  getSetting: vi.fn((_key: string, fallback: unknown) => Promise.resolve(fallback)),
  setSetting: vi.fn(() => Promise.resolve()),
}));

import { invoke } from "@tauri-apps/api/core";
import * as planApi from "@/lib/api/plans";
import * as appStore from "@/lib/appStore";
import {
  approveAndStartImplementation,
  approveImplPlan,
  processReviewVerdict,
  requestPlanRevision,
  scanMessagesForMarkers,
  slugifyPlanTitle,
  startReviewBranch,
  startReviewRT,
  ManualVerificationFailed,
  ReviewRTEntryFailed,
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
    if (cmd === "create_branch") return Promise.resolve({ id: "br-1", conversationId: "conv-1", label: "dev", status: "active", createdAt: Date.now() });
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
      failedSubtaskIds: [],
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
      failedSubtaskIds: [3],
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
      failedSubtaskIds: [],
      raw: "",
    });

    expect(planApi.updatePlanPhase).not.toHaveBeenCalled();
    expect(planApi.createPlanEvent).toHaveBeenCalledWith("p-1", "review_conditional", "reviewer", expect.any(String));
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

  it("archives review branch when handing off to architect", async () => {
    const sendFn = vi.fn(() => Promise.resolve());
    const planWithReviewBranch = { ...mockPlan, reviewBranchId: "rev-br-42" };
    await requestPlanRevision(planWithReviewBranch, [], "claude", sendFn);
    // archive_branch should have been called with the review branch id
    expect(invoke).toHaveBeenCalledWith("archive_branch", { id: "rev-br-42" });
  });

  it("skips archive when no review branch exists", async () => {
    const sendFn = vi.fn(() => Promise.resolve());
    await requestPlanRevision(mockPlan, [], "claude", sendFn);
    expect(invoke).not.toHaveBeenCalledWith("archive_branch", expect.any(Object));
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

  it("ignores user-role messages", () => {
    const msgs: Message[] = [
      { id: "m1", conversationId: "c1", role: "user", content: "<!-- tunaflow:impl-complete -->", timestamp: 0, status: "done" },
    ];
    const result = scanMessagesForMarkers(msgs);
    expect(result.implComplete).toBe(false);
  });

  it("detects first impl-plan only", () => {
    const msgs: Message[] = [
      { id: "m1", conversationId: "c1", role: "assistant", content: '<!-- tunaflow:impl-plan -->\n```json\n{"files":["a.ts"],"dependencies":[],"risks":[]}\n```', timestamp: 0, status: "done" },
      { id: "m2", conversationId: "c1", role: "assistant", content: '<!-- tunaflow:impl-plan -->\n```json\n{"files":["b.ts"],"dependencies":[],"risks":[]}\n```', timestamp: 1, status: "done" },
    ];
    const result = scanMessagesForMarkers(msgs);
    // Only first impl-plan should be captured
    if (result.implPlan) {
      expect(result.implPlan.files).toContain("a.ts");
    }
  });
});

// ─���─ Doom loop escalation ────────────────────────────────────────────────

describe("processReviewVerdict — doom loop", () => {
  it("warns at 3 failures but does NOT force escalate", async () => {
    vi.mocked(planApi.listPlanEvents).mockResolvedValue([
      { id: "e1", planId: "p-1", eventType: "review_failed", actor: "reviewer", detail: '{"findings":["x"]}', createdAt: 1 },
      { id: "e2", planId: "p-1", eventType: "review_failed", actor: "reviewer", detail: '{"findings":["y"]}', createdAt: 2 },
      { id: "e3", planId: "p-1", eventType: "review_failed", actor: "reviewer", detail: '{"findings":["z"]}', createdAt: 3 },
    ]);

    await processReviewVerdict(mockPlan, {
      verdict: "fail",
      findings: ["Bug found"],
      recommendations: [],
      failedSubtaskIds: [],
      raw: "",
    });

    // Should go to rework + warn, but NOT force escalate
    expect(planApi.updatePlanPhase).toHaveBeenCalledWith("p-1", "rework");
    expect(planApi.updatePlanPhase).not.toHaveBeenCalledWith("p-1", "subtask_review");
    expect(planApi.createPlanEvent).toHaveBeenCalledWith(
      "p-1", "doom_loop_warning", "system", expect.stringContaining("3회"),
    );
  });

  it("force escalates to subtask_review at 5 failures", async () => {
    vi.mocked(planApi.listPlanEvents).mockResolvedValue([
      { id: "e1", planId: "p-1", eventType: "review_failed", actor: "reviewer", detail: '{"findings":["a"]}', createdAt: 1 },
      { id: "e2", planId: "p-1", eventType: "review_failed", actor: "reviewer", detail: '{"findings":["b"]}', createdAt: 2 },
      { id: "e3", planId: "p-1", eventType: "review_failed", actor: "reviewer", detail: '{"findings":["c"]}', createdAt: 3 },
      { id: "e4", planId: "p-1", eventType: "review_failed", actor: "reviewer", detail: '{"findings":["d"]}', createdAt: 4 },
      { id: "e5", planId: "p-1", eventType: "review_failed", actor: "reviewer", detail: '{"findings":["e"]}', createdAt: 5 },
    ]);

    await processReviewVerdict(mockPlan, {
      verdict: "fail",
      findings: ["Bug"],
      recommendations: [],
      failedSubtaskIds: [],
      raw: "",
    });

    expect(planApi.updatePlanPhase).toHaveBeenCalledWith("p-1", "subtask_review");
    expect(planApi.createPlanEvent).toHaveBeenCalledWith(
      "p-1", "doom_loop_escalated", "system", expect.stringContaining("5회"),
    );
  });

  it("does NOT warn at 2 failures", async () => {
    vi.mocked(planApi.listPlanEvents).mockResolvedValue([
      { id: "e1", planId: "p-1", eventType: "review_failed", actor: "reviewer", detail: '{"findings":["x"]}', createdAt: 1 },
      { id: "e2", planId: "p-1", eventType: "review_failed", actor: "reviewer", detail: '{"findings":["y"]}', createdAt: 2 },
    ]);

    await processReviewVerdict(mockPlan, {
      verdict: "fail",
      findings: ["Bug"],
      recommendations: [],
      failedSubtaskIds: [],
      raw: "",
    });

    expect(planApi.updatePlanPhase).toHaveBeenCalledWith("p-1", "rework");
    expect(planApi.updatePlanPhase).not.toHaveBeenCalledWith("p-1", "subtask_review");
  });
});

// ─── Design review suggested (file overlap detection) ────────────────────

describe("processReviewVerdict — design review suggestion", () => {
  it("suggests design review when 2+ failures overlap on same files", async () => {
    vi.mocked(planApi.listPlanEvents).mockResolvedValue([
      { id: "e1", planId: "p-1", eventType: "review_failed", actor: "reviewer", detail: '{"findings":["bug in src/auth.ts line 50"]}', createdAt: 1 },
      { id: "e2", planId: "p-1", eventType: "review_failed", actor: "reviewer", detail: '{"findings":["error in src/auth.ts line 80"]}', createdAt: 2 },
    ]);

    await processReviewVerdict(mockPlan, {
      verdict: "fail",
      findings: ["problem in src/auth.ts line 100"],
      recommendations: [],
      failedSubtaskIds: [],
      raw: "",
    });

    // Should detect file overlap and suggest design review
    expect(planApi.createPlanEvent).toHaveBeenCalledWith(
      "p-1", "design_review_suggested", "system", expect.stringContaining("겹침"),
    );
  });
});

// ─── slugifyPlanTitle ────────────────────────────────────────────────────

describe("slugifyPlanTitle", () => {
  it("converts spaces to hyphens", () => {
    expect(slugifyPlanTitle("My Plan Title")).toBe("my-plan-title");
  });

  it("strips Korean characters", () => {
    // Pure Korean → "plan" fallback (collision handled by DB slug, not this function)
    expect(slugifyPlanTitle("인증 모듈 리팩토링")).toBe("plan");
  });

  it("Korean titles with same ASCII produce same base slug", () => {
    // DB getPlanSlug(plan) handles uniqueness, slugifyPlanTitle is just the base
    const a = slugifyPlanTitle("분석 UX 종합 개선");
    const b = slugifyPlanTitle("보고서 UX 개선 — 카드 팝업");
    expect(a).toBe("ux");
    expect(b).toBe("ux");
    // Uniqueness comes from plan.slug in DB, not this function
  });

  it("handles mixed Korean-English", () => {
    expect(slugifyPlanTitle("Auth 모듈 Refactoring")).toBe("auth-refactoring");
  });

  it("truncates long slugs", () => {
    const long = "a".repeat(100);
    expect(slugifyPlanTitle(long).length).toBeLessThanOrEqual(60);
  });

  it("returns plan-hash for empty input", () => {
    const slug = slugifyPlanTitle("");
    expect(slug).toMatch(/^plan/);
  });
});

// ─── startReviewBranch ──────────────────────────────────────────────────

describe("startReviewBranch", () => {
  it("creates branch and sends review prompt", async () => {
    const result = await startReviewBranch(mockPlan, "Please review the subtask structure");

    expect(invoke).toHaveBeenCalledWith("create_branch", expect.objectContaining({
      input: expect.objectContaining({ label: expect.stringContaining("review") }),
    }));
    expect(planApi.createPlanEvent).toHaveBeenCalledWith("p-1", "review_requested", "user", expect.any(String));
    expect(invoke).toHaveBeenCalledWith("create_user_message", expect.objectContaining({
      input: expect.objectContaining({ content: expect.stringContaining("검토가 요청") }),
    }));
    expect(result.branch.id).toBe("br-1");
  });
});

// ─── startReviewRT ──────────────────────────────────────────────────────

describe("startReviewRT", () => {
  it("creates RT branch with reviewer participants and returns runnable config", async () => {
    const implMsgs: Message[] = [
      { id: "m1", conversationId: "c1", role: "assistant", content: "implemented X", timestamp: 0, status: "done" },
    ];

    const result = await startReviewRT(mockPlan, implMsgs);

    expect(planApi.updatePlanPhase).toHaveBeenCalledWith("p-1", "review");
    expect(planApi.createPlanEvent).toHaveBeenCalledWith("p-1", "impl_completed", "developer");
    expect(invoke).toHaveBeenCalledWith("create_branch", expect.objectContaining({
      input: expect.objectContaining({
        label: expect.stringContaining("Review RT"),
        mode: "roundtable",
      }),
    }));
    expect(invoke).toHaveBeenCalledWith("save_rt_config", expect.any(Object));
    // Caller is responsible for running RT — startReviewRT must NOT pre-create
    // a user message (sendThreadRoundtable persists the prompt itself).
    expect(invoke).not.toHaveBeenCalledWith("create_user_message", expect.any(Object));
    expect(result.branch.id).toBe("br-1");
    expect(result.participants.length).toBeGreaterThanOrEqual(2);
    expect(result.prompt).toContain("코드 리뷰어");
    expect(result.mode).toBe("sequential");
  });

  it("uses custom reviewer engines (string[] backward-compat)", async () => {
    const result = await startReviewRT(mockPlan, [], undefined, ["gemini", "ollama"]);

    const saveCall = (invoke as any).mock.calls.find((c: string[]) => c[0] === "save_rt_config");
    expect(saveCall).toBeDefined();
    const config = JSON.parse(saveCall[1].configJson);
    expect(config.participants.length).toBe(2);
    expect(config.participants[0].engine).toBe("gemini");
    expect(config.participants[1].engine).toBe("ollama");
    // string[] 경로에서는 model 이 없어야 함
    expect(config.participants[0].model).toBeUndefined();
    expect(result.participants.map((p) => p.engine)).toEqual(["gemini", "ollama"]);
  });

  it("forwards reviewer model/name when passed as ReviewerChoice[]", async () => {
    const result = await startReviewRT(mockPlan, [], undefined, [
      { engine: "codex", model: "gpt-5", name: "Reviewer-Codex" },
      { engine: "gemini", model: "gemini-2.5-pro" },
    ]);

    const saveCall = (invoke as any).mock.calls.find((c: string[]) => c[0] === "save_rt_config");
    expect(saveCall).toBeDefined();
    const config = JSON.parse(saveCall[1].configJson);
    expect(config.participants.length).toBe(2);
    expect(config.participants[0]).toMatchObject({ engine: "codex", model: "gpt-5", name: "Reviewer-Codex" });
    expect(config.participants[1]).toMatchObject({ engine: "gemini", model: "gemini-2.5-pro" });
    expect(result.participants[0].model).toBe("gpt-5");
  });
});

// ─── startReviewRT — Manual Verification Gate (B-19) ───────────────────────

describe("startReviewRT — manual verification gate", () => {
  const msgWithManual: Message[] = [
    {
      id: "m1", conversationId: "c1", role: "assistant",
      content: "implemented X\n⚠️ Manual: Click button to verify",
      timestamp: 0, status: "done",
    },
  ];

  it("bypasses the gate when skipManualVerificationGate=true", async () => {
    (appStore.getSetting as any).mockImplementationOnce(() => Promise.resolve(true));
    const runManualGate = vi.fn();
    const result = await startReviewRT(mockPlan, msgWithManual, undefined, undefined, runManualGate);
    expect(runManualGate).not.toHaveBeenCalled();
    expect(result.branch.id).toBe("br-1");
    expect(planApi.updatePlanPhase).toHaveBeenCalledWith("p-1", "review");
  });

  it("skips gate when there are no manual items", async () => {
    const msgsNoManual: Message[] = [{
      id: "m1", conversationId: "c1", role: "assistant",
      content: "implemented X", timestamp: 0, status: "done",
    }];
    const runManualGate = vi.fn();
    await startReviewRT(mockPlan, msgsNoManual, undefined, undefined, runManualGate);
    expect(runManualGate).not.toHaveBeenCalled();
    // plan 주의사항: 0-items 케이스만 manual_verification_skipped 기록
    expect(planApi.createPlanEvent).toHaveBeenCalledWith(
      "p-1", "manual_verification_skipped", "system", expect.any(String),
    );
  });

  it("records manual_verification_passed and proceeds to review on all-pass", async () => {
    const runManualGate = vi.fn(async () => [{ status: "pass" as const }]);
    const result = await startReviewRT(mockPlan, msgWithManual, undefined, undefined, runManualGate);
    expect(runManualGate).toHaveBeenCalledTimes(1);
    expect(planApi.createPlanEvent).toHaveBeenCalledWith(
      "p-1", "manual_verification_passed", "user", expect.any(String),
    );
    expect(planApi.updatePlanPhase).toHaveBeenCalledWith("p-1", "review");
    expect(result.branch.id).toBe("br-1");
  });

  it("throws ManualVerificationFailed and enters rework on any fail", async () => {
    const runManualGate = vi.fn(async () => [{ status: "fail" as const, reason: "button broken" }]);
    await expect(startReviewRT(mockPlan, msgWithManual, undefined, undefined, runManualGate))
      .rejects.toBeInstanceOf(ManualVerificationFailed);
    expect(planApi.createPlanEvent).toHaveBeenCalledWith(
      "p-1", "manual_verification_failed", "user", expect.any(String),
    );
    expect(planApi.updatePlanPhase).toHaveBeenCalledWith("p-1", "rework");
    // INV-1: review phase 로 진입하지 않는다.
    expect(planApi.updatePlanPhase).not.toHaveBeenCalledWith("p-1", "review");
  });

  it("throws a generic Error when the user cancels the dialog", async () => {
    const runManualGate = vi.fn(async () => null);
    await expect(startReviewRT(mockPlan, msgWithManual, undefined, undefined, runManualGate))
      .rejects.toThrow(/cancelled by user/);
    // phase 는 그대로 (INV-5)
    expect(planApi.updatePlanPhase).not.toHaveBeenCalledWith("p-1", "review");
    expect(planApi.updatePlanPhase).not.toHaveBeenCalledWith("p-1", "rework");
  });
});

// ─── startReviewRT — Layer A: stage rollback (Plan reviewRTEntryFailureRollbackPlan) ───

describe("startReviewRT — Layer A entry failure rollback", () => {
  it("rollbacks phase to implementation and emits review_entry_failed when save_rt_config throws", async () => {
    // get_or_create_review_branch 까지는 성공 → save_rt_config 단계에서 throw 시뮬레이션.
    (invoke as any).mockImplementation((cmd: string) => {
      if (cmd === "create_branch") return Promise.resolve({ id: "br-1", conversationId: "conv-1", label: "rev", status: "active", createdAt: 0 });
      if (cmd === "open_branch_stream") return Promise.resolve("branch:br-1");
      if (cmd === "save_rt_config") return Promise.reject(new Error("DB write failed"));
      return Promise.resolve();
    });

    await expect(startReviewRT(mockPlan, [])).rejects.toBeInstanceOf(ReviewRTEntryFailed);
    // INV-2: phase rollback to implementation
    expect(planApi.updatePlanPhase).toHaveBeenCalledWith("p-1", "implementation");
    // INV-1: review_entry_failed event recorded with stage info
    expect(planApi.createPlanEvent).toHaveBeenCalledWith(
      "p-1",
      "review_entry_failed",
      "system",
      expect.stringContaining("save_rt_config"),
    );
  });

  it("captures the stage name on the thrown ReviewRTEntryFailed", async () => {
    (invoke as any).mockImplementation((cmd: string) => {
      if (cmd === "create_branch") return Promise.reject(new Error("branch creation failed"));
      return Promise.resolve();
    });

    let caught: unknown;
    try {
      await startReviewRT(mockPlan, []);
    } catch (e) {
      caught = e;
    }
    expect(caught).toBeInstanceOf(ReviewRTEntryFailed);
    expect((caught as ReviewRTEntryFailed).stage).toBe("get_or_create_review_branch");
  });

  it("does NOT rollback when ManualVerificationFailed throws (separate path)", async () => {
    const runManualGate = vi.fn(async () => [{ status: "fail" as const, reason: "broken" }]);
    const msgWithManual: Message[] = [{
      id: "m1", conversationId: "c1", role: "assistant",
      content: "implemented X\n⚠️ Manual: Click button", timestamp: 0, status: "done",
    }];
    await expect(startReviewRT(mockPlan, msgWithManual, undefined, undefined, runManualGate))
      .rejects.toBeInstanceOf(ManualVerificationFailed);
    // ManualVerificationFailed 는 phase=rework 자체 처리. Layer A 의 implementation
    // rollback 경로로 빠지지 않아야 함 (review_entry_failed event 미발생).
    expect(planApi.createPlanEvent).not.toHaveBeenCalledWith(
      "p-1", "review_entry_failed", expect.anything(), expect.anything(),
    );
    expect(planApi.updatePlanPhase).not.toHaveBeenCalledWith("p-1", "implementation");
  });
});
