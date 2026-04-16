import { useState } from "react";
import { cn } from "@/lib/utils";
import { Plus, Trash2 } from "lucide-react";
import { useChatStore } from "@/stores/chatStore";
import { DEFAULT_PERSONAS } from "@/lib/defaultPersonas";
import { AgentAvatar } from "../AgentAvatar";
import type { AgentProfile } from "@/types";

// Keep in sync with ENGINE_CONFIGS (src/lib/engineConfig.ts). OpenCode removed;
// Ollama + LMStudio share the openai-compatible runtime.
const ENGINES = ["claude", "codex", "gemini", "ollama", "lmstudio"] as const;
const ENGINE_LABELS: Record<(typeof ENGINES)[number], string> = {
  claude: "Claude",
  codex: "Codex",
  gemini: "Gemini",
  ollama: "Ollama",
  lmstudio: "LM Studio",
};

export function AgentsSection() {
  const profiles = useChatStore((s) => s.agentProfiles);
  const saveProfiles = useChatStore((s) => s.saveProfiles);
  const [selectedId, setSelectedId] = useState<string | null>(profiles[0]?.id ?? null);
  const engineModels = useChatStore((s) => s.engineModels);
  const skills = useChatStore((s) => s.skills);

  const save = (next: AgentProfile[]) => {
    saveProfiles(next);
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
      ? selected.defaultSkills.filter((sk: string) => sk !== skillName)
      : [...selected.defaultSkills, skillName]);
  };

  if (profiles.length === 0) return null;

  const currentModels = engineModels.filter((m) => m.engine === selected?.engine);

  return (
    <div>
      <h2 className="text-tf-base font-[550] text-foreground mb-1">Agent Profiles</h2>
      <p className="text-tf-caption text-muted-foreground mb-4">에이전트 프로필을 관리합니다. 각 프로필은 엔진, 모델, 기본 스킬을 하나의 실행 단위로 묶습니다.</p>

      <div className="flex gap-4 min-h-[300px]">
        <div className="w-[180px] shrink-0 space-y-1">
          {profiles.map((p) => (
            <div key={p.id} onClick={() => setSelectedId(p.id)}
              className={cn("group flex items-center gap-2 px-3 py-2 rounded-lg cursor-pointer transition-colors",
                selectedId === p.id ? "bg-background text-foreground" : "text-muted-foreground hover:bg-background/50")}>
              <AgentAvatar engine={p.engine} size="sm" />
              <span className="flex-1 text-tf-caption font-medium truncate">{p.label}</span>
              <button onClick={(e) => { e.stopPropagation(); deleteProfile(p.id); }}
                className="shrink-0 p-0.5 rounded opacity-0 group-hover:opacity-100 text-muted-foreground/30 hover:text-destructive transition-all">
                <Trash2 className="w-3 h-3" />
              </button>
            </div>
          ))}
          <button onClick={addProfile}
            className="flex items-center gap-2 px-3 py-2 rounded-lg text-tf-caption text-muted-foreground/50 hover:text-foreground hover:bg-background/50 transition-colors w-full">
            <Plus className="w-3.5 h-3.5" /> New Agent
          </button>
        </div>

        {selected ? (
          <div className="flex-1 min-w-0 space-y-4">
            <div>
              <label className="text-tf-sm text-muted-foreground mb-1 block">Name</label>
              <input value={selected.label} onChange={(e) => updateField("label", e.target.value)}
                className="w-full bg-background rounded-lg px-3 py-2 text-tf-caption font-medium outline-none border border-border/30 focus:border-ring/40" />
            </div>

            <div>
              <label className="text-tf-sm text-muted-foreground mb-1 block">Engine</label>
              {/* Dropdown instead of icon row — more obvious what is selected
                   when 5+ engines share the strip. AgentAvatar next to the
                   select mirrors the active engine. */}
              <div className="flex items-center gap-2">
                <AgentAvatar engine={selected.engine} size="sm" />
                <select
                  value={selected.engine}
                  onChange={(e) => {
                    if (!selectedId) return;
                    const eng = e.target.value as (typeof ENGINES)[number];
                    const recommendedModel = engineModels.find((m) => m.engine === eng && m.recommended)?.id
                      ?? engineModels.find((m) => m.engine === eng)?.id;
                    save(profiles.map((p) => p.id === selectedId ? { ...p, engine: eng, model: recommendedModel } : p));
                  }}
                  className="flex-1 bg-background rounded-lg px-3 py-2 text-tf-caption outline-none border border-border/30 focus:border-ring/40 cursor-pointer"
                >
                  {ENGINES.map((eng) => (
                    <option key={eng} value={eng}>{ENGINE_LABELS[eng]}</option>
                  ))}
                </select>
              </div>
            </div>

            <div>
              <label className="text-tf-sm text-muted-foreground mb-1 block">Model</label>
              <select value={selected.model ?? ""} onChange={(e) => updateField("model", e.target.value || undefined)}
                className="w-full bg-background rounded-lg px-3 py-2 text-tf-caption outline-none border border-border/30 focus:border-ring/40 cursor-pointer">
                <option value="">Engine default</option>
                {currentModels.map((m) => (
                  <option key={m.id} value={m.id}>{m.recommended ? "★ " : ""}{m.label}</option>
                ))}
              </select>
            </div>

            <div>
              <label className="text-tf-sm text-muted-foreground mb-1 block">Persona (= Role)</label>
              <select value={selected.personaId ?? ""} onChange={(e) => updateField("personaId", e.target.value || undefined)}
                className="w-full bg-background rounded-lg px-3 py-2 text-tf-caption outline-none border border-border/30 focus:border-ring/40 cursor-pointer">
                <option value="">None</option>
                {DEFAULT_PERSONAS.map((p) => (
                  <option key={p.id} value={p.id}>{p.name} — {p.role}</option>
                ))}
              </select>
            </div>

            <div>
              <label className="text-tf-sm text-muted-foreground mb-1 block">Default Skills ({selected.defaultSkills.length})</label>
              <div className="max-h-[150px] overflow-y-auto space-y-0.5 border border-border/30 rounded-lg p-2">
                {skills.length === 0 ? (
                  <p className="text-tf-sm text-muted-foreground/30 py-2 text-center">No skills loaded</p>
                ) : skills.map((s) => (
                  <label key={s.name} className="flex items-center gap-2 px-2 py-1 rounded hover:bg-background/50 cursor-pointer">
                    <input type="checkbox" checked={selected.defaultSkills.includes(s.name)}
                      onChange={() => toggleSkill(s.name)} className="rounded border-border/40" />
                    <span className="text-tf-sm text-foreground/70 truncate">{s.name}</span>
                  </label>
                ))}
              </div>
            </div>
          </div>
        ) : (
          <div className="flex-1 flex items-center justify-center text-muted-foreground/30 text-tf-caption">
            Select or create an agent profile
          </div>
        )}
      </div>
    </div>
  );
}
