import { describe, it, expect } from "vitest";
import {
  detectCompletedSubtasks,
  extractLatestReviewVerdict,
  collectAndAggregateVerdicts,
  computeDoomLoopState,
  computeFindingOverlap,
  shouldReuseReviewBranch,
} from "@/lib/workflow/services";
import type { Branch, Message, PlanEvent, PlanSubtask } from "@/types";

const subtask = (idx: number, status: PlanSubtask["status"] = "in_progress"): PlanSubtask => ({
  id: `st-${idx}`,
  planId: "p1",
  idx,
  title: `Subtask ${idx + 1}`,
  status,
  createdAt: 0,
  updatedAt: 0,
});

const msg = (role: Message["role"], content: string, timestamp = 0): Message => ({
  id: `m-${timestamp}-${role}`,
  conversationId: "branch:b1",
  role,
  content,
  timestamp,
  status: "done",
});

/** Streaming 상태의 assistant 메시지 — verdict/마커 포함해도 감지 대상 아님. */
const streamingMsg = (content: string, timestamp = 0): Message => ({
  id: `m-${timestamp}-streaming`,
  conversationId: "branch:b1",
  role: "assistant",
  content,
  timestamp,
  status: "streaming",
});

const planEvent = (eventType: string, createdAt = 0, detail?: string): PlanEvent => ({
  id: `e-${createdAt}-${eventType}`,
  planId: "p1",
  eventType,
  actor: "system",
  detail,
  createdAt,
});

describe("detectCompletedSubtasks", () => {
  it("treats markers and DB done as completed together (1-based merge)", () => {
    const messages = [
      msg("assistant", "done! <!-- tunaflow:subtask-done:1 -->"),
    ];
    const subtasks = [subtask(0), subtask(1, "done")];
    const state = detectCompletedSubtasks(messages, subtasks);
    expect(state.markerNums).toEqual(new Set([1]));
    expect(state.dbDoneNums).toEqual(new Set([2])); // idx 1 → num 2
    expect(state.completedNums).toEqual(new Set([1, 2]));
    expect(state.hasImplCompleteMarker).toBe(false);
    expect(state.allComplete).toBe(false);
  });

  it("impl-complete marker cascades allComplete=true", () => {
    const messages = [msg("assistant", "all good. <!-- tunaflow:impl-complete -->")];
    const subtasks = [subtask(0), subtask(1)];
    const state = detectCompletedSubtasks(messages, subtasks);
    expect(state.hasImplCompleteMarker).toBe(true);
    expect(state.allComplete).toBe(true);
    expect(state.completedNums).toEqual(new Set([1, 2]));
  });

  it("DB all-done with no markers still reports allComplete", () => {
    const subtasks = [subtask(0, "done"), subtask(1, "done")];
    const state = detectCompletedSubtasks([], subtasks);
    expect(state.hasImplCompleteMarker).toBe(false);
    expect(state.allComplete).toBe(true);
  });

  it("empty subtasks cannot be allComplete", () => {
    const state = detectCompletedSubtasks([], []);
    expect(state.allComplete).toBe(false);
  });

  it("user messages carrying markers do not count", () => {
    const messages = [msg("user", "<!-- tunaflow:subtask-done:1 -->")];
    const subtasks = [subtask(0)];
    const state = detectCompletedSubtasks(messages, subtasks);
    // scanCompletedSubtasks / hasImplComplete ignore user messages by contract
    expect(state.markerNums.size).toBe(0);
    expect(state.hasImplCompleteMarker).toBe(false);
  });

  it("streaming assistant with marker is ignored until status=done", () => {
    // Dev 가 응답 streaming 중인 동안 content 에 마커가 이미 포함될 수 있지만,
    // 완료 신호로 취급하면 "Review 시작" 버튼이 조기 활성화되는 버그를 유발한다.
    const messages = [
      streamingMsg("<!-- tunaflow:subtask-done:1 -->"),
      streamingMsg("done <!-- tunaflow:impl-complete -->"),
    ];
    const subtasks = [subtask(0)];
    const state = detectCompletedSubtasks(messages, subtasks);
    expect(state.markerNums.size).toBe(0);
    expect(state.hasImplCompleteMarker).toBe(false);
    expect(state.allComplete).toBe(false);
  });
});

describe("extractLatestReviewVerdict", () => {
  const v1 = "<!-- tunaflow:review-verdict -->\nverdict: fail\nfindings:\n- a\n<!-- /tunaflow:review-verdict -->";
  const v2 = "<!-- tunaflow:review-verdict -->\nverdict: pass\nfindings:\n- b\n<!-- /tunaflow:review-verdict -->";

  it("returns null when no verdict markers exist", () => {
    expect(extractLatestReviewVerdict([msg("assistant", "hi")])).toBeNull();
  });

  it("returns the last verdict when multiple exist", () => {
    const out = extractLatestReviewVerdict([
      msg("assistant", v1, 1),
      msg("assistant", v2, 2),
    ]);
    expect(out?.verdict).toBe("pass");
  });

  it("sinceTs filters earlier verdicts", () => {
    const out = extractLatestReviewVerdict(
      [msg("assistant", v1, 1), msg("assistant", v2, 2)],
      2,
    );
    expect(out?.verdict).toBe("pass");
    const preSince = extractLatestReviewVerdict(
      [msg("assistant", v1, 1), msg("assistant", v2, 2)],
      10,
    );
    expect(preSince).toBeNull();
  });

  it("streaming reviewer with verdict keyword is ignored", () => {
    // Reviewer 가 자유 서술 중 "verdict: pass 가능성" 같은 표현을 쓸 수 있어
    // streaming 상태에서는 verdict 추출 대상에서 제외되어야 한다.
    const freeform = "아직 검토 중이고 verdict: pass 일 가능성이 높지만 확정은 아닙니다.";
    expect(extractLatestReviewVerdict([streamingMsg(freeform, 1)])).toBeNull();
  });

  it("streaming verdict is ignored even when a prior done verdict exists", () => {
    // 같은 라운드에서 이전 reviewer 의 done verdict 뒤에 후속 reviewer 가 streaming 중이라면,
    // 최신 "latest" 는 done 인 앞 verdict 여야 한다 (streaming 중간값으로 덮어쓰지 않음).
    const out = extractLatestReviewVerdict([
      msg("assistant", v1, 1),
      streamingMsg(v2, 2),
    ]);
    expect(out?.verdict).toBe("fail");
  });
});

describe("collectAndAggregateVerdicts", () => {
  it("single verdict returns reviewerCount=1 without votes", () => {
    const single =
      "<!-- tunaflow:review-verdict -->\nverdict: pass\nfindings: []\n<!-- /tunaflow:review-verdict -->";
    const out = collectAndAggregateVerdicts([msg("assistant", single, 1)]);
    expect(out?.reviewerCount).toBe(1);
    expect(out?.votes).toBeUndefined();
    expect(out?.verdict).toBe("pass");
  });

  it("no verdict markers return null", () => {
    expect(collectAndAggregateVerdicts([msg("assistant", "nope")])).toBeNull();
  });

  it("streaming messages are excluded from aggregation", () => {
    // 2 명 streaming + 1 명 done verdict → reviewerCount 1 (fallback 단일 경로)
    const done =
      "<!-- tunaflow:review-verdict -->\nverdict: fail\nfindings: []\n<!-- /tunaflow:review-verdict -->";
    const streamingVerdict =
      "<!-- tunaflow:review-verdict -->\nverdict: pass\nfindings: []\n<!-- /tunaflow:review-verdict -->";
    const out = collectAndAggregateVerdicts([
      streamingMsg(streamingVerdict, 1),
      msg("assistant", done, 2),
      streamingMsg(streamingVerdict, 3),
    ]);
    expect(out?.reviewerCount).toBe(1);
    expect(out?.verdict).toBe("fail");
    expect(out?.votes).toBeUndefined(); // 집계 아닌 단일 fallback
  });
});

describe("computeDoomLoopState", () => {
  it("empty events → ok, failCount 0", () => {
    const s = computeDoomLoopState([]);
    expect(s.failCount).toBe(0);
    expect(s.recommendation).toBe("ok");
    expect(s.escalated).toBe(false);
  });

  it("3 fails without escalation → warn", () => {
    const events = [
      planEvent("review_failed", 1),
      planEvent("review_failed", 2),
      planEvent("review_failed", 3),
    ];
    const s = computeDoomLoopState(events);
    expect(s.failCount).toBe(3);
    expect(s.recommendation).toBe("warn");
    expect(s.escalated).toBe(false);
  });

  it("5 fails → escalate", () => {
    const events = Array.from({ length: 5 }, (_, i) =>
      planEvent("review_failed", i + 1),
    );
    const s = computeDoomLoopState(events);
    expect(s.failCount).toBe(5);
    expect(s.recommendation).toBe("escalate");
  });

  it("escalation resets the window — post-escalation counter starts at 0", () => {
    const events = [
      planEvent("review_failed", 1),
      planEvent("review_failed", 2),
      planEvent("review_failed", 3),
      planEvent("doom_loop_escalated", 4),
      planEvent("review_failed", 5),
    ];
    const s = computeDoomLoopState(events);
    expect(s.failCount).toBe(1);
    expect(s.recommendation).toBe("ok");
    expect(s.escalated).toBe(false); // in THIS window there's no escalation
  });

  it("architect_redesign_requested also resets", () => {
    const events = [
      planEvent("review_failed", 1),
      planEvent("architect_redesign_requested", 2),
      planEvent("review_failed", 3),
    ];
    const s = computeDoomLoopState(events);
    expect(s.failCount).toBe(1);
  });
});

describe("computeFindingOverlap", () => {
  it("no overlap → both ratios 0", () => {
    const o = computeFindingOverlap(
      ["src/a.ts:1 missing guard"],
      ["src/b.ts:2 other problem"],
    );
    expect(o.fileOverlapRatio).toBe(0);
    expect(o.textOverlapRatio).toBe(0);
  });

  it("same file appears in both → fileOverlap > 0", () => {
    const o = computeFindingOverlap(
      ["src/foo.ts:10 regression"],
      ["src/foo.ts:22 still broken"],
    );
    expect(o.fileOverlapRatio).toBeGreaterThan(0);
  });

  it("similar text windows match textOverlap", () => {
    const shared = "authentication middleware leaks session token";
    const o = computeFindingOverlap([shared], [shared + " again"]);
    expect(o.textOverlapRatio).toBeGreaterThan(0);
  });
});

describe("shouldReuseReviewBranch", () => {
  const plan = { reviewBranchId: "rb1" };
  const branch = (overrides: Partial<Branch> = {}): Branch => ({
    id: "rb1",
    conversationId: "c1",
    label: "Review",
    status: "active",
    mode: "roundtable",
    createdAt: 0,
    ...overrides,
  });

  it("reuses when the branch exists, matches mode, and is active", () => {
    const d = shouldReuseReviewBranch(plan, [branch()], "roundtable");
    expect(d).toEqual({ reuse: true, branchId: "rb1" });
  });

  it("rejects when archived", () => {
    const d = shouldReuseReviewBranch(plan, [branch({ status: "archived" })], "roundtable");
    expect(d.reuse).toBe(false);
  });

  it("rejects on mode mismatch", () => {
    const d = shouldReuseReviewBranch(plan, [branch()], "chat");
    expect(d.reuse).toBe(false);
  });

  it("rejects when plan has no reviewBranchId", () => {
    const d = shouldReuseReviewBranch({ reviewBranchId: undefined }, [], "chat");
    expect(d.reuse).toBe(false);
  });

  it("rejects when the branch row is missing from the list", () => {
    const d = shouldReuseReviewBranch(plan, [], "roundtable");
    expect(d.reuse).toBe(false);
  });
});
