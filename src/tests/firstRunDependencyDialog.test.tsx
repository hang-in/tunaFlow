// FirstRunDependencyDialog — first-run consent dialog test (T4 of
// windowsDependencyBootstrapPlan_2026-04-29). Validates: setting gate
// suppresses the dialog, missing-deps render, "건너뛰기" persists the
// done flag, "설치" invokes install_dependency per selected item.

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { FirstRunDependencyDialog, FIRST_RUN_SETTING_KEY } from "@/components/tunaflow/FirstRunDependencyDialog";

// react-i18next: identity mock so we assert on key strings.
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

const mockGetSetting = vi.fn<(...a: unknown[]) => unknown>();
const mockSetSetting = vi.fn<(...a: unknown[]) => Promise<void>>(() => Promise.resolve());
vi.mock("@/lib/appStore", () => ({
  getSetting: (...a: unknown[]) => mockGetSetting(...a),
  setSetting: (...a: unknown[]) => mockSetSetting(...a),
}));

beforeEach(() => {
  mockInvoke.mockReset();
  mockListen.mockClear();
  mockGetSetting.mockReset();
  mockSetSetting.mockClear();
});

describe("FirstRunDependencyDialog", () => {
  it("renders nothing when first_run_dependency_check_done is true (debug toggle)", async () => {
    mockGetSetting.mockResolvedValue(true);
    const { container } = render(<FirstRunDependencyDialog />);
    // Wait long enough for the effect to settle without state changes.
    await new Promise((r) => setTimeout(r, 0));
    expect(container.querySelector('[data-testid="first-run-deps-dialog"]')).toBeNull();
    expect(mockInvoke).not.toHaveBeenCalled();
  });

  it("renders nothing and marks done when no dependencies are missing", async () => {
    mockGetSetting.mockResolvedValue(false);
    mockInvoke.mockResolvedValue([
      { name: "context-hub", available: true, installerCommand: "npm install -g @aisuite/chub", requires: "Node.js + npm", version: "0.1.4" },
      { name: "code-review-graph", available: true, installerCommand: "pip install code-review-graph", requires: "Python 3 + pip", version: null },
    ]);
    render(<FirstRunDependencyDialog />);
    await waitFor(() => expect(mockSetSetting).toHaveBeenCalledWith(FIRST_RUN_SETTING_KEY, true));
    expect(screen.queryByTestId("first-run-deps-dialog")).toBeNull();
  });

  it("shows the dialog with cards for each missing dependency", async () => {
    mockGetSetting.mockResolvedValue(false);
    mockInvoke.mockResolvedValue([
      { name: "context-hub", available: false, installerCommand: "npm install -g @aisuite/chub", requires: "Node.js + npm", version: null },
      { name: "code-review-graph", available: false, installerCommand: "pip install code-review-graph", requires: "Python 3 + pip", version: null },
    ]);
    render(<FirstRunDependencyDialog />);
    await screen.findByTestId("first-run-deps-dialog");
    expect(screen.getByTestId("dep-card-context-hub")).toBeInTheDocument();
    expect(screen.getByTestId("dep-card-code-review-graph")).toBeInTheDocument();
    // checkboxes are pre-selected for missing deps
    const checkboxes = screen.getAllByRole("checkbox") as HTMLInputElement[];
    expect(checkboxes).toHaveLength(2);
    expect(checkboxes.every((c) => c.checked)).toBe(true);
  });

  it("clicking 건너뛰기 (skip) persists the done flag and unmounts the dialog", async () => {
    mockGetSetting.mockResolvedValue(false);
    mockInvoke.mockResolvedValue([
      { name: "context-hub", available: false, installerCommand: "npm install -g @aisuite/chub", requires: "Node.js + npm", version: null },
    ]);
    render(<FirstRunDependencyDialog />);
    await screen.findByTestId("first-run-deps-dialog");
    fireEvent.click(screen.getByText("dependency_install.skip"));
    await waitFor(() => expect(mockSetSetting).toHaveBeenCalledWith(FIRST_RUN_SETTING_KEY, true));
    await waitFor(() => expect(screen.queryByTestId("first-run-deps-dialog")).toBeNull());
  });

  it("clicking 설치 invokes install_dependency once per selected item", async () => {
    mockGetSetting.mockResolvedValue(false);
    mockInvoke.mockImplementation((...args: unknown[]) => {
      const cmd = args[0] as string;
      if (cmd === "list_dependencies") {
        return Promise.resolve([
          { name: "context-hub", available: false, installerCommand: "npm install -g @aisuite/chub", requires: "Node.js + npm", version: null },
          { name: "code-review-graph", available: false, installerCommand: "pip install code-review-graph", requires: "Python 3 + pip", version: null },
        ]);
      }
      if (cmd === "install_dependency") {
        return Promise.resolve({ name: "context-hub", success: true, message: "ok", manualCommand: null });
      }
      return Promise.resolve(null);
    });
    render(<FirstRunDependencyDialog />);
    await screen.findByTestId("first-run-deps-dialog");
    // Uncheck crg → only context-hub installs.
    const crgCheckbox = screen.getByLabelText("code-review-graph") as HTMLInputElement;
    fireEvent.click(crgCheckbox);
    expect(crgCheckbox.checked).toBe(false);

    fireEvent.click(screen.getByText("dependency_install.install"));
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("install_dependency", { name: "context-hub" })
    );
    // Only the context-hub install was triggered.
    const installCalls = mockInvoke.mock.calls.filter((c) => c[0] === "install_dependency");
    expect(installCalls).toHaveLength(1);
  });
});
