import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useChatStore } from "@/stores/chatStore";
import type { Plan, PlanSubtask, Message } from "@/types";
import * as planApi from "@/lib/api/plans";
import { scanCompletedSubtasks, hasImplComplete, hasReviewVerdict, extractReviewVerdict } from "@/lib/planProposalParser";
import { runProjectTests, type TestRunResult } from "@/lib/api/testRunner";
import type { ParsedReviewVerdict } from "@/lib/planProposalParser";

// Module-level cache: prevents re-running tests on tab switch (component remount)
const testResultCache = new Map<string, TestRunResult>();

export function useSubtaskProgress(plan: Plan) {
  const [subtasks, setSubtasks] = useState<PlanSubtask[]>([]);
  const [completedNums, setCompletedNums] = useState<Set<number>>(new Set());
  const [implComplete, setImplComplete] = useState(false);
  const [loading, setLoading] = useState(true);
  const [testResult, setTestResult] = useState<TestRunResult | null>(() => testResultCache.get(plan.id) ?? null);
  const [testRunning, setTestRunning] = useState(false);
  // Ref to prevent stale closure re-triggering tests in polling interval
  const testRanRef = useRef(testResultCache.has(plan.id));
  const [reviewVerdict, setReviewVerdict] = useState<ParsedReviewVerdict | null>(null);
  const [designReviewSuggested, setDesignReviewSuggested] = useState(false);
  const [failCount, setFailCount] = useState(0);
  const [doomLoopEscalated, setDoomLoopEscalated] = useState(false);

  const scanBranchState = async (cancelled: { current: boolean }) => {
    if (!plan.implementationBranchId) return;
    try {
      const shadowConvId = `branch:${plan.implementationBranchId}`;
      const msgs = await invoke<Message[]>("list_messages", { conversationId: shadowConvId });
      if (cancelled.current) return;
      const scanned = scanCompletedSubtasks(msgs);
      const complete = msgs.some((m) => m.role === "assistant" && hasImplComplete(m.content));

      // Merge: marker scan + DB subtask status
      // Handles: Developer didn't emit per-subtask markers in earlier rounds,
      // or Rework round only markers some subtasks
      // Note: scanned (from markers) uses 1-based numbers (subtask-done:1, subtask-done:2)
      // DB subtask.idx is 0-based → convert to 1-based for consistency
      const merged = new Set(scanned);
      let dbSubtasks: PlanSubtask[] = [];
      try {
        dbSubtasks = await planApi.listSubtasks(plan.id);
        for (const st of dbSubtasks) {
          if (st.status === "done") merged.add(st.idx + 1); // 0-based → 1-based
        }
      } catch { /* use marker scan only */ }

      // Sync marker-detected completions back to DB (fire-and-forget)
      // Markers are 1-based, DB idx is 0-based
      for (const num of scanned) {
        const st = dbSubtasks.find((s) => s.idx === num - 1);
        if (st && st.status !== "done") {
          planApi.updateSubtaskStatus(st.id, "done").catch((e) => console.debug("[subtask-sync]", e));
        }
      }

      if (complete) {
        // impl-complete means ALL subtasks are done — fill any gaps
        const allDone = new Set(merged);
        for (const st of dbSubtasks) {
          allDone.add(st.idx + 1); // 0-based → 1-based
          if (st.status !== "done") {
            planApi.updateSubtaskStatus(st.id, "done").catch((e) => console.debug("[subtask-sync]", e));
          }
        }
        setCompletedNums(allDone);
      } else {
        setCompletedNums(merged);
      }
      // Fallback: all subtasks done + agent not running → infer impl-complete
      // Even if the agent didn't emit the marker, DB state is authoritative
      const effectiveComplete = complete || (() => {
        if (dbSubtasks.length === 0) return false;
        const allDone = dbSubtasks.every((st) => st.status === "done");
        const shadowConvId = `branch:${plan.implementationBranchId}`;
        const notRunning = !useChatStore.getState().runningThreadIds.includes(shadowConvId);
        return allDone && notRunning;
      })();
      setImplComplete(effectiveComplete);

      if (effectiveComplete && !testRanRef.current && !cancelled.current) {
        testRanRef.current = true;
        try {
          const projectKey = useChatStore.getState().selectedProjectKey;
          if (projectKey) {
            const project = await invoke("get_project", { key: projectKey }) as { path?: string };
            if (project?.path) {
              setTestRunning(true);
              const result = await runProjectTests(project.path);
              // Always cache + update state — even if cancelled (user switched tabs
              // while test was running). Without this, test re-runs on every tab switch.
              testResultCache.set(plan.id, result);
              setTestResult(result);
              setTestRunning(false);
            }
          }
        } catch (e) {
          console.warn("[tunaflow] test run failed:", e);
          setTestRunning(false);
        }
      }
    } catch (e) { console.warn("[tunaflow]", e); }
  };

  useEffect(() => {
    const cancelled = { current: false };
    setLoading(true);

    (async () => {
      const sts = await planApi.listSubtasks(plan.id).catch(() => [] as PlanSubtask[]);
      if (cancelled.current) return;
      setSubtasks(sts);

      await scanBranchState(cancelled);

      if (plan.reviewBranchId && (plan.phase === "rework" || plan.phase === "review")) {
        try {
          const reviewShadow = `branch:${plan.reviewBranchId}`;
          const reviewMsgs = await invoke<Message[]>("list_messages", { conversationId: reviewShadow });
          // Use LAST verdict — followup reviews supersede earlier ones
          let latestVerdict: ParsedReviewVerdict | null = null;
          for (const msg of reviewMsgs) {
            if (msg.role === "assistant" && hasReviewVerdict(msg.content)) {
              const v = extractReviewVerdict(msg.content);
              if (v) latestVerdict = v;
            }
          }
          if (latestVerdict && !cancelled.current) setReviewVerdict(latestVerdict);
        } catch (e) { console.warn("[tunaflow]", e); }
      }

      if (plan.phase === "rework" || plan.phase === "subtask_review") {
        planApi.listPlanEvents(plan.id).then((events) => {
          if (!cancelled.current) {
            // Count failures since last escalation (not total)
            let lastEscIdx = -1;
            for (let i = events.length - 1; i >= 0; i--) {
              if (events[i].eventType === "doom_loop_escalated" || events[i].eventType === "architect_redesign_requested") { lastEscIdx = i; break; }
            }
            const sinceReset = lastEscIdx >= 0 ? events.slice(lastEscIdx + 1) : events;
            setDesignReviewSuggested(sinceReset.some((e: { eventType: string }) => e.eventType === "design_review_suggested"));
            const fails = sinceReset.filter((e: { eventType: string }) => e.eventType === "review_failed").length;
            setFailCount(fails);
            // doomLoopEscalated 도 sinceReset 기반으로 판정. 이전엔 events 전체에서
            // 찾아 한 번 escalation 이 발생하면 영구히 true 로 고정되는 버그가 있었음
            // → 새 rev 싸이클에서 Review 1회만 실패해도 "1회 연속 실패" 배너가 잘못 뜸.
            setDoomLoopEscalated(sinceReset.some((e: { eventType: string }) => e.eventType === "doom_loop_escalated"));
          }
        }).catch((e) => console.debug("[plan-events]", e));
      }

      setLoading(false);
    })();

    const interval = setInterval(() => {
      if (plan.phase === "implementation" || plan.phase === "rework") {
        scanBranchState(cancelled);
      }
      // Also poll for verdict during review phase (safety net for auto-detect)
      if (plan.phase === "review" && plan.reviewBranchId) {
        const reviewShadow = `branch:${plan.reviewBranchId}`;
        invoke<Message[]>("list_messages", { conversationId: reviewShadow }).then((reviewMsgs) => {
          if (cancelled.current) return;
          let latest: ParsedReviewVerdict | null = null;
          for (const msg of reviewMsgs) {
            if (msg.role === "assistant" && hasReviewVerdict(msg.content)) {
              const v = extractReviewVerdict(msg.content);
              if (v) latest = v;
            }
          }
          if (latest) setReviewVerdict(latest);
        }).catch((e) => console.debug("[verdict-poll]", e));
      }
    }, 5000);

    return () => { cancelled.current = true; clearInterval(interval); };
  }, [plan.id, plan.implementationBranchId, plan.phase]);

  return {
    subtasks,
    completedNums,
    implComplete,
    loading,
    testResult,
    testRunning,
    reviewVerdict,
    designReviewSuggested,
    failCount,
    doomLoopEscalated,
  };
}
