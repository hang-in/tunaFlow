import { invoke } from "@tauri-apps/api/core";
import type { SetState, GetState, EngineModel, RawqStatus } from "./types";

export interface EngineModelSlice {
  engineModels: EngineModel[];
  rawqStatus: RawqStatus | null;
  loadEngineModels: (refresh?: boolean) => Promise<void>;
}

export const createEngineModelSlice = (set: SetState, get: GetState): EngineModelSlice => ({
  engineModels: [],
  rawqStatus: null,

  loadEngineModels: async (refresh?: boolean) => {
    try {
      const engineModels = await invoke<EngineModel[]>(
        refresh ? "refresh_engine_models" : "list_engine_models"
      );
      set({ engineModels });
    } catch (e) {
      console.warn("[engine models]", e);
    }
  },
});
