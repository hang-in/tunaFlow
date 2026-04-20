/**
 * Insight-panel runtime state that must survive tab unmount.
 *
 * `InsightPanel` lives inside `CenterPanel` which conditionally renders
 * tabs (`{tab === "insight" && <InsightPanel />}`), so switching to Chat
 * or Workflow tears the component down. Before this slice existed, the
 * `running` / `progressLines` flags were `useState` and therefore reset
 * to their initial values whenever the user revisited the tab — even
 * though the Tauri background command (`run_insight_analysis`) kept
 * executing. This slice parks the live analysis state at module scope
 * so the mounted panel can read the current value and keep rendering.
 */
import type { SetState, GetState } from "./types";

export interface InsightSlice {
  /** True while an analysis run is in flight. Set by `insightStartRun`,
   *  cleared by `insightFinishRun` / `insightFailRun`. */
  insightRunning: boolean;
  /** Append-only log shown in the progress panel. Retained after
   *  completion so users see what the last run did. */
  insightProgressLines: string[];
  /** Most recent session id — used to re-select the active session on
   *  remount so the findings list stays anchored to the latest run. */
  insightActiveSessionId: string | null;

  insightStartRun: () => void;
  insightAppendProgress: (line: string) => void;
  insightFinishRun: (sessionId?: string | null) => void;
  insightFailRun: (reason: string) => void;
  insightSetActiveSessionId: (id: string | null) => void;
}

export const createInsightSlice = (set: SetState, _get: GetState): InsightSlice => ({
  insightRunning: false,
  insightProgressLines: [],
  insightActiveSessionId: null,

  insightStartRun: () => set({ insightRunning: true, insightProgressLines: ["시작..."] }),

  insightAppendProgress: (line: string) =>
    set((state) => ({ insightProgressLines: [...state.insightProgressLines, line] })),

  insightFinishRun: (sessionId?: string | null) =>
    set((state) => ({
      insightRunning: false,
      insightActiveSessionId: sessionId ?? state.insightActiveSessionId,
    })),

  insightFailRun: (reason: string) =>
    set((state) => ({
      insightRunning: false,
      insightProgressLines: [...state.insightProgressLines, `✗ 실패: ${reason}`],
    })),

  insightSetActiveSessionId: (id: string | null) =>
    set({ insightActiveSessionId: id }),
});
