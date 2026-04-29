// WindowControls — Windows custom window controls (T-WT-2 of
// windowsTitlebarUnificationPlan_2026-04-29). Verifies the three buttons
// render with proper ARIA labels, dispatch the right Tauri window APIs,
// and toggle the maximize/restore icon based on `isMaximized`.

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";

const minimize = vi.fn(() => Promise.resolve());
const toggleMaximize = vi.fn(() => Promise.resolve());
const close = vi.fn(() => Promise.resolve());
const isMaximized = vi.fn(() => Promise.resolve(false));
const onResized = vi.fn(() => Promise.resolve(() => {}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    minimize,
    toggleMaximize,
    close,
    isMaximized,
    onResized,
  }),
}));

import { WindowControls } from "@/components/tunaflow/WindowControls";

beforeEach(() => {
  minimize.mockClear();
  toggleMaximize.mockClear();
  close.mockClear();
  isMaximized.mockClear().mockResolvedValue(false);
  onResized.mockClear().mockResolvedValue(() => {});
});

describe("WindowControls", () => {
  it("renders Min / Max / Close buttons with correct ARIA labels", () => {
    render(<WindowControls />);
    expect(screen.getByRole("button", { name: "Minimize" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Maximize" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Close" })).toBeInTheDocument();
  });

  it("dispatches minimize() when Minimize is clicked", async () => {
    render(<WindowControls />);
    fireEvent.click(screen.getByRole("button", { name: "Minimize" }));
    await waitFor(() => expect(minimize).toHaveBeenCalledTimes(1));
  });

  it("dispatches toggleMaximize() when Maximize is clicked", async () => {
    render(<WindowControls />);
    fireEvent.click(screen.getByRole("button", { name: "Maximize" }));
    await waitFor(() => expect(toggleMaximize).toHaveBeenCalledTimes(1));
  });

  it("dispatches close() when Close is clicked", async () => {
    render(<WindowControls />);
    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    await waitFor(() => expect(close).toHaveBeenCalledTimes(1));
  });

  it("renders Restore label when initially maximized", async () => {
    isMaximized.mockResolvedValue(true);
    render(<WindowControls />);
    await screen.findByRole("button", { name: "Restore" });
    expect(screen.queryByRole("button", { name: "Maximize" })).toBeNull();
  });

  it("opts out of drag region", () => {
    render(<WindowControls />);
    const root = screen.getByTestId("window-controls");
    expect(root.getAttribute("data-tauri-drag-region")).toBe("false");
  });
});
