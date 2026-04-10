import "@testing-library/jest-dom/vitest";
import { vi } from "vitest";

// Mock Tauri IPC — all invoke calls return empty by default
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve()),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(() => Promise.resolve()),
}));

// Mock ptyStore (PTY session never active in tests)
vi.mock("@/stores/ptyStore", () => ({
  usePtyStore: { getState: () => ({ sessionId: null, projectPath: null, isCapturing: false, outputBuffer: "", startCapture: vi.fn(), appendOutput: vi.fn(() => ""), checkCompletion: vi.fn(() => false), endCapture: vi.fn(() => ""), setSession: vi.fn(), clearSession: vi.fn() }) },
}));

// Mock xterm.js (DOM-dependent, not available in jsdom)
vi.mock("@xterm/xterm", () => ({
  Terminal: vi.fn(() => ({
    open: vi.fn(),
    write: vi.fn(),
    clear: vi.fn(),
    dispose: vi.fn(),
    onData: vi.fn(),
    onResize: vi.fn(),
    loadAddon: vi.fn(),
    cols: 80,
    rows: 24,
  })),
}));

vi.mock("@xterm/addon-fit", () => ({
  FitAddon: vi.fn(() => ({ fit: vi.fn() })),
}));

vi.mock("@xterm/addon-unicode11", () => ({
  Unicode11Addon: vi.fn(() => ({})),
}));

vi.mock("@xterm/addon-web-links", () => ({
  WebLinksAddon: vi.fn(() => ({})),
}));
