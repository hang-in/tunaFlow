import { create } from "zustand";
import type { ChatState } from "./slices/types";
import { createProjectSlice } from "./slices/projectSlice";
import { createConversationSlice } from "./slices/conversationSlice";
import { createBranchSlice } from "./slices/branchSlice";
import { createRuntimeSlice } from "./slices/runtimeSlice";
import { createAssetSlice } from "./slices/assetSlice";
import { createEngineModelSlice } from "./slices/engineModelSlice";

export type { ChatState };

export const useChatStore = create<ChatState>((set, get) => ({
  ...createProjectSlice(set, get),
  ...createConversationSlice(set, get),
  ...createBranchSlice(set, get),
  ...createRuntimeSlice(set, get),
  ...createAssetSlice(set, get),
  ...createEngineModelSlice(set, get),
}));
