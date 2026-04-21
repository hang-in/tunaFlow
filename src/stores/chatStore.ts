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
