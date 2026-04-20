/**
 * Shared agent streaming helpers — extracted from runtimeSlice + threadSlice
 * to eliminate event-listener duplication (Finding 1-3 Step 2).
 *
 * Responsibilities:
 *   - `setupStreamLifecycle` — progress/chunk/completed/error listener wiring
 *     with conversation filtering + tool-steps logging. Main/branch
 *     differences live in the injected callbacks, not inside this module.
 *   - `extractAndPersistFollowup` — detect tool-request markers in the
 *     completed message, execute them, and persist the resulting system
 *     message. The caller decides when to call `_endRun` and whether to
 *     re-enter the send path (those signatures differ across slices).
 *   - `handleToolRequests` / `saveToolSteps` — low-level helpers retained
 *     from the previous revision, still used directly by the branch path.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useToolStepsStore } from "@/stores/toolStepsStore";
import { serializeSteps } from "@/lib/toolSteps";
import type { Message } from "@/types";

// ─── Event payload types ───────────────────────────────────────────────

export interface ProgressPayload {
  messageId: string;
  conversationId: string;
  text: string;
}

export interface ChunkPayload {
  messageId: string;
  conversationId: string;
  text: string;
}

export interface CompletedPayload {
  messageId: string;
  conversationId: string;
  durationMs?: number;
  inputTokens?: number;
  outputTokens?: number;
  costUsd?: number;
}

export interface ErrorPayload {
  /** Present on the main-chat path; omitted on branch thread errors. */
  messageId?: string;
  conversationId: string;
  error: string;
}

// ─── setupStreamLifecycle ──────────────────────────────────────────────

export interface StreamLifecycleConfig {
  convId: string;
  engineKey: string;
  hasChunkEvent: boolean;
  onProgress: (payload: ProgressPayload) => void;
  onChunk: (payload: ChunkPayload) => void;
  onCompleted: (payload: CompletedPayload) => Promise<void>;
  onError: (payload: ErrorPayload) => Promise<void>;
}

export interface StreamLifecycleHandle {
  /** Detach all listeners. Safe to call multiple times. */
  cleanup: () => void;
}

/**
 * Wire the four-listener pattern (progress / chunk / completed / error) used
 * by both runtimeSlice.sendWithEngine and threadSlice.sendThreadMessage.
 *
 * The caller supplies UI/state mutations via callbacks — this function
 * handles conversation filtering (each listener fires for every active
 * conversation; we filter to `convId`), tool-steps logging on progress,
 * tool-steps persistence on completed, and listener cleanup.
 *
 * Chunk throttling is NOT applied here; the caller should wrap `onChunk`
 * with `createSingleChunkThrottler` when desired (main-chat path) or pass
 * a direct handler (branch drawer path).
 */
export async function setupStreamLifecycle(
  config: StreamLifecycleConfig,
): Promise<StreamLifecycleHandle> {
  const { convId, engineKey, hasChunkEvent, onProgress, onChunk, onCompleted, onError } = config;

  const progressEvent = `${engineKey}:progress`;
  const chunkEvent = `${engineKey}:chunk`;

  const ulProgress = await listen<ProgressPayload>(progressEvent, (e) => {
    if (e.payload.conversationId !== convId) return;
    // Parse tool steps from the __STEP__ prefix even after the user has
    // navigated away — the step log is persisted to DB on completion and
    // must reflect the full run, not only the segment where UI was visible.
    useToolStepsStore.getState().handleProgress(e.payload.messageId, e.payload.text);
    onProgress(e.payload);
  });

  const ulChunk = hasChunkEvent
    ? await listen<ChunkPayload>(chunkEvent, (e) => {
        if (e.payload.conversationId !== convId) return;
        onChunk(e.payload);
      })
    : () => {};

  const ulCompleted = await listen<CompletedPayload>("agent:completed", async (e) => {
    if (e.payload.conversationId !== convId) return;
    // Persist accumulated tool-steps before the completion callback triggers
    // its DB reload — otherwise the reload would overwrite in-memory steps.
    await saveToolSteps(e.payload.messageId);
    await onCompleted(e.payload);
  });

  const ulError = await listen<ErrorPayload>("agent:error", async (e) => {
    if (e.payload.conversationId !== convId) return;
    await onError(e.payload);
  });

  return {
    cleanup: () => {
      ulProgress();
      ulChunk();
      ulCompleted();
      ulError();
    },
  };
}

// ─── Tool-steps persistence ────────────────────────────────────────────

export async function saveToolSteps(messageId: string): Promise<void> {
  const tsStore = useToolStepsStore.getState();
  const steps = tsStore.getSteps(messageId);
  if (steps.length > 0) {
    invoke("save_progress_content", { messageId, progressContent: serializeSteps(steps) })
      .catch((e) => console.debug("[save-steps]", e));
    tsStore.clear(messageId);
  }
}

// ─── Tool-request follow-up ────────────────────────────────────────────

export interface FollowupResult {
  followUp: string;
  /**
   * Persisted system-message id. `null` when persistence failed; the caller
   * may still send the follow-up but Rust will create a fresh user-message
   * instead of reusing this id.
   */
  sysMsgId: string | null;
}

/**
 * Detect tool-request markers in the completed assistant message, execute
 * them, and persist the resulting follow-up text as a system message.
 *
 * The caller is responsible for `_endRun(...)` bookkeeping and the recursive
 * re-send — both of those differ meaningfully between runtimeSlice (main
 * chat: `_endRun(convId)`, recurses via `sendWithEngine`) and threadSlice
 * (branch drawer: `_endRun(convId, { silent: true })` before recursing via
 * `sendThreadMessage`), so consolidating them here would either blur the
 * idle↔running flicker contract or force an ugly variant flag.
 *
 * Returns `null` when there are no markers, letting callers use a plain
 * `if (!followup) { _endRun(); return; }` pattern.
 */
export async function extractAndPersistFollowup(
  msg: Message | undefined,
  convId: string,
): Promise<FollowupResult | null> {
  const followUp = await handleToolRequests(msg);
  if (!followUp) return null;
  const sysMsgId = await invoke<string>("persist_system_msg", {
    conversationId: convId,
    content: followUp,
  }).catch((e) => {
    console.warn("[tool-request] persist_system_msg failed:", e);
    return null;
  });
  return { followUp, sysMsgId };
}

export async function handleToolRequests(
  message: Message | undefined,
): Promise<string | null> {
  if (!message || message.role !== "assistant") return null;
  try {
    const { extractToolRequests } = await import("@/lib/planProposalParser");
    const requests = extractToolRequests(message.content);
    if (requests.length > 0) {
      const { executeToolRequests } = await import("@/lib/toolRequestHandler");
      return executeToolRequests(requests);
    }
  } catch (err) {
    console.warn("[tool-request]", err);
  }
  return null;
}
