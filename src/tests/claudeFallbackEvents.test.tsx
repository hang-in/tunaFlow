// ClaudeFallbackEvents — Tauri event listener test.
//
// SSOT:
// - docs/plans/claudeTransportFlipHardeningPlan_2026-04-29.md Task 04 (claude:fresh_fallback)
// - docs/plans/claudeSdkSessionWindowGuardPlan_2026-05-09.md Task 02 (tunaflow:sdk-session-window-rotated)
//
// 본 test 는 PR-2 의 SDK window guard toast listener 가:
//  1. `tunaflow:sdk-session-window-rotated` 이벤트 수신 시 sonner toast.info 호출
//  2. 같은 conversation 의 두 번째 이벤트는 sessionStorage flag 로 spam 차단
//  3. 다른 conversation 은 별 flag 라 toast 표시
// 를 검증한다.

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render } from "@testing-library/react";
import { ClaudeFallbackEvents } from "@/components/tunaflow/ClaudeFallbackEvents";

// react-i18next mock — toast 의 t(key) 가 식별 가능하도록 identity 반환.
vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: "ko", changeLanguage: () => Promise.resolve() },
  }),
}));

// claudeRateLimitStore — listener 가 호출하는 setRateLimit 의 spy.
const setRateLimitSpy = vi.fn();
vi.mock("@/stores/claudeRateLimitStore", () => ({
  useClaudeRateLimitStore: (selector: (s: { setRateLimit: typeof setRateLimitSpy }) => unknown) =>
    selector({ setRateLimit: setRateLimitSpy }),
}));

// sonner — toast.info / toast.error 의 spy.
const toastInfoSpy = vi.fn();
const toastErrorSpy = vi.fn();
vi.mock("sonner", () => ({
  toast: {
    info: (...args: unknown[]) => toastInfoSpy(...args),
    error: (...args: unknown[]) => toastErrorSpy(...args),
  },
}));

// Tauri event listener — 이벤트 별로 callback 등록 + 외부에서 trigger 가능한 spy.
type EventCallback<T> = (event: { payload: T }) => void;
const eventListeners = new Map<string, EventCallback<unknown>>();
vi.mock("@tauri-apps/api/event", () => ({
  listen: <T,>(eventName: string, callback: EventCallback<T>) => {
    eventListeners.set(eventName, callback as EventCallback<unknown>);
    return Promise.resolve(() => {
      eventListeners.delete(eventName);
    });
  },
}));

beforeEach(() => {
  setRateLimitSpy.mockClear();
  toastInfoSpy.mockClear();
  toastErrorSpy.mockClear();
  eventListeners.clear();
  // sessionStorage spam-flag 초기화 — 테스트 간 격리.
  try {
    sessionStorage.clear();
  } catch {
    /* noop */
  }
});

afterEach(() => {
  eventListeners.clear();
});

describe("ClaudeFallbackEvents — SDK window guard listener (Task 02)", () => {
  it("registers a listener for tunaflow:sdk-session-window-rotated", async () => {
    render(<ClaudeFallbackEvents />);
    // useEffect 의 async (async () => {})() 가 listener 를 등록하기까지 대기.
    await new Promise((r) => setTimeout(r, 0));

    expect(eventListeners.has("tunaflow:sdk-session-window-rotated")).toBe(true);
    expect(eventListeners.has("claude:fresh_fallback")).toBe(true);
    expect(eventListeners.has("claude:rate_limit")).toBe(true);
  });

  it("shows sonner toast.info when window-rotated event fires", async () => {
    render(<ClaudeFallbackEvents />);
    await new Promise((r) => setTimeout(r, 0));

    const callback = eventListeners.get("tunaflow:sdk-session-window-rotated")!;
    callback({
      payload: {
        messageId: "msg-1",
        conversationId: "conv-A",
        engine: "claude-code",
        priorTokens: 185_000,
        threshold: 180_000,
      },
    });

    expect(toastInfoSpy).toHaveBeenCalledTimes(1);
    expect(toastInfoSpy).toHaveBeenCalledWith(
      "claude.windowRotated.title",
      expect.objectContaining({
        description: "claude.windowRotated.body",
        duration: 5000,
      }),
    );
  });

  it("dedupes repeat events per conversation via sessionStorage flag", async () => {
    render(<ClaudeFallbackEvents />);
    await new Promise((r) => setTimeout(r, 0));

    const callback = eventListeners.get("tunaflow:sdk-session-window-rotated")!;
    const payload = {
      messageId: "msg-1",
      conversationId: "conv-A",
      engine: "claude-code",
      priorTokens: 185_000,
      threshold: 180_000,
    };

    callback({ payload });
    callback({ payload });
    callback({ payload });

    // 첫 번째만 toast — 같은 conv 의 반복은 spam 차단.
    expect(toastInfoSpy).toHaveBeenCalledTimes(1);
    expect(sessionStorage.getItem("tunaflow.sdkWindowRotatedShown.conv-A")).toBe("1");
  });

  it("treats different conversations as separate spam-flag scopes", async () => {
    render(<ClaudeFallbackEvents />);
    await new Promise((r) => setTimeout(r, 0));

    const callback = eventListeners.get("tunaflow:sdk-session-window-rotated")!;
    callback({
      payload: {
        messageId: "msg-1",
        conversationId: "conv-A",
        engine: "claude-code",
        priorTokens: 185_000,
        threshold: 180_000,
      },
    });
    callback({
      payload: {
        messageId: "msg-2",
        conversationId: "conv-B",
        engine: "claude-code",
        priorTokens: 185_000,
        threshold: 180_000,
      },
    });

    // 다른 conv 는 별 flag → 각각 toast.
    expect(toastInfoSpy).toHaveBeenCalledTimes(2);
  });
});
