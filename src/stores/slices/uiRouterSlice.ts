/**
 * UI router — state that represents "which domain object the UI is
 * currently focusing on", as opposed to "which tab the user has open"
 * (that one stays a CenterPanel-local concern driven by window events).
 *
 * Introduced for Finding 1-4 in `docs/plans/refactorRoadmap_2026-04-20.md`.
 * Only `focusedPlanId` is domain-level (which plan row should scroll
 * into view / pulse). The UI-scope events (`tunaflow:switch-tab`,
 * `tunaflow:switch-stage`, `tunaflow:open-settings`) deliberately stay
 * as window dispatches per the roadmap.
 */
import type { SetState, GetState } from "./types";

export interface UiRouterSlice {
  /**
   * Id of the plan that a route request asked the UI to focus on.
   * Subscribers (e.g. PlansPanel) read this and trigger their own scroll /
   * highlight. Consumers clear it by calling `focusPlan(null)` after they
   * handle the focus request so re-selecting the same plan still fires.
   */
  focusedPlanId: string | null;
  /** Set or clear the focused plan id. */
  focusPlan: (planId: string | null) => void;
}

export const createUiRouterSlice = (set: SetState, _get: GetState): UiRouterSlice => ({
  focusedPlanId: null,
  focusPlan: (planId: string | null) => {
    set({ focusedPlanId: planId });
  },
});
