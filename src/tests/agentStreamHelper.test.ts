import { describe, it, expect, vi, beforeEach } from "vitest";
import { listen } from "@tauri-apps/api/event";
import {
  setupStreamLifecycle,
  extractAndPersistFollowup,
} from "@/stores/slices/agentStreamHelper";
import type { Message } from "@/types";

const mockedListen = vi.mocked(listen);

type ListenHandler = (e: { payload: Record<string, unknown> }) => void | Promise<void>;

beforeEach(() => {
  mockedListen.mockReset();
});

describe("setupStreamLifecycle", () => {
  it("filters events by conversationId", async () => {
    const handlers = new Map<string, ListenHandler>();
    mockedListen.mockImplementation(async (event: string, handler: unknown) => {
      handlers.set(event, handler as ListenHandler);
      return () => {};
    });
    const onProgress = vi.fn();

    await setupStreamLifecycle({
      convId: "c1",
      engineKey: "claude",
      hasChunkEvent: true,
      onProgress,
      onChunk: vi.fn(),
      onCompleted: vi.fn().mockResolvedValue(undefined),
      onError: vi.fn().mockResolvedValue(undefined),
    });

    const progress = handlers.get("claude:progress");
    expect(progress).toBeDefined();
    await progress!({ payload: { messageId: "m1", conversationId: "c2", text: "other" } });
    expect(onProgress).not.toHaveBeenCalled();

    await progress!({ payload: { messageId: "m1", conversationId: "c1", text: "hi" } });
    expect(onProgress).toHaveBeenCalledWith({ messageId: "m1", conversationId: "c1", text: "hi" });
  });

  it("skips chunk listener when hasChunkEvent=false", async () => {
    const registered = new Set<string>();
    mockedListen.mockImplementation(async (event: string) => {
      registered.add(event);
      return () => {};
    });
    await setupStreamLifecycle({
      convId: "c1",
      engineKey: "codex",
      hasChunkEvent: false,
      onProgress: vi.fn(),
      onChunk: vi.fn(),
      onCompleted: vi.fn().mockResolvedValue(undefined),
      onError: vi.fn().mockResolvedValue(undefined),
    });
    expect(registered.has("codex:progress")).toBe(true);
    expect(registered.has("codex:chunk")).toBe(false);
    expect(registered.has("agent:completed")).toBe(true);
    expect(registered.has("agent:error")).toBe(true);
  });

  it("forwards error payload without messageId (branch variant)", async () => {
    const handlers = new Map<string, ListenHandler>();
    mockedListen.mockImplementation(async (event: string, handler: unknown) => {
      handlers.set(event, handler as ListenHandler);
      return () => {};
    });
    const onError = vi.fn().mockResolvedValue(undefined);
    await setupStreamLifecycle({
      convId: "c1",
      engineKey: "claude",
      hasChunkEvent: true,
      onProgress: vi.fn(),
      onChunk: vi.fn(),
      onCompleted: vi.fn().mockResolvedValue(undefined),
      onError,
    });
    const errHandler = handlers.get("agent:error")!;
    await errHandler({ payload: { conversationId: "c1", error: "boom" } });
    expect(onError).toHaveBeenCalledWith({ conversationId: "c1", error: "boom" });
  });

  it("cleanup detaches every listener that was registered", async () => {
    const unlisten = vi.fn();
    mockedListen.mockImplementation(async () => unlisten);
    const handle = await setupStreamLifecycle({
      convId: "c1",
      engineKey: "claude",
      hasChunkEvent: true,
      onProgress: vi.fn(),
      onChunk: vi.fn(),
      onCompleted: vi.fn().mockResolvedValue(undefined),
      onError: vi.fn().mockResolvedValue(undefined),
    });
    handle.cleanup();
    // progress + chunk + completed + error = 4 listeners
    expect(unlisten).toHaveBeenCalledTimes(4);
  });
});

describe("extractAndPersistFollowup", () => {
  it("returns null for undefined message", async () => {
    expect(await extractAndPersistFollowup(undefined, "c1")).toBeNull();
  });

  it("returns null for user messages (no tool-request path)", async () => {
    const msg: Message = {
      id: "u1", conversationId: "c1", role: "user",
      content: "hello", timestamp: 0, status: "done",
    };
    expect(await extractAndPersistFollowup(msg, "c1")).toBeNull();
  });

  it("returns null for assistant messages without markers", async () => {
    const msg: Message = {
      id: "a1", conversationId: "c1", role: "assistant",
      content: "a plain response with no markers",
      timestamp: 0, status: "done",
    };
    expect(await extractAndPersistFollowup(msg, "c1")).toBeNull();
  });
});
