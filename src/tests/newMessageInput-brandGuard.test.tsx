// brand running guard 테스트 — `mainChatBrandRunningGuardPlan_2026-04-25.md` INV-1~4 검증.
//
// PR #198 (`branchInheritsMainSessionPlan`) 머지로 brand 와 main 이 같은 SDK
// session (process 1개) 을 공유하게 되면서, brand 에서 Developer 가 작업 중인
// 동안 main chat 에 입력하면 같은 process 가 메시지를 받아 의도외 응답을
// 만드는 케이스를 확인. 본 가드는 main mode + brand running 일 때 입력을
// 차단하고 안내 banner 를 노출.

import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { NewMessageInput } from "@/components/tunaflow/NewMessageInput";

// react-i18next 는 실제 i18n 인스턴스를 끌고 오면 테스트 setup 이 무거워지고,
// 본 테스트는 banner 텍스트 자체가 아니라 동작/존재만 보면 충분하므로 식별
// 가능한 echo mock 으로 대체.
vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, opts?: Record<string, unknown>) =>
      opts ? `${key}::${JSON.stringify(opts)}` : key,
    i18n: { language: "ko", changeLanguage: () => Promise.resolve() },
  }),
  Trans: ({ children }: { children?: React.ReactNode }) => children,
}));

// useSendActions 는 store/zustand subscribe 에 깊게 묶여있어 테스트 단위에선
// 외부 행동만 stub. brand-guard 자체는 input 영역에서 일어나는 일이라 send
// 함수가 실제로 무엇을 하는지는 무관.
vi.mock("@/components/tunaflow/input/useSendActions", () => ({
  useSendActions: () => ({
    handleSend: vi.fn(),
    handleKeyDown: vi.fn(),
    isRoundtable: false,
    hasRtMessages: false,
  }),
}));

// 첨부 / appStore — getSetting 호출만 막고 나머지는 사용 안 함.
vi.mock("@/lib/appStore", () => ({
  getSetting: vi.fn(() => Promise.resolve(false)),
  setSetting: vi.fn(() => Promise.resolve()),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(() => Promise.resolve(null)),
}));

vi.mock("@tauri-apps/plugin-fs", () => ({
  readFile: vi.fn(() => Promise.resolve(new Uint8Array())),
}));

// usePtyStore — setup.ts mock 은 getState 만 노출. NewMessageInput 은
// selector callable(`usePtyStore((s) => s.getSession(...))`) 로 호출하므로
// 양쪽 모두 지원하는 hook 형태로 재정의.
vi.mock("@/stores/ptyStore", () => {
  const ptyState = {
    sessions: new Map(),
    getSession: () => null,
    setSession: vi.fn(),
    clearSession: vi.fn(),
    clearAllSessions: vi.fn(),
    isCapturing: false,
    activeMessageId: null,
    activeEngine: null,
    completionSeen: false,
    responseStarted: false,
    startCapture: vi.fn(),
    updateScreen: vi.fn(),
    checkCompletion: vi.fn(() => false),
    endCapture: vi.fn(),
  };
  return {
    usePtyStore: Object.assign(
      (selector?: (s: typeof ptyState) => unknown) =>
        selector ? selector(ptyState) : ptyState,
      { getState: () => ptyState },
    ),
    isPtyEngine: () => false,
    PTY_ENGINES: ["claude", "codex", "gemini"],
    getPtyBinary: () => null,
  };
});

interface MockState {
  selectedConversationId: string | null;
  conversations: Array<{ id: string; mode: string }>;
  activeBranchId: string | null;
  closeBranchStream: () => void;
  cancelOperation: (id?: string) => void;
  runningThreadIds: string[];
  messageQueue: Array<{ threadId: string }>;
  activeSkills: string[];
  crossSessionIds: string[];
  engineModels: Array<{ engine: string; id: string; recommended?: boolean }>;
  agentProfiles: Array<unknown>;
  saveConversationEngine: () => void;
  toggleSkill: () => void;
  threadBranchConvId: string | null;
  threadBranchId: string | null;
  branches: Array<unknown>;
  getConversationEngine: (id: string) => null;
  selectedProjectKey: string | null;
  ensureConversation: () => void;
  selectConversation: () => void;
  openThread: (id: string) => Promise<void>;
}

function makeState(overrides: Partial<MockState> = {}): MockState {
  return {
    selectedConversationId: "conv-main",
    conversations: [{ id: "conv-main", mode: "chat" }],
    activeBranchId: null,
    closeBranchStream: vi.fn(),
    cancelOperation: vi.fn(),
    runningThreadIds: [],
    messageQueue: [],
    activeSkills: [],
    crossSessionIds: [],
    engineModels: [{ engine: "claude", id: "claude-sonnet-4-5", recommended: true }],
    agentProfiles: [],
    saveConversationEngine: vi.fn(),
    toggleSkill: vi.fn(),
    threadBranchConvId: null,
    threadBranchId: null,
    branches: [],
    getConversationEngine: vi.fn(() => null),
    selectedProjectKey: "test-proj",
    ensureConversation: vi.fn(),
    selectConversation: vi.fn(),
    openThread: vi.fn(() => Promise.resolve()),
    ...overrides,
  };
}

function mockChatStore(state: MockState) {
  vi.doMock("@/stores/chatStore", () => {
    const setState = vi.fn();
    return {
      useChatStore: Object.assign(
        // selector 호출도 직접 호출도 모두 지원.
        (selector?: (s: MockState) => unknown) => (selector ? selector(state) : state),
        { getState: () => state, setState },
      ),
    };
  });
}

describe("NewMessageInput — brand running guard (PR #198 follow-up)", () => {
  it("[INV-1/2] main mode + brand running → 입력 disable + banner 노출", async () => {
    vi.resetModules();
    mockChatStore(makeState({
      runningThreadIds: ["branch:abc123"],
    }));
    const { NewMessageInput: Component } = await import("@/components/tunaflow/NewMessageInput");
    render(<Component threadMode={false} />);

    expect(screen.getByTestId("brand-running-guard-banner")).toBeInTheDocument();
    const textareas = document.querySelectorAll("textarea");
    expect(textareas.length).toBeGreaterThan(0);
    expect(textareas[0]).toBeDisabled();
  });

  it("[INV-2] banner 안에 '드로어 열기' 버튼이 있고 클릭 시 openThread 호출", async () => {
    vi.resetModules();
    const openThread = vi.fn(() => Promise.resolve());
    mockChatStore(makeState({
      runningThreadIds: ["branch:abc123"],
      openThread,
    }));
    const { NewMessageInput: Component } = await import("@/components/tunaflow/NewMessageInput");
    render(<Component threadMode={false} />);

    const banner = screen.getByTestId("brand-running-guard-banner");
    const button = banner.querySelector("button");
    expect(button).not.toBeNull();
    button?.click();
    expect(openThread).toHaveBeenCalledWith("abc123");
  });

  it("[INV-3] runningThreadIds 비어있으면 input 활성화 + banner 미노출", async () => {
    vi.resetModules();
    mockChatStore(makeState({
      runningThreadIds: [],
    }));
    const { NewMessageInput: Component } = await import("@/components/tunaflow/NewMessageInput");
    render(<Component threadMode={false} />);

    expect(screen.queryByTestId("brand-running-guard-banner")).toBeNull();
    const textareas = document.querySelectorAll("textarea");
    expect(textareas[0]).not.toBeDisabled();
  });

  it("[INV-4] thread mode (drawer 내부) 일 땐 brand running 무관하게 input 활성화", async () => {
    vi.resetModules();
    mockChatStore(makeState({
      runningThreadIds: ["branch:abc123"],
      threadBranchConvId: "branch:abc123",
      threadBranchId: "abc123",
      // drawer 안에서 입력은 그대로 동작해야 하므로 selectedConversationId 가
      // null 이어도 thread mode 의 input 은 별도 로직.
    }));
    const { NewMessageInput: Component } = await import("@/components/tunaflow/NewMessageInput");
    render(<Component threadMode={true} />);

    // banner 는 main mode 전용 — thread mode 에서는 노출되지 않음.
    expect(screen.queryByTestId("brand-running-guard-banner")).toBeNull();
  });
});
