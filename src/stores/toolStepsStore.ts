/**
 * Lightweight store for tracking tool steps per streaming message.
 *
 * Separate from chatStore to avoid coupling with message state.
 * Steps are transient during streaming, then saved to progressContent on completion.
 * NOT used for search or ContextPack — display-only (lazy-loaded).
 */

import { create } from "zustand";
import type { ToolStep } from "@/lib/toolSteps";
import { isToolStep, parseToolStep } from "@/lib/toolSteps";

interface ToolStepsState {
  /** messageId → accumulated tool steps */
  stepsMap: Record<string, ToolStep[]>;
  /** messageId → streaming start time */
  startTimeMap: Record<string, number>;

  /** Process a progress event — parse __STEP__ or ignore */
  handleProgress: (messageId: string, text: string) => void;
  /** Get steps for a message */
  getSteps: (messageId: string) => ToolStep[];
  /** Get elapsed time for a message */
  getElapsed: (messageId: string) => number;
  /** Clear steps for a completed message (after saving to progressContent) */
  clear: (messageId: string) => void;
}

export const useToolStepsStore = create<ToolStepsState>((set, get) => ({
  stepsMap: {},
  startTimeMap: {},

  handleProgress: (messageId: string, text: string) => {
    if (!isToolStep(text)) return;
    const step = parseToolStep(text);
    if (!step) return;

    set((state) => {
      const existing = state.stepsMap[messageId] ?? [];

      // If this is a "done"/"error" step, update the last "running" step with same name
      if (step.status !== "running") {
        const updated = [...existing];
        let runningIdx = -1;
        for (let i = updated.length - 1; i >= 0; i--) {
          if (updated[i].status === "running" && updated[i].name === step.name) {
            runningIdx = i;
            break;
          }
        }
        if (runningIdx >= 0) {
          updated[runningIdx] = { ...updated[runningIdx], status: step.status, ts: step.ts };
          if (step.input) updated[runningIdx].input = step.input;
          return {
            stepsMap: { ...state.stepsMap, [messageId]: updated },
            startTimeMap: state.startTimeMap[messageId]
              ? state.startTimeMap
              : { ...state.startTimeMap, [messageId]: Date.now() },
          };
        }
      }

      return {
        stepsMap: { ...state.stepsMap, [messageId]: [...existing, step] },
        startTimeMap: state.startTimeMap[messageId]
          ? state.startTimeMap
          : { ...state.startTimeMap, [messageId]: Date.now() },
      };
    });
  },

  getSteps: (messageId: string) => get().stepsMap[messageId] ?? [],

  getElapsed: (messageId: string) => {
    const start = get().startTimeMap[messageId];
    return start ? Date.now() - start : 0;
  },

  clear: (messageId: string) => {
    set((state) => {
      const { [messageId]: _s, ...restSteps } = state.stepsMap;
      const { [messageId]: _t, ...restTimes } = state.startTimeMap;
      return { stepsMap: restSteps, startTimeMap: restTimes };
    });
  },
}));
