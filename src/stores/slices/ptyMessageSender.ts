/**
 * Shared PTY message sending logic.
 * Used by both runtimeSlice (main panel) and threadSlice (branch drawer).
 */
import { invoke } from "@tauri-apps/api/core";
import { errorMessage } from "@/lib/utils";
import { usePtyStore } from "@/stores/ptyStore";
import type { PtyEngine } from "@/stores/ptyStore";
import { ENGINE_CONFIGS } from "@/lib/engineConfig";
import type { SetState, GetState, Message } from "./types";

/** Engine-specific JSONL poll/list commands */
export function getPtyPollConfig(engine: string) {
  switch (engine) {
    case "codex": return { pollCmd: "pty_poll_codex", listCmd: "pty_list_codex_files" };
    case "gemini": return { pollCmd: "pty_poll_gemini", listCmd: "pty_list_gemini_files" };
    default: return { pollCmd: "pty_poll_jsonl", listCmd: "pty_list_jsonl_files" };
  }
}

export interface PtySendOptions {
  /** Which message array to update in the store */
  messageTarget: "messages" | "threadMessages";
  /** Guard: only update UI when this returns true */
  isActiveCheck: () => boolean;
  /**
   * Called after successful DB save, before _endRun.
   * Return true if you handle _endRun yourself (e.g. for tool-request follow-ups).
   */
  onCompleted?: (savedMsg: Message, text: string) => Promise<boolean>;
}

/**
 * Send a message through the active PTY session.
 * Handles: user message save → PTY write → JSONL polling → completion → DB save → UI update.
 * Manages _startRun/_endRun internally.
 */
export async function sendMessageViaPty(
  set: SetState, get: GetState,
  prompt: string, sessionId: number, conversationId: string, engine: string = "claude",
  opts: PtySendOptions,
): Promise<void> {
  const { listen: listenEvent } = await import("@tauri-apps/api/event");
  const mt = opts.messageTarget;

  get()._startRun(conversationId);
  const now = Date.now();

  // Save user message to DB
  let userMsgId: string;
  try {
    const userMsg = await invoke<Message>("append_user_message", {
      input: { conversationId, content: prompt },
    });
    userMsgId = userMsg.id;
  } catch (e) {
    console.error("[pty] append_user_message failed:", e);
    userMsgId = `temp-user-${now}`;
  }

  const asstMsgId = `pty-${now}`;
  const engineKey = (ENGINE_CONFIGS[engine] ?? ENGINE_CONFIGS.claude).engineKey;

  // Add messages to store
  set((state: any) => ({
    error: null,
    [mt]: [
      ...(state[mt] as Message[]),
      { id: userMsgId, conversationId, role: "user" as const, content: prompt, timestamp: now, status: "done" as const },
      { id: asstMsgId, conversationId, role: "assistant" as const, content: "", timestamp: now, status: "streaming" as const, engine: engineKey },
    ],
  }));

  // Mark PTY as capturing
  usePtyStore.getState().endCapture();
  usePtyStore.getState().startCapture(asstMsgId, engine as PtyEngine);

  let finalized = false;
  let hasToolSteps = false;

  const ptySession = usePtyStore.getState().sessions.get(engine as PtyEngine);
  const projectPath = ptySession?.projectPath || "";
  const jsonlPath = ptySession?.jsonlPath;

  // Helper: update assistant message in the targeted array
  const updateAsstMsg = (update: Partial<Message>) => {
    if (!opts.isActiveCheck()) return;
    set((state: any) => ({
      [mt]: (state[mt] as Message[]).map((m: Message) =>
        m.id === asstMsgId ? { ...m, ...update } : m
      ),
    }));
  };

  // Status indicator via pty:screen (visual only)
  const ulScreen = await listenEvent<{ sessionId: number; data: string }>("pty:screen", (e) => {
    if (e.payload.sessionId !== sessionId || finalized || hasToolSteps) return;
    const status = /⏺/.test(e.payload.data) ? "responding..." : /[✻✢✳✶✽]/.test(e.payload.data) ? "thinking..." : null;
    if (status) updateAsstMsg({ progressContent: status });
  });

  const pollConfig = getPtyPollConfig(engine);

  // Get current JSONL line count (baseline — poll for lines after this)
  let trackedJsonl = jsonlPath ?? null;
  let baselineLines = 0;
  if (trackedJsonl) {
    try {
      const pollArgs: Record<string, unknown> = { afterLine: 0 };
      if (engine === "claude") { pollArgs.projectPath = projectPath; pollArgs.jsonlPath = trackedJsonl; }
      else if (engine === "gemini") { pollArgs.jsonPath = trackedJsonl; pollArgs.afterMessageCount = 0; }
      else { pollArgs.jsonlPath = trackedJsonl; pollArgs.afterLine = 0; }
      const baseline = await invoke<{ totalLines: number; isComplete: boolean; [k: string]: unknown } | null>(
        pollConfig.pollCmd, pollArgs
      );
      if (baseline) baselineLines = baseline.totalLines;
    } catch { /* ok */ }
  }

  // Snapshot existing files for JSONL detection after prompt
  let snapshotBefore: Set<string> | null = null;
  if (!trackedJsonl) {
    try {
      const files = await invoke<string[]>(pollConfig.listCmd, { projectPath });
      snapshotBefore = new Set(files);
    } catch { /* ok */ }
  }

  // ContextPack handling:
  // - New session (no trackedJsonl): inject full ContextPack into prompt
  // - Existing session: update CLAUDE.md with current context (delta via file)
  let enrichedPrompt = prompt;
  try {
    const { activeSkills, crossSessionIds } = get();
    if (!trackedJsonl) {
      const contextResult = await invoke<{ assembledPrompt: string; sections: string[] }>(
        "pty_build_context", {
          conversationId, prompt, projectPath: projectPath || null,
          activeSkills: activeSkills ?? [], crossSessionIds: crossSessionIds ?? [],
          personaFragment: null, contextMode: null,
        }
      );
      if (contextResult.assembledPrompt) {
        enrichedPrompt = contextResult.assembledPrompt;
        console.log(`[pty] injected full ContextPack (${contextResult.sections.length} sections)`);
      }
    } else {
      // PTY session maintains its own conversation history — no need to update CLAUDE.md.
      // Previous design wrote ContextPack delta to CLAUDE.md, but this caused
      // infinite accumulation (see knownIssues_2026-04-12.md).
      // Claude CLI reads CLAUDE.md on its own; the first message already has full context.
    }
  } catch (e) {
    console.warn("[pty] ContextPack failed, sending raw prompt:", e);
  }

  // ── Timeouts ──
  const START_TIMEOUT_MS = 90_000;  // 90s: must see ANY activity (screen or JSONL) after write
  const TOTAL_TIMEOUT_MS = 600_000; // 10min: total execution limit (agents can run long)
  const POLL_INTERVAL_MS = 200;

  // Send prompt via bracket paste + Enter
  try {
    // ── Pre-write: ensure CLI is ready (❯ prompt visible) ──
    const screen = await invoke<string>("pty_get_screen", { sessionId }).catch(() => "");
    if (screen && !/❯/.test(screen)) {
      // CLI not showing prompt — send Enter to wake it
      console.log("[pty] CLI not ready, sending wake signal...");
      await invoke("pty_write", { sessionId, data: "\r" });
      // Wait for prompt to appear (max 10s)
      const { listen } = await import("@tauri-apps/api/event");
      await new Promise<void>((resolve) => {
        const timeout = setTimeout(() => { ul(); resolve(); }, 10_000);
        let ul = () => {};
        listenEvent<{ sessionId: number; data: string }>("pty:screen", (e) => {
          if (e.payload.sessionId === sessionId && /❯/.test(e.payload.data)) {
            clearTimeout(timeout); ul(); resolve();
          }
        }).then((u) => { ul = u; });
      });
    }

    await invoke("pty_write", { sessionId, data: `\x1b[200~${enrichedPrompt}\x1b[201~` });
    // Scale delay with prompt size — large prompts need more time for bracket paste to flush
    const pasteDelayMs = Math.max(200, Math.min(800, Math.floor(enrichedPrompt.length / 300)));
    await new Promise((r) => setTimeout(r, pasteDelayMs));
    await invoke("pty_write", { sessionId, data: "\r" });

    // ── Delivery confirmation: check pty:screen for idle prompt disappearing ──
    let deliveryConfirmed = false;
    let activitySeen = false; // Any signal that the agent is working (screen or JSONL)

    const ulDelivery = await listenEvent<{ sessionId: number; data: string }>("pty:screen", (e) => {
      if (e.payload.sessionId !== sessionId || deliveryConfirmed) return;
      // If we see ⏺ (response indicator) or thinking symbols, delivery is confirmed
      if (/⏺|[✻✢✳✶✽]/.test(e.payload.data)) {
        deliveryConfirmed = true;
        activitySeen = true; // Screen activity counts — resets start timeout
        ulDelivery();
        console.log("[pty] delivery confirmed via screen indicator");
      }
    });

    // Poll JSONL for assistant messages
    for (let attempt = 0; ; attempt++) {
      await new Promise((r) => setTimeout(r, POLL_INTERVAL_MS));
      if (finalized) break;

      const elapsed = Date.now() - now;

      // ── Start timeout: no JSONL activity within 30s → error ──
      if (!activitySeen && elapsed > START_TIMEOUT_MS) {
        finalized = true;
        ulScreen();
        ulDelivery();
        usePtyStore.getState().endCapture();
        console.error("[pty] start timeout: no activity within 90s");
        updateAsstMsg({
          content: "(PTY 전달 실패 — 90초 내 응답 없음. 에이전트 상태를 확인하세요)",
          status: "error" as const,
        });
        get()._endRun(conversationId);
        return;
      }

      // ��─ Total timeout: 3 minutes ──
      if (elapsed > TOTAL_TIMEOUT_MS) {
        finalized = true;
        ulScreen();
        ulDelivery();
        usePtyStore.getState().endCapture();
        console.error("[pty] total timeout: 10 minutes exceeded");
        updateAsstMsg({
          content: "(응답 대기 시간 초과 — 10분)",
          status: "error" as const,
        });
        get()._endRun(conversationId);
        return;
      }

      // Lazy JSONL detection: if no tracked path, detect new file by snapshot diff
      if (!trackedJsonl && snapshotBefore && attempt >= 10 && attempt % 15 === 0) {
        try {
          const filesNow = await invoke<string[]>("pty_list_jsonl_files", { projectPath });
          const newFiles = filesNow.filter((f) => !snapshotBefore!.has(f));
          if (newFiles.length > 0) {
            newFiles.sort();
            trackedJsonl = newFiles[newFiles.length - 1];
            usePtyStore.getState().setJsonlPath(engine as PtyEngine, trackedJsonl);
            const basename = trackedJsonl.split("/").pop() ?? "";
            const claudeSessionId = basename.replace(".jsonl", "");
            if (claudeSessionId && conversationId) {
              invoke("update_resume_token", { conversationId, resumeToken: claudeSessionId }).catch((e) =>
                console.warn("[pty-jsonl] save resume_token:", e)
              );
            }
            console.log(`[pty-jsonl] detected JSONL: ${trackedJsonl} (session: ${claudeSessionId})`);
            activitySeen = true;
            try {
              const blArgs: Record<string, unknown> = engine === "gemini"
                ? { jsonPath: trackedJsonl, afterMessageCount: 0 }
                : { projectPath, afterLine: 0, jsonlPath: trackedJsonl };
              const bl = await invoke<{ totalLines: number; isComplete: boolean; [k: string]: unknown } | null>(
                pollConfig.pollCmd, blArgs
              );
              if (bl) baselineLines = Math.max(0, bl.totalLines - 2);
            } catch { /* ok */ }
          }
        } catch { /* ok */ }
      }

      if (!trackedJsonl) continue;

      try {
        type PtyResult = {
          text: string;
          toolUses: string[];
          toolSteps: { stepType: string; name: string; toolUseId?: string; input?: string; output?: string; status: string }[];
          model: string | null;
          totalLines: number;
          isComplete: boolean;
        };
        const mainPollArgs: Record<string, unknown> = engine === "gemini"
          ? { jsonPath: trackedJsonl, afterMessageCount: baselineLines }
          : { projectPath, afterLine: baselineLines, jsonlPath: trackedJsonl };
        const result = await invoke<PtyResult | null>(pollConfig.pollCmd, mainPollArgs);

        if (result) {
          // Mark JSONL activity seen (resets start timeout concern)
          if (result.totalLines > baselineLines || result.text.length > 0) {
            activitySeen = true;
          }

          // Show tool steps progress during streaming
          if (result.toolSteps.length > 0 && !result.isComplete) {
            hasToolSteps = true;
            const liveSteps = result.toolSteps.map((s) => ({
              type: s.stepType, name: s.name, input: s.input || "", output: s.output || undefined, status: s.status, ts: Date.now(),
            }));
            updateAsstMsg({ progressContent: JSON.stringify(liveSteps) });
          }

          // Complete: final text arrived
          if (result.isComplete && result.text.length > 0) {
            finalized = true;
            ulScreen();
            ulDelivery();
            usePtyStore.getState().endCapture();

            let progressContent: string | undefined;
            if (result.toolSteps.length > 0) {
              const steps = result.toolSteps.map((s) => ({
                type: s.stepType, name: s.name, input: s.input || "", status: s.status, ts: Date.now(),
              }));
              progressContent = JSON.stringify(steps);
            }

            const durationMs = Date.now() - now;
            try {
              const savedMsg = await invoke<Message>("append_assistant_message", {
                input: { conversationId, content: result.text, engine: engineKey, model: result.model, status: "done" },
              });
              if (progressContent) {
                invoke("save_progress_content", { messageId: savedMsg.id, progressContent }).catch(() => {});
              }
              invoke("update_message_status", {
                input: { messageId: savedMsg.id, status: "done", durationMs },
              }).catch(() => {});
              if (opts.isActiveCheck()) {
                set((state: any) => ({
                  [mt]: (state[mt] as Message[]).map((m: Message) =>
                    m.id === asstMsgId
                      ? { ...savedMsg, content: result.text, progressContent, durationMs }
                      : m
                  ),
                }));
              }

              // Post-completion handling
              if (opts.onCompleted) {
                const handled = await opts.onCompleted(savedMsg, result.text);
                if (!handled) get()._endRun(conversationId);
              } else {
                // Default: scan markers + notify
                if (result.text) {
                  import("@/lib/planProposalParser").then(({ extractToolRequests }) => {
                    const requests = extractToolRequests(result.text);
                    if (requests.length > 0) {
                      console.log("[pty] tool-request markers detected:", requests.length);
                    }
                  }).catch(() => {});
                  import("@/stores/notificationStore").then(({ notify }) => {
                    notify("completed", "tunaFlow", "PTY 응답 완료", conversationId);
                  }).catch(() => {});
                }
                get()._endRun(conversationId);
              }
            } catch (err) {
              console.error("[pty-jsonl] DB save failed:", err);
              updateAsstMsg({ content: result.text, model: result.model ?? undefined, status: "done" as const, progressContent, durationMs });
              get()._endRun(conversationId);
            }
            return;
          }
        }
      } catch (e) {
        if (attempt % 75 === 0) console.log("[pty-jsonl] polling...", attempt, String(e).slice(0, 60));
      }
    }

    // Safety net: loop exited without finalization (should not reach here)
    if (!finalized) {
      finalized = true;
      ulScreen();
      ulDelivery();
      usePtyStore.getState().endCapture();
      updateAsstMsg({ content: "(응답 대기 시간 초과)", status: "error" as const });
      get()._endRun(conversationId);
    }
  } catch (err) {
    ulScreen();
    usePtyStore.getState().endCapture();
    set((state: any) => ({
      error: errorMessage(err),
      [mt]: (state[mt] as Message[]).map((m: Message) =>
        m.id === asstMsgId ? { ...m, content: errorMessage(err), status: "error" as const } : m
      ),
    }));
    get()._endRun(conversationId);
  }
}
