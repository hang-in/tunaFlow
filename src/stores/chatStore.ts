import { create } from "zustand";
import type { ChatState } from "./slices/types";
import { createProjectSlice } from "./slices/projectSlice";
import { createConversationSlice } from "./slices/conversationSlice";
import { createBranchSlice } from "./slices/branchSlice";
import { createThreadSlice } from "./slices/threadSlice";
import { createRuntimeSlice } from "./slices/runtimeSlice";
import { createAssetSlice } from "./slices/assetSlice";
import { createEngineModelSlice } from "./slices/engineModelSlice";
import { createInsightSlice } from "./slices/insightSlice";
import { createUiRouterSlice } from "./slices/uiRouterSlice";

export type { ChatState };

// TEMP DIAGNOSTIC (remove after msg-disappearance is resolved):
// Log every transition where `messages.length` shrinks so we can see exactly
// which call-site drops the user/assistant message during streaming. Enable
// by setting `localStorage.debugMessages = "1"` in the webview console.
function wrapSetForDiagnostic<T>(rawSet: T): T {
  const enabled =
    typeof window !== "undefined" &&
    window.localStorage?.getItem("debugMessages") === "1";
  if (!enabled) return rawSet;
  const s = rawSet as unknown as (...args: unknown[]) => void;
  const wrapped = ((...args: unknown[]) => {
    const prevLen = (useChatStore.getState() as { messages?: unknown[] })
      .messages?.length;
    s(...args);
    const nextLen = (useChatStore.getState() as { messages?: unknown[] })
      .messages?.length;
    if (
      typeof prevLen === "number" &&
      typeof nextLen === "number" &&
      nextLen < prevLen
    ) {
      const stack = new Error().stack?.split("\n").slice(2, 8).join("\n") ?? "";
      console.warn(
        `[debug-messages] length ${prevLen} → ${nextLen}\n${stack}`
      );
    }
  }) as unknown as T;
  return wrapped;
}

export const useChatStore = create<ChatState>((rawSet, get) => {
  const set = wrapSetForDiagnostic(rawSet);
  return {
    _staleConversations: new Set<string>(),
    ...createProjectSlice(set, get),
    ...createConversationSlice(set, get),
    ...createBranchSlice(set, get),
    ...createThreadSlice(set, get),
    ...createRuntimeSlice(set, get),
    ...createAssetSlice(set, get),
    ...createEngineModelSlice(set, get),
    ...createInsightSlice(set, get),
    ...createUiRouterSlice(set, get),
  };
});
