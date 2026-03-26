import { useState, useMemo } from "react";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { ROUNDTABLE_PARTICIPANTS } from "@/lib/constants";
import { X, Users, Plus, Minus } from "lucide-react";
import type { RtMode, RoundtableParticipant } from "@/types";
import { AgentAvatar } from "./AgentAvatar";

const RT_MODES: { id: RtMode; label: string; desc: string }[] = [
  { id: "sequential", label: "Sequential", desc: "Each agent sees prior replies within the round" },
  { id: "deliberative", label: "Deliberative", desc: "Round 1 independent, Round 2+ reflects on all" },
];

const ENGINES = ["claude", "codex", "gemini", "opencode"] as const;

interface CreateRoundtableDialogProps {
  open: boolean;
  onClose: () => void;
}

export function CreateRoundtableDialog({ open, onClose }: CreateRoundtableDialogProps) {
  const { selectedProjectKey, createConversation, selectConversation, engineModels } = useChatStore();

  const [label, setLabel] = useState("");
  const [mode, setMode] = useState<RtMode>("sequential");
  const [participants, setParticipants] = useState<RoundtableParticipant[]>(
    () => ROUNDTABLE_PARTICIPANTS.map((p) => ({ ...p }))
  );
  const [disabledIdx, setDisabledIdx] = useState<Set<number>>(new Set());
  const [creating, setCreating] = useState(false);

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
    setParticipants((prev) => [
      ...prev,
      { name: `Agent ${prev.length + 1}`, engine: "claude" },
    ]);
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

  const handleCreate = async () => {
    if (!selectedProjectKey || activeParticipants.length < 2) return;
    setCreating(true);
    try {
      const rtLabel = label.trim() || `Roundtable ${Date.now() % 10000}`;
      const conv = await createConversation({
        projectKey: selectedProjectKey,
        label: rtLabel,
        type: "main",
        mode: "roundtable",
        source: "tunadish",
      });
      // Store RT config keyed by conversation ID — NewMessageInput reads this
      sessionStorage.setItem(`rt_config:${conv.id}`, JSON.stringify({
        participants: activeParticipants,
        mode,
      }));
      await selectConversation(conv.id);
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
                      className="bg-transparent text-[10px] text-muted-foreground/70 outline-none">
                      {ENGINES.map((eng) => <option key={eng} value={eng}>{eng}</option>)}
                    </select>
                    {models.length > 0 && (
                      <select value={p.model ?? ""} onChange={(e) => updateModel(idx, e.target.value)}
                        className="bg-transparent text-[9px] text-muted-foreground/50 outline-none max-w-[100px]">
                        <option value="">default</option>
                        {models.map((m) => <option key={m.id} value={m.id}>{m.recommended ? "★ " : ""}{m.label}</option>)}
                      </select>
                    )}
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

          {/* Actions */}
          <div className="flex items-center gap-2 pt-2 border-t border-border/20">
            <span className="flex-1" />
            <button onClick={onClose} className="px-3 py-1.5 rounded-md text-[11px] text-muted-foreground hover:bg-accent transition-colors">
              Cancel
            </button>
            <button onClick={handleCreate} disabled={creating || activeParticipants.length < 2}
              className="px-4 py-1.5 rounded-md text-[11px] font-medium bg-agent-gemini/15 text-agent-gemini hover:bg-agent-gemini/25 transition-colors disabled:opacity-40">
              {creating ? "Creating…" : "Create Roundtable"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
