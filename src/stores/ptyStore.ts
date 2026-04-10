import { create } from "zustand";

/** CLI engines that support PTY interactive mode */
export const PTY_ENGINES = ["claude", "codex", "gemini"] as const;
export type PtyEngine = typeof PTY_ENGINES[number];

/** Map engine key to CLI binary name */
const ENGINE_BINARY: Record<PtyEngine, string> = {
  claude: "claude",
  codex: "codex",
  gemini: "gemini",
};

export function getPtyBinary(engine: string): string | null {
  return ENGINE_BINARY[engine as PtyEngine] ?? null;
}

export function isPtyEngine(engine: string): engine is PtyEngine {
  return PTY_ENGINES.includes(engine as PtyEngine);
}

/** Strip ANSI escape sequences and terminal control codes from output */
function stripAnsi(text: string): string {
  return text
    // CSI sequences: \x1b[...X (colors, cursor, erase, scroll, etc.)
    // eslint-disable-next-line no-control-regex
    .replace(/\x1b\[[0-9;?]*[A-Za-z]/g, "")
    // Bracket paste, kitty keyboard protocol: \x1b[<...u, \x1b[>...m, etc.
    .replace(/\x1b\[[<>][0-9;]*[A-Za-z]/g, "")
    // OSC sequences: \x1b]...BEL or \x1b]...\x1b\\
    .replace(/\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)/g, "")
    // Character set designation: \x1b(X, \x1b)X
    .replace(/\x1b[()][A-Z0-9]/g, "")
    // Application mode, keypad: \x1b=, \x1b>
    .replace(/\x1b[=>]/g, "")
    // DEC save/restore cursor: \x1b7, \x1b8
    .replace(/\x1b[78]/g, "")
    // Any remaining ESC + single char
    .replace(/\x1b[^\[]/g, "")
    // C0 control characters (except \n and \t)
    // eslint-disable-next-line no-control-regex
    .replace(/[\x00-\x08\x0b\x0c\x0e-\x1f]/g, "")
    // Carriage return
    .replace(/\r/g, "")
    // Box-drawing borders and TUI chrome (━╭╮╰╯│─┌┐└┘├┤┬┴┼)
    .replace(/[━╭╮╰╯│─┌┐└┘├┤┬┴┼╶╴╷╵]+/g, "")
    // UI hint text from Claude Code TUI
    .replace(/ctrl\+[a-z]\s*to\s*\w+.*$/gm, "")
    // Collapse excessive whitespace
    .replace(/[ \t]{3,}/g, " ")
    .replace(/\n{3,}/g, "\n\n");
}

/** Detect response completion from VTE screen snapshot.
 * Complete when: screen has ⏺ (response) AND ends with bare ❯ (new prompt = idle).
 */
function detectCompletion(text: string): boolean {
  const hasResponse = /⏺/.test(text);
  if (!hasResponse) return false;

  // Screen has response (⏺). Check if Claude is idle (bare ❯ prompt visible).
  // Scan bottom lines, skipping TUI chrome (status bar, separators).
  const lines = text.split("\n").map((l) => l.trim());
  // Check last ~8 lines for bare ❯ (skip status bar, separators, empty lines)
  const bottom = lines.slice(-8);
  for (const line of bottom) {
    if (/^❯\s*$/.test(line)) return true;
    if (/Worked for \d+/i.test(line)) return true;
  }
  return false;
}

interface PtySession {
  sessionId: number;
  engine: PtyEngine;
  projectPath: string;
}

interface PtyStoreState {
  /** Active PTY sessions by engine */
  sessions: Map<PtyEngine, PtySession>;

  /** Active message capture state */
  activeMessageId: string | null;
  activeEngine: PtyEngine | null;
  outputBuffer: string;
  isCapturing: boolean;
  completionSeen: boolean;
  responseStarted: boolean; // true after ⏺ is seen (response began)

  /** Get session ID for an engine */
  getSession: (engine: string) => number | null;
  setSession: (engine: PtyEngine, sessionId: number, projectPath: string) => void;
  clearSession: (engine: PtyEngine) => void;
  clearAllSessions: () => void;

  startCapture: (messageId: string, engine: PtyEngine) => void;
  appendOutput: (rawText: string) => string;
  checkCompletion: () => boolean;
  endCapture: () => string;
}

export const usePtyStore = create<PtyStoreState>((set, get) => ({
  sessions: new Map(),
  activeMessageId: null,
  activeEngine: null,
  outputBuffer: "",
  isCapturing: false,
  completionSeen: false,
  responseStarted: false,

  getSession: (engine) => {
    const session = get().sessions.get(engine as PtyEngine);
    return session?.sessionId ?? null;
  },

  setSession: (engine, sessionId, projectPath) => set((s) => {
    const next = new Map(s.sessions);
    next.set(engine, { sessionId, engine, projectPath });
    return { sessions: next };
  }),

  clearSession: (engine) => set((s) => {
    const next = new Map(s.sessions);
    next.delete(engine);
    return { sessions: next };
  }),

  clearAllSessions: () => set({ sessions: new Map() }),

  startCapture: (messageId, engine) => set({
    activeMessageId: messageId,
    activeEngine: engine,
    outputBuffer: "",
    isCapturing: true,
    completionSeen: false,
    responseStarted: false,
  }),

  appendOutput: (text) => {
    // VTE screen snapshot — REPLACE, don't append
    // Track response start (⏺ marker = Claude began responding)
    const hasResponseStart = /⏺/.test(text);
    const wasStarted = get().responseStarted;
    const nowStarted = wasStarted || hasResponseStart;

    // Only detect completion AFTER response has started (ignore prompt echo)
    if (nowStarted && detectCompletion(text)) {
      console.log("[pty-capture] completion detected! responseStarted:", nowStarted, "has⏺:", hasResponseStart);
      set({ outputBuffer: text, completionSeen: true, responseStarted: true });
    } else if (!get().completionSeen) {
      set({ outputBuffer: text, responseStarted: nowStarted });
    }
    return text;
  },

  checkCompletion: () => get().completionSeen,

  endCapture: () => {
    const { outputBuffer } = get();
    set({ activeMessageId: null, activeEngine: null, outputBuffer: "", isCapturing: false, completionSeen: false, responseStarted: false });
    return outputBuffer;
  },
}));
