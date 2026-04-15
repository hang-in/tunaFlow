import { invoke } from "@tauri-apps/api/core";
import { errorMessage } from "@/lib/utils";
import type {
  SetState,
  GetState,
  Conversation,
  Message,
  Branch,
  Memo,
  Artifact,
  CreateConversationInput,
} from "./types";

// ─── PTY session management (chat = session 1:1) ─────────────────────────────

/** Per-conversation spawn locks — prevents concurrent spawns for the same conversation.
 *  Map<conversationId, Promise<void>> — presence indicates spawn in progress. */
const ptySpawnLocks = new Map<string, Promise<void>>();

/** Returns true if PTY is currently being spawned for the given conversation (or any, if no id given). */
export function isPtySpawning(convId?: string) {
  if (convId) return ptySpawnLocks.has(convId);
  return ptySpawnLocks.size > 0;
}

/** Wait for the PTY spawn of the given conversation to complete (or until timeout).
 *  Resolves immediately if no spawn is in progress.
 *  On timeout, resolves silently — caller checks session state after. */
export async function waitForPtyReady(convId: string, timeoutMs = 20_000): Promise<void> {
  const spawnPromise = ptySpawnLocks.get(convId);
  if (!spawnPromise) return;
  await Promise.race([
    spawnPromise,
    new Promise<void>((resolve) => setTimeout(resolve, timeoutMs)),
  ]);
}

/** Spawn a PTY Claude session for the given conversation.
 *  If conversation has a resumeToken, resumes that exact session.
 *  Otherwise starts a new session. */
export async function spawnPtyForConversation(conv: Conversation, projectPath: string) {
  if (ptySpawnLocks.has(conv.id)) return;

  let resolveSpawn!: () => void;
  const spawnPromise = new Promise<void>((res) => { resolveSpawn = res; });
  ptySpawnLocks.set(conv.id, spawnPromise);
  try {
    const { usePtyStore, getPtyBinary, isPtyEngine } = await import("@/stores/ptyStore");
    const { invoke: tauriInvoke } = await import("@tauri-apps/api/core");

    // Determine engine from conversation (default to claude)
    const engine = (conv.engine ?? "claude").replace("claude-code", "claude") as import("@/stores/ptyStore").PtyEngine;
    if (!isPtyEngine(engine)) return; // non-PTY engine (ollama, etc.)

    const binary = getPtyBinary(engine);
    if (!binary) return;

    const pty = usePtyStore.getState();

    // Skip if PTY is already running for this conversation's session
    const existingSession = pty.sessions.get(engine);
    if (existingSession && conv.resumeToken) {
      const existingJsonl = existingSession.jsonlPath ?? "";
      if (existingJsonl.includes(conv.resumeToken)) {
        console.log(`[pty] ${engine} already running for session ${conv.resumeToken}, skipping`);
        return;
      }
    }

    // Kill ALL existing PTY sessions first (ensures clean state)
    await tauriInvoke("pty_kill_all").catch(() => {});
    pty.clearAllSessions();

    // Resolve model for this conversation from per-conversation engine map
    const { useChatStore } = await import("@/stores/chatStore");
    const savedModel = useChatStore.getState().getConversationEngine(conv.id)?.model;

    // Build engine-specific args (including resume)
    const args: string[] = [];
    if (engine === "claude") {
      if (conv.resumeToken) args.push("--resume", conv.resumeToken);
      if (savedModel) args.push("--model", savedModel);
      args.push("--permission-mode", "bypassPermissions");
    } else if (engine === "codex") {
      if (conv.resumeToken) {
        // Codex uses "resume <sessionId>" subcommand pattern
        // But in interactive mode, we pass as args to the binary
        args.push("resume", conv.resumeToken, "--full-auto");
      } else {
        args.push("--full-auto");
      }
    } else if (engine === "gemini") {
      if (conv.resumeToken) args.push("--resume", conv.resumeToken);
      args.push("-y");
    }

    const sessionId = await tauriInvoke<number>("pty_spawn", {
      file: binary, args, cwd: projectPath, cols: 220, rows: 50,
      env: { NO_COLOR: "1" },
    });
    pty.setSession(engine, sessionId, projectPath, savedModel ?? undefined);

    // Wait for CLI to become ready (❯ prompt or response indicator)
    // Claude CLI takes 2-5s to load, especially with --resume
    const { listen } = await import("@tauri-apps/api/event");
    const readyPromise = new Promise<void>((resolve) => {
      const timeout = setTimeout(() => { unlisten(); resolve(); }, 15_000); // 15s max
      let unlisten = () => {};
      listen<{ sessionId: number; data: string }>("pty:screen", (e) => {
        if (e.payload.sessionId !== sessionId) return;
        // ❯ prompt = CLI ready for input
        if (/❯/.test(e.payload.data)) {
          clearTimeout(timeout);
          unlisten();
          console.log(`[pty] ${engine} CLI ready (prompt detected)`);
          resolve();
        }
      }).then((ul) => { unlisten = ul; });
    });
    await readyPromise;

    // Find session file for resume tracking
    if (conv.resumeToken) {
      const listCmd = engine === "claude" ? "pty_list_jsonl_files"
        : engine === "codex" ? "pty_list_codex_files"
        : "pty_list_gemini_files";
      try {
        const files = await tauriInvoke<string[]>(listCmd, { projectPath });
        const match = files.find((f) => f.includes(conv.resumeToken!));
        if (match) {
          pty.setJsonlPath(engine, match);
          console.log(`[pty] ${engine} resumed session ${conv.resumeToken}, file: ${match}`);
          return;
        }
      } catch { /* ok — will detect on first message */ }
    }

    console.log(`[pty] ${engine} new session ${sessionId} for conv ${conv.id}`);

    // Note: CLAUDE.md is NOT dynamically updated — first PTY message includes full ContextPack inline.
    // See knownIssues_2026-04-12.md for why dynamic CLAUDE.md updates were removed.
  } catch (err) {
    console.warn(`[pty] unavailable:`, err);
  } finally {
    ptySpawnLocks.delete(conv.id);
    resolveSpawn();
  }
}

/** Build a lightweight context summary and write to CLAUDE.md ## tunaFlow Context section */
async function updateClaudeMdContext(
  conversationId: string,
  projectPath: string,
  invokeCmd: typeof import("@tauri-apps/api/core").invoke,
) {
  try {
    const contextResult = await invokeCmd<{ assembledPrompt: string; sections: string[] }>(
      "pty_build_context", {
        conversationId,
        prompt: "",
        projectPath,
        activeSkills: [],
        crossSessionIds: [],
        personaFragment: null,
        contextMode: "lite", // Lightweight for CLAUDE.md — just identity + plan
      }
    );
    if (contextResult.assembledPrompt) {
      await invokeCmd("pty_update_claude_md", {
        projectPath,
        contextSection: contextResult.assembledPrompt,
      });
    }
  } catch (e) {
    console.debug("[pty] updateClaudeMdContext:", e);
  }
}

export interface ConversationSlice {
  conversations: Conversation[];
  selectedConversationId: string | null;
  messages: Message[];
  createConversation: (input: CreateConversationInput) => Promise<Conversation>;
  deleteConversation: (id: string) => Promise<void>;
  selectConversation: (id: string) => Promise<void>;
  renameConversation: (id: string, customLabel: string) => Promise<void>;
  deleteMessagePair: (messageId: string) => Promise<void>;
}

export const createConversationSlice = (set: SetState, get: GetState): ConversationSlice => ({
  conversations: [],
  selectedConversationId: null,
  messages: [],

  createConversation: async (input: CreateConversationInput) => {
    const conv = await invoke<Conversation>("create_conversation", { input });
    const projectKey = get().selectedProjectKey;
    if (projectKey) {
      const conversations = await invoke<Conversation[]>("list_conversations", {
        projectKey,
      });
      set({ conversations });
    }
    return conv;
  },

  deleteConversation: async (id: string) => {
    try {
      await invoke("delete_conversation", { id });
      // Optimistic update: filter locally instead of re-fetching list_conversations.
      // Re-fetching triggers ChatPanel re-render → Virtuoso layout recalculation →
      // momentary empty messages + showTyping=true → typing dots appear at wrong position.
      set((state) => ({
        conversations: state.conversations.filter((c) => c.id !== id),
        crossSessionIds: state.crossSessionIds.filter((cid) => cid !== id),
      }));
      // Clear selection if deleted conversation was selected
      if (get().selectedConversationId === id) {
        set({
          selectedConversationId: null,
          messages: [],
          branches: [],
          memos: [],
          artifacts: [],
        });
      }
    } catch (e) {
      set({ error: errorMessage(e) });
    }
  },

  selectConversation: async (id: string) => {
    // Save current conversation's engine state before switching
    const prevConvId = get().selectedConversationId;
    if (prevConvId) {
      // Engine state will be saved by NewMessageInput via saveConversationEngine
      // (already handled on profile/engine change — no action needed here)
    }

    // Close drawer/thread if open — conversation is the primary view
    set({
      selectedConversationId: id,
      messages: [], branches: [], memos: [], artifacts: [],
      threadBranchId: null, threadBranchConvId: null, threadMessages: [],
      threadBranchLabel: null, threadParentMessage: null,
      drawerPinned: false,
    });
    import("@/lib/appStore").then(({ setSetting }) => setSetting("lastConversationId", id)).catch((e) => console.debug("[settings]", e));

    // NOTE: per-conversation engine/model restore is handled by
    // NewMessageInput's restore useEffect (effectiveConvForRestore dependency).
    // Do NOT call selectProfile here — it triggers profile useEffect which
    // races with restore useEffect and overrides the saved model.

    try {
      const [messages, branches, memos, artifacts] = await Promise.all([
        invoke<Message[]>("list_messages", { conversationId: id }),
        invoke<Branch[]>("list_branches", { conversationId: id }),
        invoke<Memo[]>("list_memos_by_conversation", { conversationId: id }),
        invoke<Artifact[]>("list_artifacts", { conversationId: id }),
      ]);
      // Clear stale mark if it was set (agent completed while user was away)
      const stale = get()._staleConversations;
      if (stale?.has(id)) {
        const next = new Set(stale);
        next.delete(id);
        set({ messages, branches, memos, artifacts, error: null, _staleConversations: next });
      } else {
        set({ messages, branches, memos, artifacts, error: null });
      }

      // PTY auto-spawn on conversation select is disabled. PTY is reserved for
      // the interactive terminal panel (user triggers spawn there explicitly).
      // Main chat send uses SDK (if API key) or `-p` CLI mode.
    } catch (e) {
      set({ error: errorMessage(e) });
    }
  },

  renameConversation: async (id: string, customLabel: string) => {
    const trimmed = customLabel.trim() || undefined;
    set((state) => ({
      conversations: state.conversations.map((c) =>
        c.id === id ? { ...c, customLabel: trimmed } : c
      ),
    }));
    try {
      await invoke("rename_conversation", { id, customLabel });
    } catch (e) {
      set({ error: errorMessage(e) });
    }
  },

  deleteMessagePair: async (messageId: string) => {
    const convId = get().selectedConversationId;
    if (!convId) return;
    try {
      await invoke("delete_message_pair", { messageId });
      const messages = await invoke<Message[]>("list_messages", { conversationId: convId });
      set({ messages });
    } catch (e) {
      set({ error: errorMessage(e) });
    }
  },
});
