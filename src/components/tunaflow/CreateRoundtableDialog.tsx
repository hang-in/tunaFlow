import { useState, useMemo, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { getSetting } from "@/lib/appStore";
import { ROUNDTABLE_PARTICIPANTS } from "@/lib/constants";
import { X, Users, Plus, Minus, ShieldCheck } from "lucide-react";
import type { RtMode, RoundtableParticipant, AgentProfile } from "@/types";
import { AgentAvatar } from "./AgentAvatar";

const RT_MODES: { id: RtMode; label: string; desc: string }[] = [
  { id: "sequential", label: "Sequential", desc: "Each agent sees prior replies within the round" },
  { id: "deliberative", label: "Deliberative", desc: "Round 1 independent, Round 2+ reflects on all" },
];

const ENGINES = ["claude", "codex", "gemini", "ollama", "lmstudio"] as const;

interface CreateRoundtableDialogProps {
  open: boolean;
  onClose: () => void;
  /** If set, creates an RT branch from this message instead of a new RT conversation */
  checkpointId?: string | null;
}

export function CreateRoundtableDialog({ open, onClose, checkpointId }: CreateRoundtableDialogProps) {
  const { t } = useTranslation("branch");
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
        // Use agent profiles — unique name per participant (engine or engine-N if duplicate)
        const counts: Record<string, number> = {};
        setParticipants(profiles.map((p) => {
          counts[p.engine] = (counts[p.engine] ?? 0) + 1;
          const name = counts[p.engine] > 1 ? `${p.engine}-${counts[p.engine]}` : p.engine;
          return { name, engine: p.engine, model: p.model, blind: false };
        }));
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
      // Generate unique name: count how many others already use this engine
      const sameEngineCount = next.filter((p, i) => i !== idx && p.engine === engine).length;
      const name = sameEngineCount > 0 ? `${engine}-${sameEngineCount + 1}` : engine;
      next[idx] = { ...next[idx], engine, name };
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
    setParticipants((prev) => {
      const engine = "claude";
      const sameCount = prev.filter((p) => p.engine === engine).length;
      const name = sameCount > 0 ? `${engine}-${sameCount + 1}` : engine;
      return [...prev, { name, engine }];
    });
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

  const updateRole = (idx: number, role: string) => {
    setParticipants((prev) => {
      const next = [...prev];
      next[idx] = { ...next[idx], role: role || undefined };
      return next;
    });
  };

  const toggleBlind = (idx: number) => {
    setParticipants((prev) => {
      const next = [...prev];
      next[idx] = { ...next[idx], blind: !next[idx].blind };
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
    } catch (e) {
      console.error("[CreateRoundtableDialog] creation failed:", e);
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
          {/* Order: Title → Parent chat → Mode → Participants.
              Title 먼저 — 사용자가 이 RT 의 주제를 먼저 명확히 하고
              구조(채팅/모드/참여자) 는 그 뒤에 배치. */}

          {/* Title */}
          <div>
            <label className="text-[11px] text-sidebar-foreground/60 mb-1 block">Title</label>
            <input value={label} onChange={(e) => setLabel(e.target.value)}
              placeholder="Roundtable title (optional)"
              className="w-full bg-input rounded-md px-3 py-1.5 text-[12px] outline-none text-foreground placeholder:text-muted-foreground/40 border border-border/30 focus:border-ring/40" />
          </div>

          {/* Parent conversation selector — shown when creating from sidebar with multiple chats */}
          {!checkpointId && chatConvs.length > 1 && (
            <div>
              <label className="text-[11px] text-sidebar-foreground/60 mb-1 block">{t("roundtable.labels.parent_chat")}</label>
              <select
                value={effectiveParentConvId ?? ""}
                onChange={(e) => setParentConvId(e.target.value || null)}
                className="w-full bg-input rounded-md px-3 py-1.5 text-[12px] outline-none text-foreground border border-border/30 focus:border-ring/40 cursor-pointer"
              >
                <option value="">{t("roundtable.placeholder.select_chat")}</option>
                {chatConvs.map((c) => (
                  <option key={c.id} value={c.id}>{c.customLabel ?? c.label}</option>
                ))}
              </select>
            </div>
          )}
          {!checkpointId && chatConvs.length === 0 && (
            <div className="text-[10px] text-destructive/70 bg-destructive/5 rounded px-2.5 py-1.5">
              {t("roundtable.empty.no_chats")}
            </div>
          )}

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
                const models = engineModels.filter((m) => m.engine === p.engine);
                return (
                  <div key={idx} className="flex items-center gap-2 px-3 py-2 rounded-md border border-border/20 bg-white/[0.02]">
                    <AgentAvatar engine={p.engine} size="sm" />
                    {/* Engine */}
                    <select value={p.engine ?? "claude"} onChange={(e) => updateEngine(idx, e.target.value)}
                      className="bg-accent/50 rounded px-1.5 py-0.5 text-[11px] font-medium text-foreground/80 outline-none border border-border/20 focus:border-ring/40 cursor-pointer">
                      {ENGINES.map((eng) => <option key={eng} value={eng}>{eng}</option>)}
                    </select>
                    {/* Model */}
                    <select value={p.model ?? ""} onChange={(e) => updateModel(idx, e.target.value)}
                      className={cn(
                        "bg-accent/50 rounded px-1.5 py-0.5 text-[10px] outline-none border focus:border-ring/40 cursor-pointer flex-1 min-w-0",
                        p.model ? "text-foreground/60 border-border/20" : "text-destructive/50 border-destructive/20"
                      )}>
                      <option value="">engine default</option>
                      {models.map((m) => <option key={m.id} value={m.id}>{m.recommended ? "★ " : ""}{m.label}</option>)}
                    </select>
                    {/* Role */}
                    <select value={p.role ?? ""} onChange={(e) => updateRole(idx, e.target.value)}
                      className="bg-accent/50 rounded px-1 py-0.5 text-[9px] text-foreground/50 outline-none border border-border/20 focus:border-ring/40 cursor-pointer w-[80px]">
                      <option value="">no role</option>
                      <option value="proposer">proposer</option>
                      <option value="reviewer">reviewer</option>
                      <option value="verifier">verifier</option>
                      <option value="synthesizer">synthesizer</option>
                    </select>
                    {/* Blind toggle */}
                    <button onClick={() => toggleBlind(idx)} title={p.blind ? "Blind verifier (active)" : "Set as blind verifier"}
                      className={cn("p-0.5 rounded transition-colors shrink-0",
                        p.blind ? "text-amber-500 bg-amber-500/10" : "text-muted-foreground/20 hover:text-muted-foreground/50")}>
                      <ShieldCheck className="w-3 h-3" />
                    </button>
                    {/* Remove */}
                    <button onClick={() => removeParticipant(idx)} title="Remove"
                      className="p-0.5 rounded text-muted-foreground/30 hover:text-destructive transition-colors shrink-0">
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
              {t("roundtable.warning.no_models", { names: noModelParticipants.map((p) => p.name).join(", ") })}
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
