/**
 * P0: 스트리밍/이벤트 흐름 테스트
 *
 * runtimeSlice.sendWithEngine의 전체 이벤트 흐름을 검증한다:
 * - progress → chunk → completed 순서에서 placeholder 정상 교체
 * - agent:error 시 cleanup + 상태 복구
 * - conversationId 불일치 이벤트 무시
 * - queue drain이 thread별로 정확히 동작
 * - pendingChunk null 처리 (flushChunk race condition 방어)
 * - roundtable 이벤트 흐름
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

// ─── Mock dependencies ─────────────────────────────────────────────────────

vi.mock("@/lib/appStore", () => ({
  getSetting: vi.fn(() => Promise.resolve({ mode: "auto", totalCap: 60000 })),
  setSetting: vi.fn(() => Promise.resolve()),
}));

vi.mock("@/stores/toolStepsStore", () => ({
  useToolStepsStore: {
    getState: () => ({
      handleProgress: vi.fn(),
      getSteps: vi.fn(() => []),
      clear: vi.fn(),
    }),
  },
}));

vi.mock("@/lib/toolSteps", () => ({
  serializeSteps: vi.fn(() => "[]"),
}));

vi.mock("@/lib/planProposalParser", () => ({
  extractToolRequests: vi.fn(() => []),
}));

vi.mock("@tauri-apps/plugin-notification", () => ({
  sendNotification: vi.fn(),
  isPermissionGranted: vi.fn(() => Promise.resolve(false)),
}));

// ─── Event capture infrastructure ──────────────────────────────────────────

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type EventHandler = (event: any) => void;
let capturedListeners: Map<string, EventHandler[]>;

function setupEventCapture() {
  capturedListeners = new Map();
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  vi.mocked(listen).mockImplementation(async (eventName: any, handler: any) => {
    if (!capturedListeners.has(eventName)) {
      capturedListeners.set(eventName, []);
    }
    capturedListeners.get(eventName)!.push(handler);
    return () => {
      const handlers = capturedListeners.get(eventName);
      if (handlers) {
        const idx = handlers.indexOf(handler);
        if (idx >= 0) handlers.splice(idx, 1);
      }
    };
  });
}

function emitEvent(eventName: string, payload: unknown) {
  const handlers = capturedListeners.get(eventName) ?? [];
  for (const handler of [...handlers]) {
    handler({ payload });
  }
}

// ─── Store factory ─────────────────────────────────────────────────────────

import { createRuntimeSlice, ENGINE_CONFIGS } from "@/stores/slices/runtimeSlice";
import type { ChatState, SetState, GetState } from "@/stores/slices/types";

function createMockStore(overrides: Partial<ChatState> = {}) {
  let state: ChatState = {
    // Minimal required state
    projects: [],
    selectedProjectKey: "proj-1",
    conversations: [],
    selectedConversationId: "conv-1",
    messages: [],
    branches: [],
    runningThreadIds: [],
    messageQueue: [],
    error: null,
    activeBranchId: null,
    parentConversationId: null,
    threadBranchId: null,
    threadBranchConvId: null,
    threadMessages: [],
    threadBranchLabel: null,
    threadParentMessage: null,
    rtParticipantStatuses: new Map(),
    rtStatusConversationId: null,
    memos: [],
    artifacts: [],
    skills: [],
    activeSkills: ["skill-a"],
    workflowSkills: {},
    crossSessionIds: [],
    rawqStatus: null,
    projectLoading: null,
    engineModels: [],
    recommendedSkills: null,
    _staleConversations: new Set(),
    handoffSource: null,
    scrollToMessageId: null,
    personaFragment: null,
    personaLabel: null,
    agentProfiles: [],
    _convEngineMap: {},
    // Stubs for methods
    getEffectiveSkills: () => ["skill-a"],
    getConversationEngine: () => null,
    ...overrides,
  } as unknown as ChatState;

  const set: SetState = (partial) => {
    if (typeof partial === "function") {
      const result = partial(state);
      if (result) state = { ...state, ...result };
    } else {
      state = { ...state, ...partial };
    }
  };

  const get: GetState = () => state;

  // Create runtime slice and merge into state
  const slice = createRuntimeSlice(set, get);
  state = { ...state, ...slice };

  return { set, get, getState: () => state };
}

// ─── Tests ─────────────────────────────────────────────────────────────────

describe("Streaming flow — sendWithEngine", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
    setupEventCapture();
    // Default: invoke resolves (command succeeds)
    vi.mocked(invoke).mockResolvedValue({ messageId: "msg-real" });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  // ─── Placeholder creation & swap ──────────────────────────────────────

  it("creates user + placeholder messages before invoke", async () => {
    const { get } = createMockStore();
    const sendPromise = get().sendWithEngine("claude", "hello");

    // Let microtasks settle (listen registration)
    await vi.advanceTimersByTimeAsync(0);

    const msgs = get().messages;
    expect(msgs.length).toBe(2);
    expect(msgs[0].role).toBe("user");
    expect(msgs[0].content).toBe("hello");
    expect(msgs[0].id).toMatch(/^temp-user-/);
    expect(msgs[1].role).toBe("assistant");
    expect(msgs[1].status).toBe("streaming");
    expect(msgs[1].id).toMatch(/^temp-thinking-/);
    expect(msgs[1].engine).toBe("claude-code");

    // Cleanup: emit completed to avoid dangling listeners
    vi.mocked(invoke).mockResolvedValue([]);
    emitEvent("agent:completed", { messageId: "msg-real", conversationId: "conv-1" });
    await vi.advanceTimersByTimeAsync(100);
  });

  it("swaps placeholder with real messageId on progress event", async () => {
    const { get } = createMockStore();
    const sendPromise = get().sendWithEngine("claude", "test");
    await vi.advanceTimersByTimeAsync(0);

    emitEvent("claude:progress", { messageId: "msg-42", conversationId: "conv-1", text: "" });

    const msgs = get().messages;
    const real = msgs.find((m) => m.id === "msg-42");
    expect(real).toBeDefined();
    expect(real!.status).toBe("streaming");
    // Placeholder should be gone
    expect(msgs.filter((m) => m.id.startsWith("temp-thinking-")).length).toBe(0);

    vi.mocked(invoke).mockResolvedValue([]);
    emitEvent("agent:completed", { messageId: "msg-42", conversationId: "conv-1" });
    await vi.advanceTimersByTimeAsync(100);
  });

  // ─── Chunk streaming with throttle ────────────────────────────────────

  it("updates content via chunk events (throttled)", async () => {
    const { get } = createMockStore();
    get().sendWithEngine("claude", "stream test");
    await vi.advanceTimersByTimeAsync(0);

    // Swap placeholder first
    emitEvent("claude:progress", { messageId: "msg-s1", conversationId: "conv-1", text: "" });

    // Send chunk — should be pending (throttled)
    emitEvent("claude:chunk", { messageId: "msg-s1", conversationId: "conv-1", text: "Hello" });
    const beforeFlush = get().messages.find((m) => m.id === "msg-s1");
    expect(beforeFlush!.content).toBe(""); // Not flushed yet

    // Advance 200ms to trigger flush
    vi.advanceTimersByTime(200);
    const afterFlush = get().messages.find((m) => m.id === "msg-s1");
    expect(afterFlush!.content).toBe("Hello");

    // Multiple rapid chunks — only last one should survive throttle
    emitEvent("claude:chunk", { messageId: "msg-s1", conversationId: "conv-1", text: "Hello World" });
    emitEvent("claude:chunk", { messageId: "msg-s1", conversationId: "conv-1", text: "Hello World!" });
    vi.advanceTimersByTime(200);
    const final = get().messages.find((m) => m.id === "msg-s1");
    expect(final!.content).toBe("Hello World!");

    vi.mocked(invoke).mockResolvedValue([]);
    emitEvent("agent:completed", { messageId: "msg-s1", conversationId: "conv-1" });
    await vi.advanceTimersByTimeAsync(100);
  });

  // ─── agent:completed flow ─────────────────────────────────────────────

  it("reloads messages from DB on agent:completed", async () => {
    const dbMessages = [
      { id: "u1", conversationId: "conv-1", role: "user", content: "hello", timestamp: 1, status: "done" },
      { id: "a1", conversationId: "conv-1", role: "assistant", content: "world", timestamp: 2, status: "done" },
    ];

    const { get } = createMockStore();
    get().sendWithEngine("claude", "hello");
    await vi.advanceTimersByTimeAsync(0);

    // Mock list_messages for the completed handler
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "list_messages") return dbMessages;
      return {};
    });

    emitEvent("agent:completed", { messageId: "a1", conversationId: "conv-1" });
    await vi.advanceTimersByTimeAsync(100);

    const msgs = get().messages;
    expect(msgs.length).toBe(2);
    expect(msgs[1].content).toBe("world");
    expect(msgs[1].status).toBe("done");

    // runningThreadIds should be cleared
    expect(get().runningThreadIds).not.toContain("conv-1");
  });

  // ─── agent:error flow ─────────────────────────────────────────────────

  it("sets error state and reloads on agent:error", async () => {
    const { get } = createMockStore();
    get().sendWithEngine("claude", "will fail");
    await vi.advanceTimersByTimeAsync(0);

    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "list_messages") return [{ id: "u1", conversationId: "conv-1", role: "user", content: "will fail", timestamp: 1, status: "done" }];
      return {};
    });

    emitEvent("agent:error", { messageId: "err-1", conversationId: "conv-1", error: "CLI crashed" });
    await vi.advanceTimersByTimeAsync(100);

    expect(get().error).toBe("CLI crashed");
    expect(get().messages.length).toBe(1);
    expect(get().runningThreadIds).not.toContain("conv-1");
  });

  // ─── conversationId filtering ─────────────────────────────────────────

  it("ignores events from different conversationId", async () => {
    const { get } = createMockStore();
    get().sendWithEngine("claude", "my conv");
    await vi.advanceTimersByTimeAsync(0);

    const msgsBefore = get().messages.length;

    // Events from a different conversation
    emitEvent("claude:progress", { messageId: "other-msg", conversationId: "conv-OTHER", text: "" });
    emitEvent("claude:chunk", { messageId: "other-msg", conversationId: "conv-OTHER", text: "noise" });
    vi.advanceTimersByTime(200);

    // Messages should not change (except the temp placeholders from sendWithEngine)
    expect(get().messages.length).toBe(msgsBefore);
    expect(get().messages.every((m) => m.conversationId === "conv-1")).toBe(true);

    // This event should also be ignored
    emitEvent("agent:completed", { messageId: "other-msg", conversationId: "conv-OTHER" });
    await vi.advanceTimersByTimeAsync(100);
    // Should still be running because conv-1 didn't complete
    expect(get().runningThreadIds).toContain("conv-1");

    // Actual completion
    vi.mocked(invoke).mockResolvedValue([]);
    emitEvent("agent:completed", { messageId: "real-msg", conversationId: "conv-1" });
    await vi.advanceTimersByTimeAsync(100);
  });

  // ─── Race condition: pendingChunk discarded before cleanup ────────────

  it("discards pendingChunk before cleanup on completed (race condition prevention)", async () => {
    const dbMessages = [
      { id: "u1", conversationId: "conv-1", role: "user", content: "q", timestamp: 1, status: "done" },
      { id: "a1", conversationId: "conv-1", role: "assistant", content: "final answer", timestamp: 2, status: "done" },
    ];

    const { get } = createMockStore();
    get().sendWithEngine("claude", "q");
    await vi.advanceTimersByTimeAsync(0);

    emitEvent("claude:progress", { messageId: "a1", conversationId: "conv-1", text: "" });
    // Queue a chunk but DON'T flush yet (timer pending)
    emitEvent("claude:chunk", { messageId: "a1", conversationId: "conv-1", text: "partial..." });

    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "list_messages") return dbMessages;
      return {};
    });

    // Complete arrives while chunk timer is still pending
    // The pendingChunk should be discarded BEFORE cleanup flushChunk runs
    emitEvent("agent:completed", { messageId: "a1", conversationId: "conv-1" });
    await vi.advanceTimersByTimeAsync(200);

    // Messages should be the fresh DB version, not the stale "partial..."
    const msgs = get().messages;
    const assistant = msgs.find((m) => m.id === "a1");
    expect(assistant!.content).toBe("final answer");
    expect(assistant!.status).toBe("done");
  });

  // ─── Queue drain ─────────────────────────────────────────────────────

  it("queues second send when thread is busy, executes after first completes", async () => {
    const { get } = createMockStore();

    // First send
    get().sendWithEngine("claude", "first");
    await vi.advanceTimersByTimeAsync(0);

    expect(get().runningThreadIds).toContain("conv-1");

    // Second send while first is running → should queue
    get().sendWithEngine("claude", "second");
    expect(get().messageQueue.length).toBe(1);
    expect(get().messageQueue[0].threadId).toBe("conv-1");

    // Complete first send
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "list_messages") return [];
      if (cmd === "get_active_plan_phase") return null;
      return { messageId: "msg-2" };
    });

    emitEvent("agent:completed", { messageId: "msg-1", conversationId: "conv-1" });
    await vi.advanceTimersByTimeAsync(100);

    // Queue should be drained (execute was called)
    expect(get().messageQueue.length).toBe(0);
  });

  // ─── invoke failure (command throw) ───────────────────────────────────

  it("cleans up on invoke failure", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "start_claude_stream") throw new Error("binary not found");
      if (cmd === "get_active_plan_phase") return null;
      return {};
    });

    const { get } = createMockStore();
    await get().sendWithEngine("claude", "will throw");
    await vi.advanceTimersByTimeAsync(100);

    expect(get().error).toBe("binary not found");
    expect(get().runningThreadIds).not.toContain("conv-1");
    // Placeholder should be removed
    expect(get().messages.some((m) => m.id.startsWith("temp-thinking-"))).toBe(false);
  });

  // ─── Engine routing ───────────────────────────────────────────────────

  it.each([
    ["claude", "start_claude_stream", "claude-code"],
    ["codex", "start_codex_run", "codex"],
    ["gemini", "start_gemini_stream", "gemini"],
    ["opencode", "start_opencode_run", "opencode"],
    ["ollama", "start_openai_compat_stream", "ollama"],
  ])("routes %s to command %s with engineKey %s", async (engine, command, engineKey) => {
    vi.mocked(invoke).mockResolvedValue({ messageId: "m1" });

    const { get } = createMockStore();
    get().sendWithEngine(engine, "test");
    await vi.advanceTimersByTimeAsync(0);

    // Check placeholder has correct engineKey
    const placeholder = get().messages.find((m) => m.role === "assistant");
    expect(placeholder!.engine).toBe(engineKey);

    // Check invoke was called with correct command
    expect(vi.mocked(invoke)).toHaveBeenCalledWith(command, expect.any(Object));

    vi.mocked(invoke).mockResolvedValue([]);
    emitEvent("agent:completed", { messageId: "m1", conversationId: "conv-1" });
    await vi.advanceTimersByTimeAsync(100);
  });

  // ─── Stale conversation tracking ─────────────────────────────────────

  it("marks conversation as stale when completed while navigated away", async () => {
    const { get, set } = createMockStore();
    get().sendWithEngine("claude", "hello");
    await vi.advanceTimersByTimeAsync(0);

    // Simulate user navigated to different conversation
    set({ selectedConversationId: "conv-OTHER" });

    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "list_messages") return [{ id: "a1", conversationId: "conv-1", role: "assistant", content: "done", timestamp: 1, status: "done" }];
      return {};
    });

    emitEvent("agent:completed", { messageId: "a1", conversationId: "conv-1" });
    await vi.advanceTimersByTimeAsync(100);

    expect(get()._staleConversations.has("conv-1")).toBe(true);
  });

  // ─── No project / no conversation guard ───────────────────────────────

  it("returns early when no project selected", async () => {
    const { get } = createMockStore({ selectedProjectKey: null });
    await get().sendWithEngine("claude", "test");
    expect(get().messages.length).toBe(0);
    expect(get().runningThreadIds.length).toBe(0);
  });

  it("returns early when no conversation selected", async () => {
    const { get } = createMockStore({ selectedConversationId: null });
    await get().sendWithEngine("claude", "test");
    expect(get().messages.length).toBe(0);
    expect(get().runningThreadIds.length).toBe(0);
  });
});

// ─── Roundtable flow ────────────────────────────────────────────────────────

describe("Streaming flow — sendRoundtable", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
    setupEventCapture();
    vi.mocked(invoke).mockResolvedValue({ messageId: "rt-msg" });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("creates placeholder and adds RT messages via roundtable:progress", async () => {
    const { get } = createMockStore();
    const participants = [
      { name: "Agent A", engine: "claude", blind: false },
      { name: "Agent B", engine: "gemini", blind: false },
    ];

    get().sendRoundtable("discuss X", participants, "sequential");
    await vi.advanceTimersByTimeAsync(0);

    // Should have user message + placeholder
    expect(get().messages.length).toBe(2);
    expect(get().messages[1].engine).toBe("system");

    // RT progress message replaces placeholder
    const rtMsg1 = { id: "rt-a1", conversationId: "conv-1", role: "assistant", content: "Agent A says...", timestamp: Date.now(), status: "done", engine: "claude" };
    emitEvent("roundtable:progress", { payload: rtMsg1, ...rtMsg1 });
    // The listen handler gets event.payload directly
    emitEvent("roundtable:progress", rtMsg1);

    // Second RT message
    const rtMsg2 = { id: "rt-b1", conversationId: "conv-1", role: "assistant", content: "Agent B says...", timestamp: Date.now(), status: "done", engine: "gemini" };
    emitEvent("roundtable:progress", rtMsg2);

    vi.mocked(invoke).mockResolvedValue([rtMsg1, rtMsg2]);
    emitEvent("agent:completed", { conversationId: "conv-1" });
    await vi.advanceTimersByTimeAsync(100);

    expect(get().runningThreadIds).not.toContain("conv-1");
  });

  it("ignores user-role RT progress messages", async () => {
    const { get } = createMockStore();
    get().sendRoundtable("topic", [{ name: "A", engine: "claude", blind: false }], "sequential");
    await vi.advanceTimersByTimeAsync(0);

    const before = get().messages.length;
    emitEvent("roundtable:progress", { id: "u1", conversationId: "conv-1", role: "user", content: "user msg", timestamp: Date.now(), status: "done" });
    expect(get().messages.length).toBe(before); // unchanged

    vi.mocked(invoke).mockResolvedValue([]);
    emitEvent("agent:completed", { conversationId: "conv-1" });
    await vi.advanceTimersByTimeAsync(100);
  });

  it("deduplicates RT progress messages", async () => {
    const { get } = createMockStore();
    get().sendRoundtable("topic", [{ name: "A", engine: "claude", blind: false }], "sequential");
    await vi.advanceTimersByTimeAsync(0);

    const msg = { id: "dup-1", conversationId: "conv-1", role: "assistant", content: "response", timestamp: Date.now(), status: "done" };
    emitEvent("roundtable:progress", msg);
    emitEvent("roundtable:progress", msg); // duplicate

    // Only one copy should exist (excluding user + placeholder)
    const assistantMsgs = get().messages.filter((m) => m.id === "dup-1");
    expect(assistantMsgs.length).toBe(1);

    vi.mocked(invoke).mockResolvedValue([]);
    emitEvent("agent:completed", { conversationId: "conv-1" });
    await vi.advanceTimersByTimeAsync(100);
  });
});

// ─── cancelOperation ────────────────────────────────────────────────────────

describe("Streaming flow — cancelOperation", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
    setupEventCapture();
    vi.mocked(invoke).mockResolvedValue({ messageId: "msg-c" });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("clears runningThreadIds and messageQueue for the target", async () => {
    const { get } = createMockStore();
    get().sendWithEngine("claude", "msg 1");
    await vi.advanceTimersByTimeAsync(0);

    // Queue a second
    get().sendWithEngine("claude", "msg 2");
    expect(get().messageQueue.length).toBe(1);

    vi.mocked(invoke).mockResolvedValue([]);
    await get().cancelOperation("conv-1");
    await vi.advanceTimersByTimeAsync(100);

    expect(get().runningThreadIds).not.toContain("conv-1");
    expect(get().messageQueue.filter((q) => q.threadId === "conv-1").length).toBe(0);
  });
});

// ─── _startRun / _endRun ────────────────────────────────────────────────────

describe("Streaming flow — thread run helpers", () => {
  it("_startRun adds threadId, _endRun removes it", () => {
    const { get } = createMockStore();
    get()._startRun("t1");
    expect(get().runningThreadIds).toContain("t1");

    get()._startRun("t2");
    expect(get().runningThreadIds).toEqual(expect.arrayContaining(["t1", "t2"]));

    // Duplicate startRun should not create duplicates
    get()._startRun("t1");
    expect(get().runningThreadIds.filter((id) => id === "t1").length).toBe(1);

    get()._endRun("t1");
    expect(get().runningThreadIds).not.toContain("t1");
    expect(get().runningThreadIds).toContain("t2");
  });
});
