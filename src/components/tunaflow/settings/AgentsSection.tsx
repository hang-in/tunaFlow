import { useState, useEffect } from "react";
import { cn } from "@/lib/utils";
import { Plus, Trash2, CheckCircle2, AlertCircle, AlertTriangle } from "lucide-react";
import { useChatStore } from "@/stores/chatStore";
import { DEFAULT_PERSONAS } from "@/lib/defaultPersonas";
import { AgentAvatar } from "../AgentAvatar";
import type { AgentProfile } from "@/types";
import {
  loadRoleAssignments, saveRoleAssignments, inferRoleAssignments, evaluateCoverage,
  type RoleAssignments, type RoleCoverage, type RoleKey,
} from "@/lib/roleAssignments";
import {
  loadMetaConfig, saveMetaConfig, DEFAULT_CONFIG,
  type MetaAnalysisConfig, type MetaAnalysisEngine,
} from "@/lib/metaAnalysis";

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
  // Sync selectedId when profiles load asynchronously
  useEffect(() => {
    if (!selectedId && profiles.length > 0) setSelectedId(profiles[0].id);
  }, [profiles, selectedId]);
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

      <RoleCoveragePanel profiles={profiles} />
      <MetaAnalysisPanel />


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

// ─── Role Coverage Panel ─────────────────────────────────────────────────────

/** 4개 핵심 역할(Architect/Developer/Reviewers/Synthesizer) 에 대한 프로필 배정 상태.
 *  Review RT / Plan 승인 / RT 합성 직전에 이 값이 assertRoleReady() 로 검증된다. */
function RoleCoveragePanel({ profiles }: { profiles: AgentProfile[] }) {
  const [assignments, setAssignments] = useState<RoleAssignments>({ reviewers: [] });
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    let alive = true;
    loadRoleAssignments().then((a) => {
      if (!alive) return;
      // 저장값이 비어있고 profiles 가 있으면 auto-infer 제안 (실제 저장은 사용자 토글)
      if (!a.architect && !a.developer && a.reviewers.length === 0 && profiles.length > 0) {
        setAssignments(inferRoleAssignments(profiles));
      } else {
        setAssignments(a);
      }
      setLoaded(true);
    });
    return () => { alive = false; };
  }, [profiles.length]);

  const updateAndSave = (next: RoleAssignments) => {
    setAssignments(next);
    saveRoleAssignments(next);
  };

  const setSingle = (role: Exclude<RoleKey, "reviewers">, value: string) => {
    updateAndSave({ ...assignments, [role]: value || undefined });
  };

  const toggleReviewer = (profileId: string) => {
    const has = assignments.reviewers.includes(profileId);
    const next = has
      ? assignments.reviewers.filter((id) => id !== profileId)
      : [...assignments.reviewers, profileId];
    updateAndSave({ ...assignments, reviewers: next });
  };

  if (!loaded) return null;

  const coverage = evaluateCoverage(assignments, profiles);
  const allReady = coverage.every((c) => c.status === "ready");

  return (
    <div className={cn("mb-4 rounded-lg border p-3 space-y-2",
      allReady ? "border-status-approved/40 bg-status-approved/5" : "border-amber-500/40 bg-amber-500/5")}>
      <div className="flex items-center gap-2">
        {allReady ? <CheckCircle2 className="w-4 h-4 text-status-approved" /> : <AlertTriangle className="w-4 h-4 text-amber-500" />}
        <h3 className="text-tf-sm font-medium text-foreground flex-1">역할 커버리지</h3>
        <span className="text-tf-sm text-muted-foreground">
          {coverage.filter((c) => c.status === "ready").length}/{coverage.length} ready
        </span>
      </div>
      <p className="text-[10px] text-muted-foreground">
        Architect(설계), Developer(구현), Reviewer(검토 ≥2), Synthesizer(RT 합성) 역할에 프로필을 배정하면
        Review RT / Plan 승인 / RT 합성 시 설정 그대로 실행됩니다.
      </p>
      <div className="space-y-1.5">
        <RoleRow label="Architect" coverage={coverage[0]!} profiles={profiles}
          selectedId={assignments.architect ?? ""} onChange={(v) => setSingle("architect", v)} />
        <RoleRow label="Developer" coverage={coverage[1]!} profiles={profiles}
          selectedId={assignments.developer ?? ""} onChange={(v) => setSingle("developer", v)} />
        <ReviewersRow coverage={coverage[2]!} profiles={profiles}
          selectedIds={assignments.reviewers} onToggle={toggleReviewer} />
        <RoleRow label="Synthesizer" coverage={coverage[3]!} profiles={profiles}
          selectedId={assignments.synthesizer ?? ""} onChange={(v) => setSingle("synthesizer", v)} />
      </div>
    </div>
  );
}

function statusIcon(status: RoleCoverage["status"]) {
  if (status === "ready") return <CheckCircle2 className="w-3 h-3 text-status-approved" />;
  if (status === "model-unset") return <AlertTriangle className="w-3 h-3 text-amber-500" />;
  return <AlertCircle className="w-3 h-3 text-destructive" />;
}

function RoleRow({ label, coverage, profiles, selectedId, onChange }: {
  label: string;
  coverage: RoleCoverage;
  profiles: AgentProfile[];
  selectedId: string;
  onChange: (v: string) => void;
}) {
  return (
    <div className="flex items-center gap-2">
      <div className="w-[90px] flex items-center gap-1.5 shrink-0">
        {statusIcon(coverage.status)}
        <span className="text-tf-sm text-foreground">{label}</span>
      </div>
      <select value={selectedId} onChange={(e) => onChange(e.target.value)}
        className="flex-1 bg-background rounded px-2 py-1 text-tf-sm outline-none border border-border/30 focus:border-ring/40">
        <option value="">— 선택 안 됨 —</option>
        {profiles.map((p) => (
          <option key={p.id} value={p.id}>{p.label} ({p.engine}{p.model ? `/${p.model}` : ""})</option>
        ))}
      </select>
    </div>
  );
}

function ReviewersRow({ coverage, profiles, selectedIds, onToggle }: {
  coverage: RoleCoverage;
  profiles: AgentProfile[];
  selectedIds: string[];
  onToggle: (id: string) => void;
}) {
  return (
    <div className="flex items-start gap-2">
      <div className="w-[90px] flex items-center gap-1.5 shrink-0 pt-1">
        {statusIcon(coverage.status)}
        <span className="text-tf-sm text-foreground">Reviewers</span>
      </div>
      <div className="flex-1 flex flex-wrap gap-1.5">
        {profiles.map((p) => {
          const checked = selectedIds.includes(p.id);
          return (
            <button key={p.id} onClick={() => onToggle(p.id)}
              className={cn("px-2 py-0.5 rounded text-[11px] transition-colors",
                checked ? "bg-primary/20 text-foreground border border-primary/40" : "bg-background text-muted-foreground border border-border/30 hover:text-foreground")}>
              {p.label}
            </button>
          );
        })}
        {profiles.length === 0 && <span className="text-[10px] text-muted-foreground">프로필이 없습니다</span>}
      </div>
    </div>
  );
}

// ─── Meta Analysis (Tier 2) Panel ───────────────────────────────────────────

const ENGINE_OPTIONS: { value: MetaAnalysisEngine; label: string; hint: string }[] = [
  { value: "off",           label: "끔",                  hint: "메타 자동 분석 비활성화" },
  { value: "auto",          label: "자동",                hint: "프로젝트 스택 기반 자동 선택" },
  { value: "claude-haiku",  label: "Claude Haiku",        hint: "JSON/한국어 요약 안정적, 저렴" },
  { value: "gemini-flash",  label: "Gemini Flash 2.5",    hint: "대용량 input 저렴, 빠름" },
];

function MetaAnalysisPanel() {
  const [cfg, setCfg] = useState<MetaAnalysisConfig>(DEFAULT_CONFIG);
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    let alive = true;
    loadMetaConfig().then((c) => { if (alive) { setCfg(c); setLoaded(true); } });
    return () => { alive = false; };
  }, []);

  const updateAndSave = (patch: Partial<MetaAnalysisConfig>) => {
    const next = { ...cfg, ...patch };
    setCfg(next);
    saveMetaConfig(next);
  };

  if (!loaded) return null;

  const disabled = cfg.engine === "off";

  return (
    <div className="mb-4 rounded-lg border border-border/30 bg-background/50 p-3 space-y-2.5">
      <div>
        <h3 className="text-tf-sm font-medium text-foreground">메타 에이전트 자동 분석 (Tier 2)</h3>
        <p className="text-[10px] text-muted-foreground mt-0.5">
          주간 요약, 실패 패턴 분석, artifacts 요약 등을 저비용 엔진으로 자동 생성합니다.
          읽기 전용 — 제안만 하며 실행은 사용자 승인 필요.
        </p>
      </div>

      <div className="flex items-center gap-2">
        <span className="text-[11px] text-muted-foreground w-[60px] shrink-0">엔진</span>
        <select
          value={cfg.engine}
          onChange={(e) => updateAndSave({ engine: e.target.value as MetaAnalysisEngine })}
          className="flex-1 bg-background rounded px-2 py-1 text-[11px] outline-none border border-border/30 focus:border-ring/40"
        >
          {ENGINE_OPTIONS.map((opt) => (
            <option key={opt.value} value={opt.value}>{opt.label} — {opt.hint}</option>
          ))}
        </select>
      </div>

      <label className={cn("flex items-center gap-2 cursor-pointer", disabled && "opacity-40 pointer-events-none")}>
        <input
          type="checkbox"
          checked={cfg.autoTrigger && !disabled}
          onChange={(e) => updateAndSave({ autoTrigger: e.target.checked })}
          className="rounded border-border/40"
        />
        <span className="text-[11px] text-foreground">자동 트리거</span>
        <span className="text-[10px] text-muted-foreground">— 임계값 도달 시 분석 자동 실행</span>
      </label>

      {!disabled && cfg.autoTrigger && (
        <div className="pl-4 grid grid-cols-2 gap-x-3 gap-y-1.5 text-[10px]">
          <label className="flex items-center gap-1.5">
            <span className="text-muted-foreground w-[90px] truncate">주간 요약 (pass 누적)</span>
            <input
              type="number" min={1} max={100}
              value={cfg.thresholds.reviewPassedCount}
              onChange={(e) => updateAndSave({
                thresholds: { ...cfg.thresholds, reviewPassedCount: parseInt(e.target.value, 10) || 10 },
              })}
              className="w-12 bg-background rounded px-1 py-0.5 text-[10px] border border-border/30"
            />
          </label>
          <label className="flex items-center gap-1.5">
            <span className="text-muted-foreground w-[90px] truncate">실패 패턴 (fail 누적)</span>
            <input
              type="number" min={1} max={50}
              value={cfg.thresholds.reviewFailedCount}
              onChange={(e) => updateAndSave({
                thresholds: { ...cfg.thresholds, reviewFailedCount: parseInt(e.target.value, 10) || 5 },
              })}
              className="w-12 bg-background rounded px-1 py-0.5 text-[10px] border border-border/30"
            />
          </label>
          <label className="flex items-center gap-1.5">
            <span className="text-muted-foreground w-[90px] truncate">artifact 요약 (누적)</span>
            <input
              type="number" min={1} max={100}
              value={cfg.thresholds.artifactCount}
              onChange={(e) => updateAndSave({
                thresholds: { ...cfg.thresholds, artifactCount: parseInt(e.target.value, 10) || 10 },
              })}
              className="w-12 bg-background rounded px-1 py-0.5 text-[10px] border border-border/30"
            />
          </label>
          <label className="flex items-center gap-1.5">
            <span className="text-muted-foreground w-[90px] truncate">idle 일수</span>
            <input
              type="number" min={1} max={90}
              value={cfg.thresholds.idleDays}
              onChange={(e) => updateAndSave({
                thresholds: { ...cfg.thresholds, idleDays: parseInt(e.target.value, 10) || 7 },
              })}
              className="w-12 bg-background rounded px-1 py-0.5 text-[10px] border border-border/30"
            />
          </label>
        </div>
      )}
    </div>
  );
}
