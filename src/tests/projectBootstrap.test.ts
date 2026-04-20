import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import {
  loadConversations,
  ensureMainConversation,
  ProjectBootstrapError,
  teardownPreviousProject,
} from "@/lib/bootstrap/project";
import type { AgentProfile, Conversation } from "@/types";

const mockedInvoke = vi.mocked(invoke);

const conv = (id: string, type: "main" | "discussion" = "discussion"): Conversation => ({
  id,
  projectKey: "p",
  label: id,
  type,
  mode: "chat",
  source: "tunadish",
  createdAt: 0,
  updatedAt: 0,
  totalInputTokens: 0,
  totalOutputTokens: 0,
  totalCostUsd: 0,
});

beforeEach(() => {
  mockedInvoke.mockReset();
});

describe("loadConversations", () => {
  it("returns existing conversations without creating Main", async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "list_conversations") return [conv("c1"), conv("c2")];
      throw new Error(`unexpected cmd ${cmd}`);
    });
    const list = await loadConversations("proj");
    expect(list).toHaveLength(2);
    expect(list[0].id).toBe("c1");
  });

  it("auto-creates a Main conversation when the project is empty", async () => {
    const created: string[] = [];
    mockedInvoke.mockImplementation(async (cmd: string, args: unknown) => {
      if (cmd === "list_conversations") return [];
      if (cmd === "create_conversation") {
        const input = (args as { input: { label: string; type: string } }).input;
        created.push(`${input.type}/${input.label}`);
        return conv("main-new", "main");
      }
      throw new Error(`unexpected cmd ${cmd}`);
    });
    const list = await loadConversations("proj");
    expect(list).toHaveLength(1);
    expect(list[0].type).toBe("main");
    expect(created).toEqual(["main/Main"]);
  });

  it("wraps backend failure in ProjectBootstrapError with the step name", async () => {
    mockedInvoke.mockImplementation(async () => {
      throw new Error("db offline");
    });
    await expect(loadConversations("proj")).rejects.toBeInstanceOf(ProjectBootstrapError);
    await expect(loadConversations("proj")).rejects.toMatchObject({ step: "loadConversations" });
  });
});

describe("ensureMainConversation", () => {
  const profile = (id: string, engine: string, model?: string): AgentProfile => ({
    id, label: engine, engine, model, defaultSkills: [],
  });

  function makeCallbacks(overrides?: Partial<Parameters<typeof ensureMainConversation>[1]>) {
    const savedEngine: { id: string; engine: { engine: string; model?: string; profileId: string | null } }[] = [];
    return {
      calls: savedEngine,
      cb: {
        setState: vi.fn(),
        getSelectedProjectKey: () => "p",
        getSelectedConversationId: () => null,
        selectConversation: vi.fn(async () => {}),
        getConversationEngine: () => null,
        saveConversationEngine: (id: string, engine: { engine: string; model?: string; profileId: string | null }) => {
          savedEngine.push({ id, engine });
        },
        getAgentProfiles: () => [profile("arch", "claude", "sonnet-4-6")],
        loadWorkflowSkills: vi.fn(async () => {}),
        loadSkills: vi.fn(async () => {}),
        detectAndRecommendSkills: vi.fn(async () => {}),
        ...overrides,
      },
    };
  }

  it("prefers a conversation with type=main", async () => {
    const { cb } = makeCallbacks();
    await ensureMainConversation([conv("c1"), conv("c2", "main"), conv("c3")], cb);
    expect(cb.selectConversation).toHaveBeenCalledWith("c2");
  });

  it("falls back to the first conversation when no main exists", async () => {
    const { cb } = makeCallbacks();
    await ensureMainConversation([conv("first"), conv("second")], cb);
    expect(cb.selectConversation).toHaveBeenCalledWith("first");
  });

  it("does not overwrite a saved engine/model for the selected conversation", async () => {
    const { cb, calls } = makeCallbacks({
      getConversationEngine: () => ({ engine: "codex", model: "gpt-5-codex", profileId: "saved" }),
    });
    await ensureMainConversation([conv("c1", "main")], cb);
    expect(calls).toHaveLength(0);
  });

  it("assigns the first agent profile when the conversation has no saved engine", async () => {
    const { cb, calls } = makeCallbacks();
    await ensureMainConversation([conv("c1", "main")], cb);
    expect(calls).toHaveLength(1);
    expect(calls[0]).toMatchObject({ id: "c1", engine: { engine: "claude", model: "sonnet-4-6", profileId: "arch" } });
  });

  it("no-ops on an empty conversation list", async () => {
    const { cb } = makeCallbacks();
    await ensureMainConversation([], cb);
    expect(cb.selectConversation).not.toHaveBeenCalled();
  });
});

describe("teardownPreviousProject", () => {
  it("is idempotent — safe to call when nothing is subscribed", () => {
    expect(() => teardownPreviousProject()).not.toThrow();
    expect(() => teardownPreviousProject()).not.toThrow();
  });
});
