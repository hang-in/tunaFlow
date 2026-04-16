import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useChatStore } from "@/stores/chatStore";
import type { TraceSpan } from "./TraceSpanCard";

interface AgentJob {
  id: string;
  conversationId: string;
  messageId: string | null;
  engine: string;
  kind: string;
  status: string;
  error: string | null;
  startedAt: number;
  updatedAt: number;
}

interface MemoryStatus {
  state: string;
  sourceCount: number | null;
  messageCount: number;
  createdAt: number | null;
  updatedAt: number | null;
  newMessagesSince: number;
  summaryLength: number | null;
  topicCount: number;
  provenance: string | null;
  modelUsed: string | null;
}

export function useTraceData(convId: string | null) {
  const runningThreadIds = useChatStore((s) => s.runningThreadIds);
  const [spans, setSpans] = useState<TraceSpan[]>([]);
  const [jobs, setJobs] = useState<AgentJob[]>([]);
  const [loading, setLoading] = useState(false);
  const [tick, setTick] = useState(0);
  const [memoryStatus, setMemoryStatus] = useState<MemoryStatus | null>(null);

  const loadTraces = async () => {
    if (!convId) return;
    setLoading(true);
    try {
      const data = await invoke<TraceSpan[]>("list_traces", { conversationId: convId, traceId: null });
      setSpans(data);
    } catch { setSpans([]); }
    finally { setLoading(false); }
  };

  const loadJobs = async () => {
    try {
      const data = await invoke<AgentJob[]>("list_active_jobs");
      setJobs(data);
    } catch { setJobs([]); }
  };

  const loadMemoryStatus = async () => {
    if (!convId) return;
    try {
      const s = await invoke<MemoryStatus>("get_conversation_memory_status", { conversationId: convId });
      setMemoryStatus(s);
    } catch { setMemoryStatus(null); }
  };

  // Initial load
  useEffect(() => {
    loadTraces();
    loadJobs();
    loadMemoryStatus();
  }, [convId]);

  // Polling while running
  const threadRunning = convId ? runningThreadIds.includes(convId) : false;
  useEffect(() => {
    if (!threadRunning) return;
    let traceTickCount = 0;
    const interval = setInterval(() => {
      loadJobs();
      setTick((t) => t + 1);
      // Traces: refresh every 5 ticks (5s) during active run
      traceTickCount++;
      if (traceTickCount >= 5) { traceTickCount = 0; loadTraces(); }
    }, 1000);
    return () => clearInterval(interval);
  }, [threadRunning]);

  // Auto-refresh traces when a run completes (threadRunning goes false)
  const prevRunning = useState(false);
  useEffect(() => {
    if (prevRunning[0] && !threadRunning) { loadTraces(); loadMemoryStatus(); }
    prevRunning[1](threadRunning);
  }, [threadRunning]);

  return {
    spans, jobs, loading, tick, memoryStatus,
    threadRunning,
    refresh: () => { loadTraces(); loadJobs(); loadMemoryStatus(); },
    refreshMemory: loadMemoryStatus,
  };
}

export type { AgentJob, MemoryStatus };
