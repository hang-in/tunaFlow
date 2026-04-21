import { describe, it, expect, beforeEach } from "vitest";
import { useChatStore } from "@/stores/chatStore";
import type { Conversation, Message } from "@/types";

const conv = (id: string): Conversation => ({
  id,
  projectKey: "p",
  label: id,
  type: "main",
  mode: "chat",
  source: "tunadish",
  createdAt: 0,
  updatedAt: 0,
  totalInputTokens: 0,
  totalOutputTokens: 0,
  totalCostUsd: 0,
});

const msg = (id: string, content = ""): Message => ({
  id,
  conversationId: "c1",
  role: "assistant",
  content,
  timestamp: 0,
  status: "streaming",
});

// Reset between tests so state from one case cannot leak into the next.
beforeEach(() => {
  const s = useChatStore.getState();
  s.resetConversationData();
  s.resetBranchState();
  s.clearConversationAssets();
});

describe("resetConversationData (conversationSlice)", () => {
  it("drops conversations / messages / selected id / stale set", () => {
    useChatStore.setState({
      conversations: [conv("c1"), conv("c2")],
      selectedConversationId: "c1",
      messages: [msg("m1"), msg("m2")],
      _staleConversations: new Set(["c3"]),
      error: "boom",
    });
    useChatStore.getState().resetConversationData();
    const s = useChatStore.getState();
    expect(s.conversations).toHaveLength(0);
    expect(s.selectedConversationId).toBeNull();
    expect(s.messages).toHaveLength(0);
    expect(s._staleConversations.size).toBe(0);
    expect(s.error).toBeNull();
  });
});

describe("applyStreamingUpdate (conversationSlice)", () => {
  it("patches the targeted message only", () => {
    useChatStore.setState({ messages: [msg("m1", ""), msg("m2", "")] });
    useChatStore.getState().applyStreamingUpdate("m1", { content: "hello" });
    const [m1, m2] = useChatStore.getState().messages;
    expect(m1.content).toBe("hello");
    expect(m2.content).toBe("");
  });

  it("no-ops when the id isn't present — no throw, no extra row", () => {
    useChatStore.setState({ messages: [msg("m1", "a")] });
    useChatStore.getState().applyStreamingUpdate("missing", { content: "x" });
    const ms = useChatStore.getState().messages;
    expect(ms).toHaveLength(1);
    expect(ms[0].content).toBe("a");
  });
});

describe("markConversationStale (conversationSlice)", () => {
  it("adds idempotently and preserves existing members", () => {
    useChatStore.setState({ _staleConversations: new Set(["c0"]) });
    useChatStore.getState().markConversationStale("c1");
    useChatStore.getState().markConversationStale("c1"); // idempotent
    const stale = useChatStore.getState()._staleConversations;
    expect(stale.has("c0")).toBe(true);
    expect(stale.has("c1")).toBe(true);
    expect(stale.size).toBe(2);
  });
});

describe("ensureConversation (conversationSlice)", () => {
  it("appends when the id is new", () => {
    useChatStore.setState({ conversations: [conv("c1")] });
    useChatStore.getState().ensureConversation(conv("c2"));
    const ids = useChatStore.getState().conversations.map((c) => c.id);
    expect(ids).toEqual(["c1", "c2"]);
  });

  it("is a no-op when the id already exists", () => {
    useChatStore.setState({ conversations: [conv("c1")] });
    useChatStore.getState().ensureConversation({ ...conv("c1"), label: "renamed" });
    const list = useChatStore.getState().conversations;
    expect(list).toHaveLength(1);
    // existing row preserved — label from the new arg is NOT applied
    expect(list[0].label).toBe("c1");
  });
});

describe("resetBranchState (branchSlice)", () => {
  it("drops branches + activeBranchId + parentConversationId", () => {
    useChatStore.setState({
      branches: [{ id: "b1", conversationId: "c1", label: "b", status: "active", mode: "chat", createdAt: 0 }],
      activeBranchId: "b1",
      parentConversationId: "c1",
    });
    useChatStore.getState().resetBranchState();
    const s = useChatStore.getState();
    expect(s.branches).toHaveLength(0);
    expect(s.activeBranchId).toBeNull();
    expect(s.parentConversationId).toBeNull();
  });
});

describe("clearConversationAssets (assetSlice)", () => {
  it("drops memos + artifacts but leaves skills/profiles alone", () => {
    useChatStore.setState({
      memos: [{
        id: "mem1", projectKey: "p", conversationId: "c1", messageId: "m1",
        type: "note", content: "x", tags: "", createdAt: 0,
      }],
      artifacts: [{
        id: "a1", conversationId: "c1", title: "t", type: "note",
        content: "", status: "draft", createdAt: 0, updatedAt: 0,
      }],
      skills: [{ name: "s", description: "d", content: "c", layer: "reference", bindPhases: [] }],
      agentProfiles: [{ id: "p", label: "p", engine: "claude", defaultSkills: [] }],
    });
    useChatStore.getState().clearConversationAssets();
    const s = useChatStore.getState();
    expect(s.memos).toHaveLength(0);
    expect(s.artifacts).toHaveLength(0);
    expect(s.skills).toHaveLength(1);
    expect(s.agentProfiles).toHaveLength(1);
  });
});
