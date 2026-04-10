import { create } from "zustand";

/** Strip ANSI escape sequences from terminal output */
function stripAnsi(text: string): string {
  // eslint-disable-next-line no-control-regex
  return text.replace(/\x1b\[[0-9;]*[A-Za-z]/g, "")
    .replace(/\x1b\][^\x07]*\x07/g, "")   // OSC sequences
    .replace(/\x1b\[\?[0-9;]*[hl]/g, "")   // DEC private modes
    .replace(/\x1b[()][A-Z0-9]/g, "")       // Character set
    .replace(/\x1b=/g, "")                   // Application keypad
    .replace(/\r/g, "");                     // Carriage returns
}

/** Detect Claude Code completion pattern */
function detectCompletion(text: string): boolean {
  // "Worked for Xs" or "> " prompt after output
  return /Worked for \d+/i.test(text) || /\n❯\s*$/.test(text);
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
