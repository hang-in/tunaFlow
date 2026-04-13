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
export type { PtySendOptions } from "./ptyTypes";
export { getPtyPollConfig } from "./ptyTypes";
import type { PtySendOptions } from "./ptyTypes";
import { getPtyPollConfig } from "./ptyTypes";

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
      { id: asstMsgId, conversationId, role: "assistant" as const, content: "", timestamp: now, status: "streaming" as const, engine: engineKey, persona: opts.personaLabel },
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
      const { getSetting } = await import("@/lib/appStore");
      const userProfile = await getSetting("userProfile", null as unknown as object).catch(() => null);
      const contextResult = await invoke<{ assembledPrompt: string; sections: string[] }>(
        "pty_build_context", {
          conversationId, prompt, projectPath: projectPath || null,
          activeSkills: activeSkills ?? [], crossSessionIds: crossSessionIds ?? [],
          personaFragment: null, contextMode: null,
          userProfileJson: userProfile ? JSON.stringify(userProfile) : null,
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
  // No hard TOTAL_TIMEOUT: we keep waiting as long as the PTY process is alive.
  // The loop checks pty_is_alive every 30s; if the process exits, we finalize.
  const POLL_INTERVAL_MS = 200;

  // Send prompt via bracket paste + Enter
  try {
    // ── Pre-write: ensure CLI is ready (❯ prompt visible) ──
    const screen = await invoke<string>("pty_get_screen", { sessionId }).catch(() => "");
    // Wait for ❯ prompt if: screen is empty (CLI still initializing) OR screen has no ❯ yet.
    // Previously checked `screen && !/❯/` which skipped the wait when screen was empty —
    // causing bracket paste to arrive before Claude CLI was ready to accept input.
    if (!/❯/.test(screen)) {
      console.log("[pty] CLI not ready (screen empty or no ❯), waiting...");
      if (screen) {
        // Screen has content but no ❯ — send Enter to wake it
        await invoke("pty_write", { sessionId, data: "\r" });
      }
      // Wait for ❯ prompt to appear (max 10s)
      await new Promise<void>((resolve) => {
        let resolved = false;
        let ul: () => void = () => {};
        const timeout = setTimeout(() => {
          if (!resolved) { resolved = true; ul(); resolve(); }
        }, 10_000);
        listenEvent<{ sessionId: number; data: string }>("pty:screen", (e) => {
          if (!resolved && e.payload.sessionId === sessionId && /❯/.test(e.payload.data)) {
            resolved = true;
            clearTimeout(timeout); ul(); resolve();
          }
        }).then((u) => { ul = u; });
      });
    }

    await invoke("pty_write", { sessionId, data: `\x1b[200~${enrichedPrompt}\x1b[201~` });
    // Scale delay with prompt length — gives Claude CLI time to process bracket paste buffer
    // Formula: ~1ms per 100 chars, min 300ms, max 1500ms
    // Large prompts (ContextPack ~50KB+) need more headroom for rendering + CLI readline flush
    const pasteDelayMs = Math.max(300, Math.min(1500, Math.ceil(enrichedPrompt.length / 100)));
    await new Promise((r) => setTimeout(r, pasteDelayMs));
    await invoke("pty_write", { sessionId, data: "\r" }).catch((e) => {
      console.error("[pty] Enter write failed:", e);
    });

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

      // ── Process alive check: every 30s verify PTY is still running ──
      if (elapsed > 0 && attempt % Math.round(30_000 / POLL_INTERVAL_MS) === 0) {
        const alive = await invoke<boolean>("pty_is_alive", { sessionId }).catch(() => false);
        if (!alive) {
          finalized = true;
          ulScreen();
          ulDelivery();
          usePtyStore.getState().endCapture();
          console.error("[pty] process exited unexpectedly");
          updateAsstMsg({
            content: "(PTY 프로세스가 종료되었습니다 — 에이전트가 예기치 않게 종료됨)",
            status: "error" as const,
          });
          get()._endRun(conversationId);
          return;
        }
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
                input: { conversationId, content: result.text, engine: engineKey, model: result.model, status: "done", personaLabel: opts.personaLabel },
              });
              if (progressContent) {
                invoke("save_progress_content", { messageId: savedMsg.id, progressContent }).catch(() => {});
              }
              invoke("update_message_status", {
                input: { messageId: savedMsg.id, status: "done", durationMs },
              }).catch((e) => console.debug("[pty] update_message_status failed:", e));
              if (opts.isActiveCheck()) {
                const currentMsgs = (get() as any)[mt] as Message[];
                const stillPresent = currentMsgs.some((m: Message) => m.id === asstMsgId);
                if (stillPresent) {
                  set((state: any) => ({
                    [mt]: (state[mt] as Message[]).map((m: Message) =>
                      m.id === asstMsgId
                        ? { ...savedMsg, content: result.text, progressContent, durationMs }
                        : m
                    ),
                  }));
                } else {
                  // asstMsgId was removed (e.g. by adoptBranch reload) — reload from DB to surface completed result
                  console.log("[pty] asstMsgId not found in store, reloading from DB after completion");
                  try {
                    const refreshed = await invoke<Message[]>("list_messages", { conversationId });
                    const withMeta = refreshed.map((m: Message) =>
                      m.id === savedMsg.id ? { ...m, durationMs, progressContent } : m
                    );
                    set((state: any) => ({ [mt]: withMeta }));
                  } catch (refreshErr) {
                    console.warn("[pty] DB reload after adoption failed:", refreshErr);
                  }
                }
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
                  // Notification handled by _endRun to avoid duplicate
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
