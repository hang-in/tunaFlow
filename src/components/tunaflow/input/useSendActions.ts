import { useChatStore } from "@/stores/chatStore";
import { ROUNDTABLE_PARTICIPANTS } from "@/lib/constants";
import type { RtMode, RoundtableParticipant } from "@/types";
import type { Engine } from "./EngineSelector";

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
      console.log("[RT config] loaded:", conversationId, config.participants.map((p) => `${p.name}(${p.engine}/${p.model ?? "default"})`));
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
  "오픈코드": "opencode",
  // 영어
  "claude": "claude",
  "codex": "codex",
  "gemini": "gemini",
  "opencode": "opencode",
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
}

export function useSendActions({
  text, setText, engine, selectedModel, rtMode,
  activeParticipants, setActiveParticipants,
  threadMode = false,
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
      if (lines.length === 2) lines.push("(카탈로그가 비어 있습니다)");
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
            content: `⚠️ **넘길 이전 응답이 없습니다.**\n\n먼저 에이전트에게 질문하고, 응답을 받은 후 handoff를 사용하세요.\n\n입력: \`${prompt}\` → ${handoff.engine}`,
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
        participants = configParticipants;
      } else {
        // No RT config found — warn user instead of silent Haiku fallback
        console.warn("[RT] No rt_config found for", effectiveConvId, "— using default participants");
        const now = Date.now();
        useChatStore.setState((state) => ({
          messages: [...state.messages, {
            id: `local-rt-warn-${now}`,
            conversationId: effectiveConvId!,
            role: "assistant" as const,
            content: "⚠️ **RT 설정을 불러오지 못했습니다.** 기본 참가자로 실행합니다.\n\n새 RT를 만들거나 사이드바에서 [+]로 다시 설정하세요.",
            timestamp: now,
            status: "done",
            engine: "system",
          }],
        }));
        participants = ROUNDTABLE_PARTICIPANTS.filter((p) =>
          activeParticipants.has(p.name),
        );
      }

      // Log actual participants being sent
      console.log("[RT send] convId:", effectiveConvId, "hasRtMessages:", hasRtMessages,
        "participants:", participants.map((p) => `${p.name}(${p.engine}/${p.model ?? "NO MODEL"})`));
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
      await sendThreadMessage(prompt, engine, selectedModel || undefined);
    } else {
      await sendWithEngine(engine ?? "claude", prompt, selectedModel || undefined);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  return { handleSend, handleKeyDown, isRoundtable, hasRtMessages };
}
