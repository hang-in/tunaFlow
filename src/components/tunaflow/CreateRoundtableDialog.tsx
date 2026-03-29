import { useState, useMemo, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { getSetting } from "@/lib/appStore";
import { ROUNDTABLE_PARTICIPANTS } from "@/lib/constants";
import { X, Users, Plus, Minus } from "lucide-react";
import type { RtMode, RoundtableParticipant, AgentProfile } from "@/types";
import { AgentAvatar } from "./AgentAvatar";

const RT_MODES: { id: RtMode; label: string; desc: string }[] = [
  { id: "sequential", label: "Sequential", desc: "Each agent sees prior replies within the round" },
  { id: "deliberative", label: "Deliberative", desc: "Round 1 independent, Round 2+ reflects on all" },
];

const ENGINES = ["claude", "codex", "gemini", "opencode"] as const;

interface CreateRoundtableDialogProps {
  open: boolean;
  onClose: () => void;
  /** If set, creates an RT branch from this message instead of a new RT conversation */
  checkpointId?: string | null;
}

export function CreateRoundtableDialog({ open, onClose, checkpointId }: CreateRoundtableDialogProps) {
  const { selectedConversationId, conversations, createBranch, selectConversation, engineModels } = useChatStore();

  // Non-shadow, non-RT conversations for parent selection
  const chatConvs = useMemo(
    () => conversations.filter((c) => c.type !== "branch" && c.mode !== "roundtable"),
    [conversations],
  );

  const [label, setLabel] = useState("");
  const [mode, setMode] = useState<RtMode>("sequential");
  // Parent conversation for sidebar-created RT (auto-selected if only 1 chat)
  const [parentConvId, setParentConvId] = useState<string | null>(null);
  // Auto-select parent when dialog opens
  const effectiveParentConvId = checkpointId
    ? selectedConversationId
    : parentConvId ?? (chatConvs.length === 1 ? chatConvs[0].id : null);
  const [agentProfiles, setAgentProfiles] = useState<AgentProfile[]>([]);
  const [participants, setParticipants] = useState<RoundtableParticipant[]>(
    () => ROUNDTABLE_PARTICIPANTS.map((p) => ({ ...p }))
  );
  const [disabledIdx, setDisabledIdx] = useState<Set<number>>(new Set());
  const [creating, setCreating] = useState(false);

  // Load agent profiles and use them as initial participants if available
  useEffect(() => {
    if (!open) return;
    getSetting<AgentProfile[]>("agentProfiles", []).then((profiles) => {
      setAgentProfiles(profiles);
      if (profiles.length >= 2) {
        // Use agent profiles as participants
        setParticipants(profiles.map((p) => ({
          name: p.label,
          engine: p.engine,
          model: p.model,
        })));
        setDisabledIdx(new Set());
      }
    });
  }, [open]);

  const toggleParticipant = (idx: number) => {
    setDisabledIdx((prev) => {
      const next = new Set(prev);
      next.has(idx) ? next.delete(idx) : next.add(idx);
      return next;
    });
  };

  const activeParticipants = participants.filter((_, i) => !disabledIdx.has(i));

  const updateEngine = (idx: number, engine: string) => {
    setParticipants((prev) => {
      const next = [...prev];
      next[idx] = { ...next[idx], engine };
      // Auto-select recommended model for new engine
      const rec = engineModels.find((m) => m.engine === engine && m.recommended);
      next[idx].model = rec?.id;
      return next;
    });
  };

  const updateModel = (idx: number, model: string) => {
    setParticipants((prev) => {
      const next = [...prev];
      next[idx] = { ...next[idx], model: model || undefined };
      return next;
    });
  };

  const addParticipant = () => {
    // Find a profile not already in participants
    const usedNames = new Set(participants.map((p) => p.name));
    const unused = agentProfiles.find((p) => !usedNames.has(p.label));
    if (unused) {
      setParticipants((prev) => [...prev, { name: unused.label, engine: unused.engine, model: unused.model }]);
    } else {
      setParticipants((prev) => [...prev, { name: `Agent ${prev.length + 1}`, engine: "claude" }]);
    }
  };

  const removeParticipant = (idx: number) => {
    if (participants.length <= 2) return;
    setParticipants((prev) => prev.filter((_, i) => i !== idx));
  };

  const updateName = (idx: number, name: string) => {
    setParticipants((prev) => {
      const next = [...prev];
      next[idx] = { ...next[idx], name };
      return next;
    });
  };

  const noModelParticipants = activeParticipants.filter((p) => !p.model);

  const handleCreate = async () => {
    if (activeParticipants.length < 2) return;
    if (!effectiveParentConvId) return;
    setCreating(true);
    try {
      // Always create RT as a branch of the parent conversation
      // checkpointId is set when branching from a specific message, null when creating from sidebar
      await createBranch(effectiveParentConvId, checkpointId ?? undefined, label.trim() || undefined, "roundtable");
      // Find the newly created branch and store config
      const { branches } = useChatStore.getState();
      const newBranch = checkpointId
        ? branches.find((b) => b.checkpointId === checkpointId && b.mode === "roundtable")
        : branches.filter((b) => b.mode === "roundtable" && b.conversationId === effectiveParentConvId)
            .sort((a, b) => b.createdAt - a.createdAt)[0];
      if (newBranch) {
        // Ensure shadow conversation exists before saving config
        const shadowId = await invoke<string>("open_branch_stream", { branchId: newBranch.id });
        const configJson = JSON.stringify({ participants: activeParticipants, mode });
        await invoke("save_rt_config", { conversationId: shadowId, configJson });
        // Ensure parent conversation is selected, then open RT branch in drawer
        if (effectiveParentConvId !== useChatStore.getState().selectedConversationId) {
          await selectConversation(effectiveParentConvId);
        }
        await useChatStore.getState().openThread(newBranch.id);
      }
      onClose();
    } catch {
      // silent
    } finally {
      setCreating(false);
    }
  };

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-[60] flex items-center justify-center">
      {/* Backdrop */}
      <div className="absolute inset-0 bg-black/30 backdrop-blur-[1px]" onClick={onClose} />

      {/* Dialog */}
      <div className="relative bg-card border border-border/40 rounded-lg shadow-2xl w-[480px] max-h-[80vh] overflow-y-auto">
        {/* Header */}
        <div className="flex items-center gap-2 px-4 h-11 border-b border-border/30">
          <Users className="w-4 h-4 text-agent-gemini" />
          <span className="text-[13px] font-medium text-foreground flex-1">New Roundtable</span>
          <button onClick={onClose} className="p-1 rounded hover:bg-accent text-muted-foreground hover:text-foreground transition-colors">
            <X className="w-4 h-4" />
          </button>
        </div>

        <div className="p-4 space-y-4">
          {/* Parent conversation selector — shown when creating from sidebar with multiple chats */}
          {!checkpointId && chatConvs.length > 1 && (
            <div>
              <label className="text-[11px] text-sidebar-foreground/60 mb-1 block">상위 채팅</label>
              <select
                value={effectiveParentConvId ?? ""}
                onChange={(e) => setParentConvId(e.target.value || null)}
                className="w-full bg-input rounded-md px-3 py-1.5 text-[12px] outline-none text-foreground border border-border/30 focus:border-ring/40 cursor-pointer"
              >
                <option value="">채팅을 선택하세요</option>
                {chatConvs.map((c) => (
                  <option key={c.id} value={c.id}>{c.customLabel ?? c.label}</option>
                ))}
              </select>
            </div>
          )}
          {!checkpointId && chatConvs.length === 0 && (
            <div className="text-[10px] text-destructive/70 bg-destructive/5 rounded px-2.5 py-1.5">
              채팅이 없습니다. 먼저 채팅을 생성하세요.
            </div>
          )}

          {/* Label */}
          <div>
            <label className="text-[11px] text-sidebar-foreground/60 mb-1 block">Title</label>
            <input value={label} onChange={(e) => setLabel(e.target.value)}
              placeholder="Roundtable title (optional)"
              className="w-full bg-input rounded-md px-3 py-1.5 text-[12px] outline-none text-foreground placeholder:text-muted-foreground/40 border border-border/30 focus:border-ring/40" />
          </div>

          {/* Mode */}
          <div>
            <label className="text-[11px] text-sidebar-foreground/60 mb-1.5 block">Mode</label>
            <div className="flex gap-2">
              {RT_MODES.map((m) => (
                <button key={m.id} onClick={() => setMode(m.id)}
                  className={cn("flex-1 px-3 py-2 rounded-md border text-left transition-colors",
                    mode === m.id ? "border-agent-gemini/40 bg-agent-gemini/8" : "border-border/20 hover:border-border/40")}>
                  <span className={cn("text-[11px] font-medium block", mode === m.id ? "text-agent-gemini" : "text-foreground/70")}>{m.label}</span>
                  <span className="text-[9px] text-muted-foreground/50 block mt-0.5">{m.desc}</span>
                </button>
              ))}
            </div>
          </div>

          {/* Participants */}
          <div>
            <div className="flex items-center gap-2 mb-2">
              <label className="text-[11px] text-sidebar-foreground/60 flex-1">Participants ({activeParticipants.length})</label>
              <button onClick={addParticipant} className="flex items-center gap-1 text-[10px] text-primary/70 hover:text-primary transition-colors">
                <Plus className="w-3 h-3" /> Add
              </button>
            </div>
            <div className="space-y-2">
              {participants.map((p, idx) => {
                const disabled = disabledIdx.has(idx);
                const models = engineModels.filter((m) => m.engine === p.engine);
                return (
                  <div key={idx} className={cn("flex items-center gap-2 px-3 py-2 rounded-md border transition-colors",
                    disabled ? "border-border/10 opacity-40" : "border-border/20 bg-white/[0.02]")}>
                    <AgentAvatar engine={p.engine} size="sm" />
                    <input value={p.name} onChange={(e) => updateName(idx, e.target.value)}
                      className="w-[80px] bg-transparent text-[11px] font-medium text-foreground outline-none border-b border-transparent focus:border-border/40" />
                    <select value={p.engine ?? "claude"} onChange={(e) => updateEngine(idx, e.target.value)}
                      className="bg-accent/50 rounded px-1.5 py-0.5 text-[10px] text-foreground/70 outline-none border border-border/20 focus:border-ring/40 cursor-pointer">
                      {ENGINES.map((eng) => <option key={eng} value={eng}>{eng}</option>)}
                    </select>
                    <select value={p.model ?? ""} onChange={(e) => updateModel(idx, e.target.value)}
                      className={cn(
                        "bg-accent/50 rounded px-1.5 py-0.5 text-[9px] outline-none border focus:border-ring/40 cursor-pointer max-w-[140px]",
                        p.model ? "text-foreground/60 border-border/20" : "text-destructive/50 border-destructive/20"
                      )}>
                      <option value="">engine default</option>
                      {models.map((m) => <option key={m.id} value={m.id}>{m.recommended ? "★ " : ""}{m.label}</option>)}
                    </select>
                    <span className="flex-1" />
                    <button onClick={() => removeParticipant(idx)} title="Remove"
                      className="p-0.5 rounded text-muted-foreground/30 hover:text-destructive transition-colors">
                      <Minus className="w-3 h-3" />
                    </button>
                  </div>
                );
              })}
            </div>
          </div>

          {/* Validation warnings */}
          {noModelParticipants.length > 0 && (
            <div className="text-[10px] text-amber-500/70 bg-amber-500/5 rounded px-2.5 py-1.5">
              {noModelParticipants.map((p) => p.name).join(", ")} — 모델 미선택 (엔진 기본값 사용)
            </div>
          )}

          {/* Actions */}
          <div className="flex items-center gap-2 pt-2 border-t border-border/20">
            <span className="flex-1" />
            <button onClick={onClose} className="px-3 py-1.5 rounded-md text-[11px] text-muted-foreground hover:bg-accent transition-colors">
              Cancel
            </button>
            <button onClick={handleCreate} disabled={creating || activeParticipants.length < 2 || !effectiveParentConvId}
              className="px-4 py-1.5 rounded-md text-[11px] font-medium bg-agent-gemini/15 text-agent-gemini hover:bg-agent-gemini/25 transition-colors disabled:opacity-40">
              {creating ? "Creating…" : "Create Roundtable"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
