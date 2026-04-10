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
    // eslint-disable-next-line no-control-regex
    .replace(/\x1b\[[0-9;]*[A-Za-z]/g, "")
    .replace(/\x1b\[\?[0-9;]*[hl]/g, "")
    .replace(/\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)/g, "")
    .replace(/\x1b[()][A-Z0-9]/g, "")
    .replace(/\x1b[=>]/g, "")
    .replace(/\x1b[78]/g, "")
    // eslint-disable-next-line no-control-regex
    .replace(/[\x00-\x08\x0b\x0c\x0e-\x1f]/g, "")
    .replace(/\r/g, "");
}

/** Detect response completion */
function detectCompletion(text: string): boolean {
  if (text.includes("<!-- tunaflow:response-complete -->")) return true;
  const tail = text.slice(-300);
  if (/Worked for \d+/i.test(tail)) return true;
  if (/[❯>]\s*$/.test(tail)) return true;
  if (/\$\d+\.\d{2}\s*$/.test(tail)) return true;
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

  appendOutput: (rawText) => {
    const stripped = stripAnsi(rawText);
    set((s) => ({ outputBuffer: s.outputBuffer + stripped }));
    return stripped;
  },

  checkCompletion: () => detectCompletion(get().outputBuffer),

  endCapture: () => {
    const { outputBuffer } = get();
    set({ activeMessageId: null, activeEngine: null, outputBuffer: "", isCapturing: false });
    return outputBuffer;
  },
}));
