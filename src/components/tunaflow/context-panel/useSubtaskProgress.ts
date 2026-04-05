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
          planApi.updateSubtaskStatus(st.id, "done").catch(() => {});
        }
      }

      if (complete) {
        // impl-complete means ALL subtasks are done — fill any gaps
        const allDone = new Set(merged);
        for (const st of dbSubtasks) {
          allDone.add(st.idx + 1); // 0-based → 1-based
          if (st.status !== "done") {
            planApi.updateSubtaskStatus(st.id, "done").catch(() => {});
          }
        }
        setCompletedNums(allDone);
      } else {
        setCompletedNums(merged);
      }
      setImplComplete(complete);

      if (complete && !testRanRef.current && !cancelled.current) {
        testRanRef.current = true;
        try {
          const projectKey = useChatStore.getState().selectedProjectKey;
          if (projectKey) {
            const project = await invoke("get_project", { key: projectKey }) as { path?: string };
            if (project?.path) {
              setTestRunning(true);
              const result = await runProjectTests(project.path);
              if (!cancelled.current) {
                setTestResult(result);
                setTestRunning(false);
                testResultCache.set(plan.id, result);
              }
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
          for (const msg of reviewMsgs) {
            if (msg.role === "assistant" && hasReviewVerdict(msg.content)) {
              const v = extractReviewVerdict(msg.content);
              if (v && !cancelled.current) setReviewVerdict(v);
              break;
            }
          }
        } catch (e) { console.warn("[tunaflow]", e); }
      }

      if (plan.phase === "rework" || plan.phase === "subtask_review") {
        planApi.listPlanEvents(plan.id).then((events) => {
          if (!cancelled.current) {
            setDesignReviewSuggested(events.some((e) => e.eventType === "design_review_suggested"));
            const fails = events.filter((e) => e.eventType === "review_failed").length;
            setFailCount(fails);
            setDoomLoopEscalated(events.some((e) => e.eventType === "doom_loop_escalated"));
          }
        }).catch(() => {});
      }

      setLoading(false);
    })();

    const interval = setInterval(() => {
      if (plan.phase === "implementation" || plan.phase === "rework") {
        scanBranchState(cancelled);
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
