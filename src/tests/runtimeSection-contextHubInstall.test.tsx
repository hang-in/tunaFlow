// RuntimeSection ContextHubPanel — manual install trigger (T5 of
// windowsDependencyBootstrapPlan_2026-04-29). Verifies the install
// button is rendered only when context-hub is unavailable, and that
// clicking it dispatches install_dependency.

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";

// react-i18next: identity mock — assertions go against keys.
vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: "ko", changeLanguage: () => Promise.resolve() },
  }),
}));

const mockInvoke = vi.fn<(...a: unknown[]) => unknown>();
const mockListen = vi.fn<() => Promise<() => void>>(() => Promise.resolve(() => {}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...a: unknown[]) => mockInvoke(...a) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: () => mockListen() }));

vi.mock("@/lib/clipboard", () => ({ copyToClipboard: vi.fn() }));
vi.mock("@/lib/skillSets", () => ({ SKILL_SETS: [], expandSkillRefs: () => [] }));
vi.mock("@/lib/appStore", () => ({
  getSetting: vi.fn((_key: string, fallback: unknown) => Promise.resolve(fallback)),
  setSetting: vi.fn(() => Promise.resolve()),
}));

vi.mock("@/stores/chatStore", () => {
  const baseStoreState = {
    rawqStatus: null,
    selectedProjectKey: null,
    engineModels: [],
    loadEngineModels: vi.fn(),
    workflowSkills: {},
    saveWorkflowSkills: vi.fn(),
    setHandoffSource: vi.fn(),
    selectedConversationId: null,
  };
  const useChatStore = Object.assign(
    vi.fn((selector?: (s: typeof baseStoreState) => unknown) =>
      selector ? selector(baseStoreState) : baseStoreState,
    ),
    { getState: () => baseStoreState },
  );
  return { useChatStore };
});

import { RuntimeSection } from "@/components/tunaflow/settings/RuntimeSection";

beforeEach(() => {
  mockInvoke.mockReset();
  mockListen.mockClear();
});

describe("RuntimeSection — context-hub manual install (T5)", () => {
  it("does not render install button when context-hub is ready", async () => {
    mockInvoke.mockImplementation((...args: unknown[]) => {
      if (args[0] === "context_hub_health") {
        return Promise.resolve({ available: true, version: "0.1.4", message: "context-hub 0.1.4 ready" });
      }
      return Promise.resolve(null);
    });
    render(<RuntimeSection />);
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("context_hub_health"),
    );
    expect(screen.queryByTestId("context-hub-install-panel")).toBeNull();
  });

  it("shows install button when context-hub is unavailable and triggers install_dependency on click", async () => {
    mockInvoke.mockImplementation((...args: unknown[]) => {
      const cmd = args[0] as string;
      if (cmd === "context_hub_health") {
        return Promise.resolve({ available: false, version: null, message: "context-hub not installed" });
      }
      if (cmd === "install_dependency") {
        return Promise.resolve({ name: "context-hub", success: true, message: "ok", manualCommand: null });
      }
      return Promise.resolve(null);
    });
    render(<RuntimeSection />);
    await screen.findByTestId("context-hub-install-panel");
    fireEvent.click(screen.getByText("runtime.context_hub_install.button"));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("install_dependency", { name: "context-hub" }),
    );
  });
});
