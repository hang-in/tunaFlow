import { describe, it, expect, beforeEach } from "vitest";
import { useChatStore } from "@/stores/chatStore";

beforeEach(() => {
  useChatStore.getState().focusPlan(null);
});

describe("uiRouterSlice.focusPlan", () => {
  it("sets the focused plan id", () => {
    useChatStore.getState().focusPlan("plan-xyz");
    expect(useChatStore.getState().focusedPlanId).toBe("plan-xyz");
  });

  it("clears when called with null", () => {
    useChatStore.getState().focusPlan("plan-xyz");
    useChatStore.getState().focusPlan(null);
    expect(useChatStore.getState().focusedPlanId).toBeNull();
  });

  it("overwrites a previous focus request", () => {
    useChatStore.getState().focusPlan("plan-a");
    useChatStore.getState().focusPlan("plan-b");
    expect(useChatStore.getState().focusedPlanId).toBe("plan-b");
  });
});
