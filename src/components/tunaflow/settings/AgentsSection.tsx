import { useState, useEffect } from "react";
import { cn } from "@/lib/utils";
import { Plus, Trash2 } from "lucide-react";
import { useChatStore } from "@/stores/chatStore";
import { getSetting, setSetting } from "@/lib/appStore";
import { DEFAULT_PERSONAS } from "@/lib/defaultPersonas";
import { AgentAvatar } from "../AgentAvatar";
import type { AgentProfile } from "@/types";

const ENGINES = ["claude", "codex", "gemini", "opencode"] as const;

const DEFAULT_PROFILES: AgentProfile[] = [
  { id: "architect-claude", label: "Architect Claude", engine: "claude", defaultSkills: [] },
  { id: "reviewer-codex", label: "Reviewer Codex", engine: "codex", defaultSkills: [] },
  { id: "tester-gemini", label: "Tester Gemini", engine: "gemini", defaultSkills: [] },
  { id: "general-opencode", label: "General OpenCode", engine: "opencode", defaultSkills: [] },
];

export function AgentsSection() {
  const [profiles, setProfiles] = useState<AgentProfile[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [loaded, setLoaded] = useState(false);
  const engineModels = useChatStore((s) => s.engineModels);
  const skills = useChatStore((s) => s.skills);

  useEffect(() => {
    getSetting<AgentProfile[]>("agentProfiles", DEFAULT_PROFILES).then((p) => {
      setProfiles(p);
      if (p.length > 0) setSelectedId(p[0].id);
      setLoaded(true);
    });
  }, []);

  const save = (next: AgentProfile[]) => {
    setProfiles(next);
    setSetting("agentProfiles", next);
  };

  const selected = profiles.find((p) => p.id === selectedId);

  const updateField = <K extends keyof AgentProfile>(field: K, value: AgentProfile[K]) => {
    if (!selectedId) return;
    save(profiles.map((p) => p.id === selectedId ? { ...p, [field]: value } : p));
  };

  const addProfile = () => {
    const id = `agent-${Date.now()}`;
    const newProfile: AgentProfile = { id, label: "New Agent", engine: "claude", defaultSkills: [] };
    save([...profiles, newProfile]);
    setSelectedId(id);
  };

  const deleteProfile = (id: string) => {
    const next = profiles.filter((p) => p.id !== id);
    save(next);
    if (selectedId === id) setSelectedId(next[0]?.id ?? null);
  };

  const toggleSkill = (skillName: string) => {
    if (!selected) return;
    const has = selected.defaultSkills.includes(skillName);
    updateField("defaultSkills", has
      ? selected.defaultSkills.filter((s) => s !== skillName)
      : [...selected.defaultSkills, skillName]);
  };

  if (!loaded) return null;

  const currentModels = engineModels.filter((m) => m.engine === selected?.engine);

  return (
    <div>
      <h2 className="text-[14px] font-[550] text-foreground mb-1">Agent Profiles</h2>
      <p className="text-[12px] text-muted-foreground mb-4">에이전트 프로필을 관리합니다. 각 프로필은 엔진, 모델, 기본 스킬을 하나의 실행 단위로 묶습니다.</p>

      <div className="flex gap-4 min-h-[300px]">
        <div className="w-[180px] shrink-0 space-y-1">
          {profiles.map((p) => (
            <div key={p.id} onClick={() => setSelectedId(p.id)}
              className={cn("group flex items-center gap-2 px-3 py-2 rounded-lg cursor-pointer transition-colors",
                selectedId === p.id ? "bg-background text-foreground" : "text-muted-foreground hover:bg-background/50")}>
              <AgentAvatar engine={p.engine} size="sm" />
              <span className="flex-1 text-[12px] font-medium truncate">{p.label}</span>
              <button onClick={(e) => { e.stopPropagation(); deleteProfile(p.id); }}
                className="shrink-0 p-0.5 rounded opacity-0 group-hover:opacity-100 text-muted-foreground/30 hover:text-destructive transition-all">
                <Trash2 className="w-3 h-3" />
              </button>
            </div>
          ))}
          <button onClick={addProfile}
            className="flex items-center gap-2 px-3 py-2 rounded-lg text-[12px] text-muted-foreground/50 hover:text-foreground hover:bg-background/50 transition-colors w-full">
            <Plus className="w-3.5 h-3.5" /> New Agent
          </button>
        </div>

        {selected ? (
          <div className="flex-1 min-w-0 space-y-4">
            <div>
              <label className="text-[11px] text-muted-foreground mb-1 block">Name</label>
              <input value={selected.label} onChange={(e) => updateField("label", e.target.value)}
                className="w-full bg-background rounded-lg px-3 py-2 text-[13px] font-medium outline-none border border-border/30 focus:border-ring/40" />
            </div>

            <div>
              <label className="text-[11px] text-muted-foreground mb-1 block">Engine</label>
              <div className="flex gap-1.5">
                {ENGINES.map((eng) => (
                  <button key={eng}
                    onClick={() => {
                      if (!selectedId) return;
                      save(profiles.map((p) => p.id === selectedId ? { ...p, engine: eng, model: undefined } : p));
                    }}
                    className={cn(
                      "flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-[12px] font-medium transition-colors border",
                      selected.engine === eng ? "border-primary/40 bg-primary/8 text-foreground" : "border-border/20 text-muted-foreground hover:border-border/40"
                    )}>
                    <AgentAvatar engine={eng} size="xs" />
                    {eng}
                  </button>
                ))}
              </div>
            </div>

            <div>
              <label className="text-[11px] text-muted-foreground mb-1 block">Model</label>
              <select value={selected.model ?? ""} onChange={(e) => updateField("model", e.target.value || undefined)}
                className="w-full bg-background rounded-lg px-3 py-2 text-[12px] outline-none border border-border/30 focus:border-ring/40 cursor-pointer">
                <option value="">Engine default</option>
                {currentModels.map((m) => (
                  <option key={m.id} value={m.id}>{m.recommended ? "★ " : ""}{m.label}</option>
                ))}
              </select>
            </div>

            <div>
              <label className="text-[11px] text-muted-foreground mb-1 block">Persona</label>
              <select value={selected.personaId ?? ""} onChange={(e) => updateField("personaId", e.target.value || undefined)}
                className="w-full bg-background rounded-lg px-3 py-2 text-[12px] outline-none border border-border/30 focus:border-ring/40 cursor-pointer">
                <option value="">None</option>
                {DEFAULT_PERSONAS.map((p) => (
                  <option key={p.id} value={p.id}>{p.name} — {p.role}</option>
                ))}
              </select>
            </div>

            <div>
              <label className="text-[11px] text-muted-foreground mb-1 block">Default Skills ({selected.defaultSkills.length})</label>
              <div className="max-h-[150px] overflow-y-auto space-y-0.5 border border-border/30 rounded-lg p-2">
                {skills.length === 0 ? (
                  <p className="text-[11px] text-muted-foreground/30 py-2 text-center">No skills loaded</p>
                ) : skills.map((s) => (
                  <label key={s.name} className="flex items-center gap-2 px-2 py-1 rounded hover:bg-background/50 cursor-pointer">
                    <input type="checkbox" checked={selected.defaultSkills.includes(s.name)}
                      onChange={() => toggleSkill(s.name)} className="rounded border-border/40" />
                    <span className="text-[11px] text-foreground/70 truncate">{s.name}</span>
                  </label>
                ))}
              </div>
            </div>
          </div>
        ) : (
          <div className="flex-1 flex items-center justify-center text-muted-foreground/30 text-[13px]">
            Select or create an agent profile
          </div>
        )}
      </div>
    </div>
  );
}
