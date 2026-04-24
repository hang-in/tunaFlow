import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { ManualVerificationGate } from "./ManualVerificationGate";
import type { ManualVerificationItem } from "@/lib/manualVerification";

const items: ManualVerificationItem[] = [
  { label: "Open Settings and verify toggle", source: "developer" },
  { label: "Click New Project button", source: "developer" },
];

describe("ManualVerificationGate", () => {
  it("renders each item label", () => {
    render(
      <ManualVerificationGate
        open
        items={items}
        onComplete={vi.fn()}
        onCancel={vi.fn()}
      />
    );
    expect(screen.getByText(items[0].label)).toBeInTheDocument();
    expect(screen.getByText(items[1].label)).toBeInTheDocument();
  });

  it("disables the 진행 button until every item has a status selected", () => {
    render(
      <ManualVerificationGate
        open
        items={items}
        onComplete={vi.fn()}
        onCancel={vi.fn()}
      />
    );
    const submit = screen.getByRole("button", { name: /진행/ });
    expect(submit).toBeDisabled();
    // pick status for both items
    const passButtons = screen.getAllByRole("button", { name: "Pass" });
    fireEvent.click(passButtons[0]);
    fireEvent.click(passButtons[1]);
    expect(submit).not.toBeDisabled();
  });

  it("shows a reason textarea only when Fail is selected", () => {
    render(
      <ManualVerificationGate
        open
        items={items}
        onComplete={vi.fn()}
        onCancel={vi.fn()}
      />
    );
    expect(screen.queryByPlaceholderText(/실패 사유/)).not.toBeInTheDocument();
    const failButtons = screen.getAllByRole("button", { name: "Fail" });
    fireEvent.click(failButtons[0]);
    expect(screen.getByPlaceholderText(/실패 사유/)).toBeInTheDocument();
  });

  it("모두 Pass button marks every item as pass and enables submit", () => {
    render(
      <ManualVerificationGate
        open
        items={items}
        onComplete={vi.fn()}
        onCancel={vi.fn()}
      />
    );
    fireEvent.click(screen.getByRole("button", { name: "모두 Pass" }));
    expect(screen.getByRole("button", { name: /진행/ })).not.toBeDisabled();
  });

  it("submits results in items order with optional fail reasons", () => {
    const onComplete = vi.fn();
    render(
      <ManualVerificationGate
        open
        items={items}
        onComplete={onComplete}
        onCancel={vi.fn()}
      />
    );
    const passButtons = screen.getAllByRole("button", { name: "Pass" });
    const failButtons = screen.getAllByRole("button", { name: "Fail" });
    fireEvent.click(passButtons[0]);
    fireEvent.click(failButtons[1]);
    const textarea = screen.getByPlaceholderText(/실패 사유/);
    fireEvent.change(textarea, { target: { value: "button did not respond" } });
    fireEvent.click(screen.getByRole("button", { name: /진행/ }));
    expect(onComplete).toHaveBeenCalledTimes(1);
    expect(onComplete.mock.calls[0][0]).toEqual([
      { status: "pass", reason: undefined },
      { status: "fail", reason: "button did not respond" },
    ]);
  });
});
