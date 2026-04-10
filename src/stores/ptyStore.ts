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

/** Detect response completion */
function detectCompletion(text: string): boolean {
  // Primary: explicit done marker (injected in PTY prompt suffix)
  if (text.includes("TUNAFLOW_DONE")) return true;
  // Secondary: tunaflow HTML marker
  if (text.includes("<!-- tunaflow:response-complete -->")) return true;
  // Fallback: Claude Code specific
  const tail = text.slice(-300);
  if (/Worked for \d+/i.test(tail)) return true;
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
  }),

  appendOutput: (text) => {
    // text is already ANSI-stripped by Rust (pty:text event)
    set((s) => ({ outputBuffer: s.outputBuffer + text }));
    return text;
  },

  checkCompletion: () => detectCompletion(get().outputBuffer),

  endCapture: () => {
    const { outputBuffer } = get();
    set({ activeMessageId: null, activeEngine: null, outputBuffer: "", isCapturing: false });
    return outputBuffer;
  },
}));
