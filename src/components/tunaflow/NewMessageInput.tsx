import { useState, useRef, useEffect, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { getSetting, setSetting } from "@/lib/appStore";
import { DEFAULT_PERSONAS } from "@/lib/defaultPersonas";
import { ROUNDTABLE_PARTICIPANTS } from "@/lib/constants";
import { SendHorizonal, Users, Loader2 } from "lucide-react";
import type { RtMode, RoundtableParticipant, AgentProfile } from "@/types";

import { EngineSelector, type Engine } from "./input/EngineSelector";
import { ModelSelector } from "./input/ModelSelector";
import { ProfileSelector } from "./input/ProfileSelector";
import { isPtyEngine, usePtyStore } from "@/stores/ptyStore";
import { RoundtableControls } from "./input/RoundtableControls";
import { ContextBadges } from "./input/ContextBadges";
import { useSendActions } from "./input/useSendActions";

interface NewMessageInputProps {
  threadMode?: boolean;
  onCreateRT?: () => void;
}

export function NewMessageInput({ threadMode = false, onCreateRT }: NewMessageInputProps) {
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
  const ptySessionId = usePtyStore((s) => s.getSession(engine));
  const [rtMode, setRtMode] = useState<RtMode>("sequential");
  const [ptyRespawning, setPtyRespawning] = useState(false);

  // Agent Profile state — from Zustand store (shared with Settings)
  const profiles = useChatStore((s) => s.agentProfiles);
  const saveConversationEngine = useChatStore((s) => s.saveConversationEngine);
  const toggleSkill = useChatStore((s) => s.toggleSkill);

  // Derive current profileId from per-conversation state (SSOT: _convEngineMap)
  const threadBranchConvIdForRestore = useChatStore((s) => s.threadBranchConvId);
  const effectiveConvForRestore = threadMode ? threadBranchConvIdForRestore : selectedConversationId;
  const getConversationEngine = useChatStore((s) => s.getConversationEngine);
  const selectedProfileId = effectiveConvForRestore
    ? getConversationEngine(effectiveConvForRestore)?.profileId ?? null
    : null;

  const applyProfile = (profile: AgentProfile) => {
    setEngine(profile.engine as Engine);
    if (profile.model) setSelectedModel(profile.model);
    const persona = profile.personaId ? DEFAULT_PERSONAS.find((p) => p.id === profile.personaId) : null;
    useChatStore.setState({
      personaFragment: persona?.promptFragment ?? null,
      personaLabel: persona ? (profile.label === persona.name ? profile.label : `${profile.label} · ${persona.name}`) : profile.label,
    });
    const store = useChatStore.getState();
    const currentSkills = new Set(store.activeSkills);
    const allSkills = new Set([...profile.defaultSkills, ...(persona?.recommendedSkills ?? [])]);
    for (const skill of allSkills) {
      if (!currentSkills.has(skill)) toggleSkill(skill);
    }
  };

  // Restore per-conversation engine state when conversation/thread changes
  // Also re-runs when profiles finish loading (profiles may be empty on first render)
  useEffect(() => {
    if (!effectiveConvForRestore || profiles.length === 0) return;
    const saved = useChatStore.getState().getConversationEngine(effectiveConvForRestore);
    if (saved) {
      setEngine(saved.engine as Engine);
      if (saved.model) setSelectedModel(saved.model);
      if (saved.profileId) {
        const profile = profiles.find((p) => p.id === saved.profileId);
        if (profile) applyProfile(profile);
      }
    } else {
      const defaultProfile = profiles[0];
      if (defaultProfile) {
        applyProfile(defaultProfile);
        saveConversationEngine(effectiveConvForRestore, {
          profileId: defaultProfile.id,
          engine: defaultProfile.engine,
          model: defaultProfile.model,
        });
      }
    }
  // threadBranchConvIdForRestore 포함: 브랜치 드로어가 닫힐 때(null로 변경) 메인챗 프로필 재복원
  }, [effectiveConvForRestore, profiles, threadBranchConvIdForRestore]);

  const handleProfileSelect = (profileId: string | null) => {
    if (!profileId) {
      useChatStore.setState({ personaFragment: null, personaLabel: null });
    } else {
      const profile = profiles.find((p) => p.id === profileId);
      if (profile) applyProfile(profile);
    }
    // Save to per-conversation map only (no global state change)
    const saveTarget = threadMode ? threadBranchConvIdForRestore : selectedConversationId;
    if (saveTarget) {
      const profile = profileId ? profiles.find((p) => p.id === profileId) : null;
      const targetEngine = profile?.engine ?? engine;
      // If profile has no explicit model, pick the recommended model for the engine
      // (do NOT inherit stale selectedModel — that causes wrong model persistence)
      const targetModel = profile?.model
        ?? engineModels.find((m) => m.engine === targetEngine && m.recommended)?.id
        ?? engineModels.find((m) => m.engine === targetEngine)?.id
        ?? undefined;
      saveConversationEngine(saveTarget, {
        profileId,
        engine: targetEngine,
        model: targetModel,
      });
    }
  };
  const [activeParticipants, setActiveParticipants] = useState<Set<string>>(
    () => new Set(ROUNDTABLE_PARTICIPANTS.map((p) => p.name)),
  );

  // Load RT config when entering an RT conversation or branch
  const threadBranchConvId = useChatStore((s) => s.threadBranchConvId);
  const threadBranchId = useChatStore((s) => s.threadBranchId);
  // In thread mode, check branch mode directly (shadow conv may not be in conversations yet)
  const effectiveConvId = threadMode ? threadBranchConvId : selectedConversationId;
  const branches = useChatStore((s) => s.branches);
  const threadBranch = threadMode && threadBranchId ? branches.find((b) => b.id === threadBranchId) : null;
  const isRtConv = threadMode
    ? (threadBranch?.mode === "roundtable")
    : (conversations.find((c) => c.id === effectiveConvId)?.mode === "roundtable");
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

  // 엔진 변경 또는 모델 목록 로드 시 모델 자동 선택
  // convEngineMap에 저장된 model 최우선, 없으면 추천 모델
  useEffect(() => {
    if (currentModels.length === 0) return;
    // Don't auto-select until a conversation is loaded — restore useEffect will handle it
    const convId = threadMode ? threadBranchConvIdForRestore : selectedConversationId;
    if (!convId) return;
    // Check saved model first
    const saved = useChatStore.getState().getConversationEngine(convId);
    if (saved?.model && currentModels.some((m) => m.id === saved.model)) {
      if (selectedModel !== saved.model) setSelectedModel(saved.model);
      return;
    }
    // Current model is valid for this engine — keep it
    if (selectedModel && currentModels.some((m) => m.id === selectedModel)) return;
    // No saved model — use recommended
    const rec = currentModels.find((m) => m.recommended);
    setSelectedModel(rec?.id ?? currentModels[0]?.id ?? "");
  }, [engine, currentModels.length, selectedConversationId]);

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
        <div className="flex items-center gap-1.5 px-2.5 pt-2 pb-1.5 flex-wrap">
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
              {/* Profile selector or running state */}
              {isCurrentThreadRunning ? (
                <div className="flex items-center gap-1.5 shrink-0" title="에이전트 실행 중">
                  <Loader2 className="w-3 h-3 animate-spin text-primary/70" />
                  <span className="text-[11px] text-foreground/60 font-medium">
                    {profiles.find((p) => p.id === selectedProfileId)?.label || engine || "실행 중"}
                  </span>
                </div>
              ) : profiles.length > 0 && (
                <ProfileSelector
                  profiles={profiles}
                  selectedProfileId={selectedProfileId}
                  onSelectProfile={handleProfileSelect}
                />
              )}
              {/* Custom mode: show engine/model selectors (only after profiles loaded) */}
              {profiles.length > 0 && !selectedProfileId && (
                <>
                  <EngineSelector engine={engine} setEngine={(e) => {
                    setEngine(e);
                    // Reset persona if engine doesn't match current profile
                    const currentProfile = profiles.find((p) => p.id === selectedProfileId);
                    if (currentProfile && currentProfile.engine !== e) {
                      useChatStore.setState({ personaFragment: null, personaLabel: null });
                    }
                    // Save to per-conversation map
                    const target = threadMode ? threadBranchConvIdForRestore : selectedConversationId;
                    if (target) {
                      saveConversationEngine(target, { profileId: null, engine: e, model: selectedModel || undefined });
                    }
                  }} />
                  {ptyRespawning && isPtyEngine(engine) && (
                    <span className="flex items-center gap-1 text-[10px] text-muted-foreground/50">
                      <Loader2 className="w-3 h-3 animate-spin" />PTY 로딩 중
                    </span>
                  )}
                  <ModelSelector
                    currentModels={currentModels}
                    selectedModel={selectedModel}
                    setSelectedModel={async (m) => {
                      setSelectedModel(m);
                      // Persist model change to convEngineMap
                      const target = threadMode ? threadBranchConvIdForRestore : selectedConversationId;
                      if (target) {
                        saveConversationEngine(target, { profileId: selectedProfileId, engine, model: m || undefined });
                      }
                      // Respawn PTY session on model change — only if PTY mode is enabled.
                      // Main chat routes through SDK / `-p` CLI by default.
                      const { getSetting: getAppSettingPty } = await import("@/lib/appStore");
                      const ptyOptIn = await getAppSettingPty<boolean>("ptyEnabled", false);
                      if (ptyOptIn && !threadMode && isPtyEngine(engine) && selectedConversationId) {
                        const conv = useChatStore.getState().conversations.find((c) => c.id === selectedConversationId);
                        const project = useChatStore.getState().projects.find((p) => p.key === useChatStore.getState().selectedProjectKey);
                        if (conv && project?.path) {
                          const { spawnPtyForConversation, isPtySpawning } = await import("@/stores/slices/conversationSlice");
                          if (!isPtySpawning(conv.id)) {
                            setPtyRespawning(true);
                            spawnPtyForConversation(conv, project.path)
                              .catch((e) => console.warn("[pty] respawn failed:", e))
                              .finally(() => setPtyRespawning(false));
                          }
                        }
                      }
                    }}
                  />
                </>
              )}
              {isPtyEngine(engine) && (
                <span
                  className={cn(
                    "text-[9px] font-mono font-semibold px-1 rounded",
                    ptySessionId !== null ? "text-status-approved/70 bg-status-approved/8" : "text-muted-foreground/20",
                  )}
                  title={ptySessionId !== null ? "PTY 연결됨" : "PTY 연결 없음"}
                >
                  P
                </span>
              )}
              <span className="flex-1" />
              {onCreateRT && (
                <button
                  onClick={onCreateRT}
                  className="flex items-center gap-1 px-2 py-1 rounded-md text-[10px] font-medium text-agent-gemini/50 hover:text-agent-gemini hover:bg-agent-gemini/10 transition-colors border border-agent-gemini/15"
                  title="New Roundtable"
                >
                  <Users className="w-3 h-3" />
                  <span>RT</span>
                </button>
              )}
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
                ? "/follow codex,claude <prompt> or type…  ↵ send · ⇧↵ newline"
                : "Start roundtable…  ↵ send · ⇧↵ newline"
              : "Ask anything…  ↵ send · ⇧↵ newline"
          }
          disabled={!selectedConversationId}
          rows={1}
          className="w-full px-2.5 py-2 text-[13px] bg-transparent resize-none outline-none text-foreground placeholder:text-muted-foreground/40 leading-relaxed disabled:opacity-40"
        />

        {/* Action bar */}
        <div className="flex items-center gap-1.5 px-2.5 pb-2 pt-0.5">
          <span className="flex-1" />
          {isCurrentThreadRunning && (
            <button
              onClick={() => cancelOperation(selectedConversationId ?? undefined)}
              className="px-2 py-1 rounded text-[11px] font-medium text-destructive/60 hover:text-destructive hover:bg-destructive/8 transition-colors"
            >
              Cancel
            </button>
          )}
          <button
            onClick={handleSend}
            disabled={!text.trim() || !selectedConversationId || ptyRespawning}
            className={cn(
              "flex items-center gap-1 px-2.5 py-1 rounded text-[11px] font-medium transition-colors",
              text.trim() && selectedConversationId && !ptyRespawning
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
