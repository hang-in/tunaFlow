import i18n from "@/locales";
import { useChatStore } from "@/stores/chatStore";
import { ROUNDTABLE_PARTICIPANTS } from "@/lib/constants";
import type { RtMode, RoundtableParticipant } from "@/types";
import type { Engine } from "./EngineSelector";
import { appendAttachmentsToPrompt, type Attachment } from "@/lib/attachments";

// ─── RT config helpers ───────────────────────────────────────────────────────

/** Read RT participant config from DB. */
async function getRtConfigParticipants(conversationId: string | null): Promise<RoundtableParticipant[] | null> {
  if (!conversationId) return null;
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    const raw = await invoke<string | null>("get_rt_config", { conversationId });
    if (!raw) {
      console.warn("[RT config] not found in DB for:", conversationId);
      return null;
    }
    const config = JSON.parse(raw) as { participants?: RoundtableParticipant[] };
    if (config.participants?.length) {
      // RT config loaded successfully
      return config.participants;
    }
    return null;
  } catch { return null; }
}

// ─── Parse helpers ────────────────────────────────────────────────────────────

/** Parse `/follow name1,name2 <prompt>` — returns null if no match */
export function parseFollowCommand(
  text: string,
  allParticipants: RoundtableParticipant[],
): { participants: RoundtableParticipant[]; prompt: string } | null {
  const match = text.match(/^\/follow\s+([\w,\s]+?)\s+([\s\S]+)/);
  if (!match) return null;
  const requestedNames = match[1]
    .split(",")
    .map((n) => n.trim().toLowerCase())
    .filter(Boolean);
  const matched = allParticipants.filter((p) =>
    requestedNames.includes(p.name.toLowerCase()),
  );
  if (matched.length === 0) return null;
  return { participants: matched, prompt: match[2].trim() };
}

// ─── Natural language handoff alias parser ────────────────────────────────────

const ENGINE_ALIASES: Record<string, string> = {
  // 한국어
  "클로드": "claude", "클": "claude",
  "코덱스": "codex", "코": "codex",
  "제미나이": "gemini", "제미니": "gemini", "젬": "gemini",
  "올라마": "ollama",
  "엘엠": "lmstudio",
  // 영어
  "claude": "claude",
  "codex": "codex",
  "gemini": "gemini",
  "ollama": "ollama",
  "lmstudio": "lmstudio",
};

const GOAL_ALIASES: Record<string, string> = {
  "구현": "implement", "구현해": "implement", "만들어": "implement",
  "검토": "critique", "검토해": "critique", "리뷰": "critique",
  "다듬": "refine", "다듬어": "refine", "정리": "refine", "정리해": "refine",
  "넘겨": "", "넘기기": "", "보내": "", "시켜": "",
  "implement": "implement", "refine": "refine", "critique": "critique",
  "review": "critique", "fix": "implement", "summarize": "refine",
};

// Pattern: "{engine alias}로 {goal?}" or "{engine alias}에게 {goal?}" or just "{engine alias}로"
// Also: "{goal} {engine alias}로" (reversed)
export function parseNaturalHandoff(text: string): { engine: string; goal: string } | null {
  const trimmed = text.trim();
  // Too long → not a handoff command
  if (trimmed.length > 40) return null;

  const lower = trimmed.toLowerCase();

  // Try each engine alias
  for (const [alias, engine] of Object.entries(ENGINE_ALIASES)) {
    // Pattern: "{alias}로 {goal}" or "{alias}에게 {goal}" or "{alias}로"
    const suffixes = [`${alias}로`, `${alias}에게`, `${alias}한테`];
    for (const suffix of suffixes) {
      if (lower.startsWith(suffix)) {
        const rest = lower.slice(suffix.length).trim();
        const goal = rest ? (GOAL_ALIASES[rest] ?? rest) : "";
        return { engine, goal };
      }
      if (lower.endsWith(suffix)) {
        const rest = lower.slice(0, lower.length - suffix.length).trim();
        const goal = rest ? (GOAL_ALIASES[rest] ?? rest) : "";
        return { engine, goal };
      }
    }

    // Pattern: just the engine name alone (e.g., "codex", "claude")
    if (lower === alias) {
      return { engine, goal: "" };
    }
  }

  return null;
}

// ─── Hook ─────────────────────────────────────────────────────────────────────

interface UseSendActionsParams {
  text: string;
  setText: (v: string) => void;
  engine: Engine;
  selectedModel: string;
  rtMode: RtMode;
  activeParticipants: Set<string>;
  setActiveParticipants: React.Dispatch<React.SetStateAction<Set<string>>>;
  /** When true, routes sends through sendThreadMessage instead of main send functions */
  threadMode?: boolean;
  /** Attachments to include with this send — appended as `[첨부 파일]` section. */
  attachments?: Attachment[];
  /** Called after a successful send — used by caller to clear attachments. */
  onSendComplete?: () => void;
}

export function useSendActions({
  text, setText, engine, selectedModel, rtMode,
  activeParticipants, setActiveParticipants,
  threadMode = false,
  attachments = [],
  onSendComplete,
}: UseSendActionsParams) {
  const {
    selectedConversationId,
    threadBranchConvId,
    conversations,
    messages,
    threadMessages,
    runningThreadIds,
    sendMessage,
    sendWithEngine,
    sendRoundtable,
    sendRoundtableFollowup,
    sendThreadRoundtable,
    sendThreadRoundtableFollowup,
    sendFollowup,
    sendThreadMessage,
    loadEngineModels,
  } = useChatStore();

  // Resolve model at send time: convEngineMap model > selectedModel > undefined
  const resolveModel = (): string | undefined => {
    const store = useChatStore.getState();
    const convId = threadMode
      ? store.threadBranchConvId
      : store.selectedConversationId;
    if (convId) {
      const saved = store.getConversationEngine(convId);
      if (saved?.model) return saved.model;
    }
    return selectedModel || undefined;
  };

  // In thread mode, use thread's shadow conversation for RT detection
  const effectiveConvId = threadMode ? threadBranchConvId : selectedConversationId;
  const effectiveMessages = threadMode ? threadMessages : messages;
  const currentConv = conversations.find((c) => c.id === effectiveConvId);
  // Also check branch mode directly (more reliable than shadow conv lookup)
  const threadBranchId = useChatStore((s) => s.threadBranchId);
  const branches = useChatStore((s) => s.branches);
  const threadBranch = threadMode && threadBranchId ? branches.find((b) => b.id === threadBranchId) : null;
  const isRoundtable = threadMode
    ? (threadBranch?.mode === "roundtable")
    : (currentConv?.mode === "roundtable");
  const hasRtMessages = isRoundtable && effectiveMessages.some((m) => m.persona);

  const handleSend = async () => {
    let prompt = text.trim();
    if (!prompt || !effectiveConvId) return;

    // 첨부가 있으면 prompt 끝에 경로 섹션 append. 사용자가 직접 경로를 적지
    // 않고도 에이전트가 Read 툴/vision 으로 확인하게 유도.
    if (attachments.length > 0) {
      prompt = appendAttachmentsToPrompt(prompt, attachments);
    }

    // !models 명령 처리
    if (prompt === "!models" || prompt === "!models --refresh") {
      if (prompt.includes("--refresh")) {
        await loadEngineModels(true);
      }
      const lines = ["## Engine Model Catalog", ""];
      let lastEngine = "";
      for (const m of useChatStore.getState().engineModels) {
        if (m.engine !== lastEngine) {
          lines.push(`### ${m.engine}`);
          lastEngine = m.engine;
        }
        lines.push(`- ${m.recommended ? "★ " : "  "}${m.id} — ${m.label} [${m.source}]`);
      }
      if (lines.length === 2) lines.push(i18n.t("chat:input.models_catalog_empty"));
      // 로컬 표시 — 임시 메시지로 추가
      const now = Date.now();
      useChatStore.setState((state) => ({
        messages: [...state.messages, {
          id: `local-models-${now}`,
          conversationId: effectiveConvId!,
          role: "assistant" as const,
          content: lines.join("\n"),
          timestamp: now,
          status: "done",
          engine: "system",
        }],
      }));
      setText("");
      return;
    }

    // /clear — reset PTY session for current conversation
    if (prompt === "/clear") {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        const { usePtyStore } = await import("@/stores/ptyStore");
        const { toast } = await import("sonner");
        const { getSetting: getAppSetting } = await import("@/lib/appStore");
        const ptyOptIn = await getAppSetting<boolean>("ptyEnabled", false);
        // Kill current PTY
        const pty = usePtyStore.getState();
        const sid = pty.getSession("claude");
        if (sid !== null) {
          await invoke("pty_kill", { sessionId: sid }).catch(() => {});
          pty.clearSession("claude");
        }
        // Clear resume_token in DB
        await invoke("update_resume_token", { conversationId: effectiveConvId, resumeToken: null });
        // Spawn fresh PTY session only if PTY mode is opted in.
        if (ptyOptIn) {
          const projectKey = useChatStore.getState().selectedProjectKey;
          if (projectKey) {
            const project = await invoke<{ path?: string }>("get_project", { key: projectKey });
            if (project.path) {
              const conv = await invoke<import("@/types").Conversation>("get_conversation", { id: effectiveConvId });
              const { spawnPtyForConversation } = await import("@/stores/slices/conversationSlice");
              await spawnPtyForConversation({ ...conv, resumeToken: undefined }, project.path);
            }
          }
        }
        toast.success(i18n.t("chat:input.pty_clear_success"));
      } catch (e) {
        console.error("[/clear]", e);
        const { toast } = await import("sonner");
        toast.error(i18n.t("chat:input.pty_clear_failed"));
      }
      setText("");
      return;
    }

    // Natural language handoff: "클로드로 넘겨", "codex로 구현", etc.
    const handoff = parseNaturalHandoff(prompt);
    if (handoff && !isRoundtable) {
      // Source priority: 1) explicit handoffSource (artifact/plan expanded) 2) last assistant message
      const explicitSource = useChatStore.getState().handoffSource;
      if (explicitSource) {
        setText("");
        useChatStore.setState({ handoffSource: null });
        await sendFollowup(handoff.engine, explicitSource.type, explicitSource.content, handoff.goal || undefined);
        return;
      }
      const lastAssistant = effectiveMessages.filter((m) => m.role === "assistant" && m.status === "done").pop();
      if (!lastAssistant) {
        // No source — block handoff, show inline guide
        const now = Date.now();
        useChatStore.setState((state) => ({
          messages: [...state.messages, {
            id: `local-guide-${now}`,
            conversationId: effectiveConvId!,
            role: "assistant" as const,
            content: i18n.t("chat:input.no_previous_response", { prompt, engine: handoff.engine }),
            timestamp: now,
            status: "done",
            engine: "system",
          }],
        }));
        setText("");
        return;
      }
      setText("");
      await sendFollowup(handoff.engine, "message", lastAssistant.content, handoff.goal || undefined);
      return;
    }

    setText("");
    // 메인 전송 경로 진입 직전에 첨부 clear. /help, /clear, handoff 등 특수
    // 경로는 첨부와 무관하므로 clear 하지 않음 (첨부는 다음 일반 send 때 포함).
    onSendComplete?.();

    if (isRoundtable) {
      // Determine participants: /follow override → RT config (DB) → warn on missing config
      let participants: RoundtableParticipant[];
      const configParticipants = await getRtConfigParticipants(effectiveConvId);
      const allParticipants = configParticipants ?? ROUNDTABLE_PARTICIPANTS;
      const followCmd = parseFollowCommand(prompt, allParticipants);
      if (followCmd) {
        participants = followCmd.participants;
        prompt = followCmd.prompt;
        setActiveParticipants(new Set(participants.map((p) => p.name)));
      } else if (configParticipants) {
        // Filter by activeParticipants toggle state — 1명도 허용 (targeted follow-up, solo synthesis)
        const filtered = configParticipants.filter((p) => activeParticipants.has(p.name));
        participants = filtered.length > 0 ? filtered : configParticipants; // 0명일 때만 전체 fallback
      } else {
        // No RT config found — warn user instead of silent Haiku fallback
        console.warn("[RT] No rt_config found for", effectiveConvId, "— using default participants");
        const now = Date.now();
        useChatStore.setState((state) => ({
          messages: [...state.messages, {
            id: `local-rt-warn-${now}`,
            conversationId: effectiveConvId!,
            role: "assistant" as const,
            content: i18n.t("chat:input.rt_config_missing"),
            timestamp: now,
            status: "done",
            engine: "system",
          }],
        }));
        participants = ROUNDTABLE_PARTICIPANTS.filter((p) =>
          activeParticipants.has(p.name),
        );
      }

      // RT send — debug logging removed (participants visible in RT status events)
      const noModel = participants.filter((p) => !p.model);
      if (noModel.length > 0) {
        console.warn("[RT] Participants without explicit model:", noModel.map((p) => p.name).join(", "), "— engine default will be used");
      }

      if (threadMode) {
        // Thread RT: use thread-aware RT functions
        if (hasRtMessages) {
          await sendThreadRoundtableFollowup(prompt, participants, rtMode);
        } else {
          await sendThreadRoundtable(prompt, participants, rtMode);
        }
      } else if (hasRtMessages) {
        await sendRoundtableFollowup(prompt, participants, rtMode);
      } else {
        await sendRoundtable(prompt, participants, rtMode);
      }
    } else if (threadMode) {
      // Thread mode: route through sendThreadMessage
      const model = resolveModel();
      // Codex vision: pass abs paths of image attachments as -i argv. Other
      // engines read via Read tool from the prompt path section.
      const imagePaths = attachments.filter((a) => a.isImage && a.absPath).map((a) => a.absPath);
      await sendThreadMessage(prompt, engine, model, imagePaths.length > 0 ? { imagePaths } : undefined);
    } else {
      const model = resolveModel();
      const imagePaths = attachments.filter((a) => a.isImage && a.absPath).map((a) => a.absPath);
      await sendWithEngine(engine ?? "claude", prompt, model, undefined, imagePaths.length > 0 ? { imagePaths } : undefined);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    // Skip send during IME composition (Korean/CJK input) — first Enter confirms the
    // composed character; user needs a second Enter to actually send.
    if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
      e.preventDefault();
      handleSend();
    }
  };

  return { handleSend, handleKeyDown, isRoundtable, hasRtMessages };
}
