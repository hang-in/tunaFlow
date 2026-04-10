import { create } from "zustand";

/** Strip ANSI escape sequences and terminal control codes from output */
function stripAnsi(text: string): string {
  return text
    // CSI sequences: \x1b[...X (colors, cursor, erase, scroll, etc.)
    // eslint-disable-next-line no-control-regex
    .replace(/\x1b\[[0-9;]*[A-Za-z]/g, "")
    // DEC private modes: \x1b[?...h/l
    .replace(/\x1b\[\?[0-9;]*[hl]/g, "")
    // OSC sequences: \x1b]...BEL
    .replace(/\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)/g, "")
    // Character set designation: \x1b(X, \x1b)X
    .replace(/\x1b[()][A-Z0-9]/g, "")
    // Application mode: \x1b=, \x1b>
    .replace(/\x1b[=>]/g, "")
    // DEC save/restore cursor: \x1b7, \x1b8
    .replace(/\x1b[78]/g, "")
    // Single-char C1 controls
    // eslint-disable-next-line no-control-regex
    .replace(/[\x00-\x08\x0b\x0c\x0e-\x1f]/g, "")
    // Carriage return (keep \n)
    .replace(/\r/g, "");
}

/** Detect Claude Code completion pattern */
function detectCompletion(text: string): boolean {
  // Check last ~200 chars for completion signals
  const tail = text.slice(-200);
  // "Worked for Xs" — primary signal
  if (/Worked for \d+/i.test(tail)) return true;
  // Prompt ready: ❯ or > at end of output
  if (/[❯>]\s*$/.test(tail)) return true;
  // Cost line: "$X.XX" at end (Claude shows cost after completion)
  if (/\$\d+\.\d{2}\s*$/.test(tail)) return true;
  return false;
}

interface PtyStoreState {
  sessionId: number | null;
  projectPath: string | null;

  // Active message tracking (when sendViaPty is in progress)
  activeMessageId: string | null;
  outputBuffer: string;
  isCapturing: boolean;

  setSession: (id: number, path: string) => void;
  clearSession: () => void;

  startCapture: (messageId: string) => void;
  appendOutput: (rawText: string) => string; // returns ANSI-stripped text
  checkCompletion: () => boolean;
  endCapture: () => string; // returns accumulated text
}

export const usePtyStore = create<PtyStoreState>((set, get) => ({
  sessionId: null,
  projectPath: null,
  activeMessageId: null,
  outputBuffer: "",
  isCapturing: false,

  setSession: (id, path) => set({ sessionId: id, projectPath: path }),
  clearSession: () => set({ sessionId: null, projectPath: null }),

  startCapture: (messageId) => set({
    activeMessageId: messageId,
    outputBuffer: "",
    isCapturing: true,
  }),

  appendOutput: (rawText) => {
    const stripped = stripAnsi(rawText);
    set((s) => ({ outputBuffer: s.outputBuffer + stripped }));
    return stripped;
  },

  checkCompletion: () => {
    const { outputBuffer } = get();
    return detectCompletion(outputBuffer);
  },

  endCapture: () => {
    const { outputBuffer } = get();
    set({ activeMessageId: null, outputBuffer: "", isCapturing: false });
    return outputBuffer;
  },
}));
