import { useState, useRef, useEffect, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { ROUNDTABLE_PARTICIPANTS } from "@/lib/constants";
import { SendHorizonal } from "lucide-react";
import type { RtMode, RoundtableParticipant } from "@/types";

import { EngineSelector, type Engine } from "./input/EngineSelector";
import { ModelSelector } from "./input/ModelSelector";
import { RoundtableControls } from "./input/RoundtableControls";
import { ContextBadges } from "./input/ContextBadges";
import { useSendActions } from "./input/useSendActions";

interface NewMessageInputProps {
  threadMode?: boolean;
}

export function NewMessageInput({ threadMode = false }: NewMessageInputProps) {
  const selectedConversationId = useChatStore((s) => s.selectedConversationId);
  const conversations = useChatStore((s) => s.conversations);
  const activeBranchId = useChatStore((s) => s.activeBranchId);
  const closeBranchStream = useChatStore((s) => s.closeBranchStream);
  const cancelOperation = useChatStore((s) => s.cancelOperation);
  const runningThreadIds = useChatStore((s) => s.runningThreadIds);
  const messageQueue = useChatStore((s) => s.messageQueue);
  const activeSkills = useChatStore((s) => s.activeSkills);
  const crossSessionIds = useChatStore((s) => s.crossSessionIds);
  const engineModels = useChatStore((s) => s.engineModels);

  const [text, setText] = useState("");
  const [engine, setEngine] = useState<Engine>("claude");
  const [selectedModel, setSelectedModel] = useState<string>("");
  const [rtMode, setRtMode] = useState<RtMode>("sequential");
  const [activeParticipants, setActiveParticipants] = useState<Set<string>>(
    () => new Set(ROUNDTABLE_PARTICIPANTS.map((p) => p.name)),
  );

  // Load RT config when entering an RT conversation or branch
  const threadBranchConvId = useChatStore((s) => s.threadBranchConvId);
  const threadBranchId = useChatStore((s) => s.threadBranchId);
  // In thread mode, check the shadow conversation for RT detection
  const effectiveConvId = threadMode ? threadBranchConvId : selectedConversationId;
  const effectiveConv = conversations.find((c) => c.id === effectiveConvId);
  const isRtConv = effectiveConv?.mode === "roundtable";
  // RT config key: for thread branches use shadow ID, for branches use shadow ID, for conversations use conversation ID
  const rtConfigKey = threadMode ? threadBranchConvId : activeBranchId ? `branch:${activeBranchId}` : selectedConversationId;
  const [rtParticipants, setRtParticipants] = useState<RoundtableParticipant[]>(ROUNDTABLE_PARTICIPANTS);
  useEffect(() => {
    if (!effectiveConvId || !isRtConv) {
      setRtParticipants(ROUNDTABLE_PARTICIPANTS);
      return;
    }
    // Load RT config from DB (persists across app restarts)
    const configId = rtConfigKey ?? effectiveConvId;
    invoke<string | null>("get_rt_config", { conversationId: configId }).then((raw) => {
      if (!raw) {
        // Try parent conversation if this is a branch
        if (configId !== effectiveConvId) {
          return invoke<string | null>("get_rt_config", { conversationId: effectiveConvId });
        }
        return null;
      }
      return raw;
    }).then((raw) => {
      if (!raw) {
        console.warn("[RT] No config in DB for", configId);
        setRtParticipants(ROUNDTABLE_PARTICIPANTS);
        return;
      }
      try {
        const config = JSON.parse(raw) as { participants?: RoundtableParticipant[]; mode?: RtMode };
        if (config.mode) setRtMode(config.mode);
        if (config.participants?.length) {
          setRtParticipants(config.participants);
          setActiveParticipants(new Set(config.participants.map((p) => p.name)));
        } else {
          setRtParticipants(ROUNDTABLE_PARTICIPANTS);
        }
      } catch { setRtParticipants(ROUNDTABLE_PARTICIPANTS); }
    }).catch(() => setRtParticipants(ROUNDTABLE_PARTICIPANTS));
  }, [effectiveConvId, activeBranchId, threadBranchId]);

  const effectiveThreadId = threadMode ? threadBranchConvId : selectedConversationId;
  const isCurrentThreadRunning = !!effectiveThreadId && runningThreadIds.includes(effectiveThreadId);
  const currentQueueLength = messageQueue.filter((q) => q.threadId === selectedConversationId).length;

  // 현재 엔진의 모델 목록
  const currentModels = useMemo(
    () => engineModels.filter((m) => m.engine === engine),
    [engineModels, engine],
  );

  // 엔진 변경 시 추천 모델로 자동 선택
  useEffect(() => {
    const rec = currentModels.find((m) => m.recommended);
    setSelectedModel(rec?.id ?? currentModels[0]?.id ?? "");
  }, [engine, currentModels.length]);

  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const { handleSend, handleKeyDown, isRoundtable, hasRtMessages } = useSendActions({
    text, setText, engine, selectedModel, rtMode,
    activeParticipants, setActiveParticipants,
    threadMode,
  });

  const toggleParticipant = (name: string) => {
    setActiveParticipants((prev) => {
      const next = new Set(prev);
      if (next.has(name)) {
        if (next.size > 1) next.delete(name); // keep at least 1
      } else {
        next.add(name);
      }
      return next;
    });
  };

  useEffect(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, 160)}px`;
  }, [text]);

  return (
    <div className="px-4 pb-3 pt-1.5 shrink-0">
      {/* Branch stream banner — hide in thread mode (drawer has its own header) */}
      {!threadMode && activeBranchId && (() => {
        const { branches } = useChatStore.getState();
        const ab = branches.find((b) => b.id === activeBranchId);
        const abIsRT = ab?.mode === "roundtable";
        const abLabel = ab?.customLabel ?? ab?.label ?? "Branch";
        return (
        <div className={cn("mb-1.5 flex items-center gap-2 text-[10px] rounded px-2.5 py-1",
          abIsRT ? "text-agent-gemini/60 bg-agent-gemini/5" : "text-muted-foreground/60 bg-primary/5")}>
          <span className="font-mono text-[9px] uppercase tracking-wide">{abIsRT ? "RT Branch" : "Branch"}</span>
          <span className="font-medium text-foreground/60 flex-1 truncate">{abLabel}</span>
          <button
            onClick={closeBranchStream}
            className="ml-auto text-muted-foreground/50 hover:text-foreground text-[10px]"
          >
            ← Back
          </button>
        </div>
        ); })()}

      {/* Context status */}
      <ContextBadges activeSkills={activeSkills} crossSessionIds={crossSessionIds} />

      <div className="rounded-lg border border-border/40 bg-card/60 focus-within:border-ring/40 transition-colors">
        {/* Mode bar */}
        <div className="flex items-center gap-1.5 px-2.5 pt-2 pb-1.5 border-b border-border/30 flex-wrap">
          {isRoundtable ? (
            <RoundtableControls
              rtMode={rtMode}
              setRtMode={setRtMode}
              participants={rtParticipants}
              activeParticipants={activeParticipants}
              toggleParticipant={toggleParticipant}
            />
          ) : (
            <>
              <EngineSelector engine={engine} setEngine={setEngine} />
              <ModelSelector
                currentModels={currentModels}
                selectedModel={selectedModel}
                setSelectedModel={setSelectedModel}
              />
            </>
          )}
        </div>

        {/* Textarea */}
        <textarea
          ref={textareaRef}
          value={text}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={
            !selectedConversationId
              ? "Select a conversation first"
              : isRoundtable
              ? hasRtMessages
                ? "/follow codex,claude <prompt> or type… (↵)"
                : "Start roundtable… (↵)"
              : "Ask anything… (↵)"
          }
          disabled={!selectedConversationId}
          rows={1}
          className="w-full px-2.5 py-2 text-[13px] bg-transparent resize-none outline-none text-foreground placeholder:text-muted-foreground/40 leading-relaxed disabled:opacity-40"
        />

        {/* Action bar */}
        <div className="flex items-center gap-1.5 px-2.5 pb-2 pt-0.5">
          <span className="text-[9px] text-muted-foreground/30 font-mono">↵ send · ⇧↵ newline</span>
          <span className="flex-1" />
          {isCurrentThreadRunning && (
            <button
              onClick={() => cancelOperation(selectedConversationId ?? undefined)}
              className="px-2 py-1 rounded text-[10px] font-medium text-destructive/60 hover:text-destructive hover:bg-destructive/8 transition-colors"
            >
              Cancel
            </button>
          )}
          <button
            onClick={handleSend}
            disabled={!text.trim() || !selectedConversationId}
            className={cn(
              "flex items-center gap-1 px-2.5 py-1 rounded text-[10px] font-medium transition-colors",
              text.trim() && selectedConversationId
                ? isCurrentThreadRunning
                  ? "bg-agent-gemini/12 text-agent-gemini/80 hover:bg-agent-gemini/20"
                  : "bg-primary/90 text-primary-foreground hover:bg-primary"
                : "bg-muted text-muted-foreground/40 cursor-not-allowed"
            )}
          >
            <SendHorizonal className="w-3 h-3" />
            {isCurrentThreadRunning
              ? `Queue${currentQueueLength > 0 ? ` (${currentQueueLength})` : ""}`
              : isRoundtable ? (hasRtMessages ? "Next" : "Start") : "Send"}
          </button>
        </div>
      </div>
    </div>
  );
}
