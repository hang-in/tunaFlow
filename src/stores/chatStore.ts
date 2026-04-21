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

export const useChatStore = create<ChatState>((set, get) => ({
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
}));

// TEMP DIAGNOSTIC (remove after msg-disappearance is resolved):
// Subscribe at the store level so every transition — regardless of whether
// it's routed through the internal `set()` or a direct `useChatStore.setState()` —
// is observed. Fires only when `messages.length` shrinks, which is the
// specific symptom we're chasing.
//
// Enable by setting `localStorage.debugMessages = "1"` in the webview console,
// then reload. Disable by `localStorage.removeItem("debugMessages")`.
if (typeof window !== "undefined") {
  const enabled = () => window.localStorage?.getItem("debugMessages") === "1";
  let prev = useChatStore.getState().messages;
  useChatStore.subscribe((state) => {
    if (!enabled()) {
      prev = state.messages;
      return;
    }
    if (state.messages !== prev && state.messages.length < prev.length) {
      const lostIds = prev
        .filter((m) => !state.messages.find((n) => n.id === m.id))
        .map((m) => ({ id: m.id, role: m.role, status: m.status, len: m.content?.length ?? 0 }));
      console.warn(
        `[debug-messages] length ${prev.length} → ${state.messages.length}`,
        { lostIds, stillHas: state.messages.map((m) => m.id) }
      );
      console.trace("[debug-messages] stack");
    }
    prev = state.messages;
  });
}
