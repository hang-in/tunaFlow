import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { copyToClipboard } from "@/lib/clipboard";
import { invoke } from "@tauri-apps/api/core";
import { cn, errorMessage } from "@/lib/utils";
import { X, Search, FileText, ChevronRight, Copy, Send, Archive, ChevronDown } from "lucide-react";
import { useChatStore } from "@/stores/chatStore";
import { getSetting, setSetting } from "@/lib/appStore";
import { SKILL_SETS, expandSkillRefs } from "@/lib/skillSets";

// ─── Workflow Skills Config ──────────────────────────────────────────────────

function WorkflowSkillsConfig() {
  const { t } = useTranslation("settings");
  const WORKFLOW_PHASES = [
    { key: "chat", label: "Chat / Planning", desc: t("runtime.workflow_skills.phase.chat_desc") },
    { key: "implementation", label: "Implementation", desc: t("runtime.workflow_skills.phase.implementation_desc") },
    { key: "review", label: "Review", desc: t("runtime.workflow_skills.phase.review_desc") },
  ] as const;
  const workflowSkills = useChatStore((s) => s.workflowSkills);
  const saveWorkflowSkills = useChatStore((s) => s.saveWorkflowSkills);
  const [expandedPhase, setExpandedPhase] = useState<string | null>(null);

  const toggleRef = (phase: string, ref: string) => {
    const current = workflowSkills[phase] ?? [];
    const updated = current.includes(ref)
      ? current.filter((s) => s !== ref)
      : [...current, ref];
    saveWorkflowSkills({ ...workflowSkills, [phase]: updated });
  };

  return (
    <div className="rounded-lg border border-border/30 bg-background/50 p-4 space-y-3">
      <div>
        <h3 className="text-[13px] font-medium text-foreground">{t("runtime.workflow_skills.heading")}</h3>
        <p className="text-[11px] text-muted-foreground/60 mt-0.5">{t("runtime.workflow_skills.description")}</p>
      </div>
      <div className="space-y-3">
        {WORKFLOW_PHASES.map(({ key, label, desc }) => {
          const refs = workflowSkills[key] ?? [];
          const expandedCount = expandSkillRefs(refs).length;
          const isExpanded = expandedPhase === key;
          return (
            <div key={key} className="space-y-1.5">
              <button
                className="flex items-center gap-2 w-full text-left"
                onClick={() => setExpandedPhase(isExpanded ? null : key)}
              >
                <ChevronDown className={cn("w-3 h-3 text-muted-foreground/40 transition-transform", isExpanded && "rotate-180")} />
                <span className="text-[12px] font-medium text-foreground/80">{label}</span>
                <span className="text-[10px] text-muted-foreground/40">{desc}</span>
                {expandedCount > 0 && (
                  <span className="text-[9px] px-1.5 py-0.5 rounded bg-primary/10 text-primary font-medium ml-auto">{expandedCount} skills</span>
                )}
              </button>
              {isExpanded && (
                <div className="pl-5 space-y-2">
                  {/* Skill Sets */}
                  <div className="flex flex-wrap gap-1.5">
                    {SKILL_SETS.map((set) => {
                      const setRef = `set:${set.id}`;
                      const isActive = refs.includes(setRef);
                      return (
                        <button
                          key={set.id}
                          onClick={() => toggleRef(key, setRef)}
                          title={`${set.description}\n${set.skills.join(", ")}`}
                          className={cn(
                            "px-2.5 py-1 rounded-md text-[11px] transition-colors border",
                            isActive
                              ? "bg-primary/15 text-primary border-primary/30 font-medium"
                              : "bg-accent/30 text-muted-foreground/60 border-transparent hover:text-foreground/80 hover:border-border/30"
                          )}
                        >
                          {set.label}
                          <span className="ml-1 text-[9px] opacity-50">{set.skills.length}</span>
                        </button>
                      );
                    })}
                  </div>
                  {/* Active refs summary */}
                  {refs.length > 0 && (
                    <div className="text-[10px] text-muted-foreground/40">
                      {refs.map((r) => r.startsWith("set:") ? r.slice(4) : r).join(", ")}
                    </div>
                  )}
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}

// ─── Context Budget Control ──────────────────────────────────────────────────

const SECTION_POLICY: Record<string, string[]> = {
  lite: ["Project", "Context"],
  standard: ["Project", "Context", "Plan", "Findings", "Artifacts"],
  full: ["Project", "Context", "Plan", "Findings", "Artifacts", "Skills", "rawq", "Cross-session"],
};

const BUDGET_MIN = 20_000;
const BUDGET_MAX = 120_000;
const BUDGET_STEP = 10_000;
const BUDGET_DEFAULT = 60_000;

// ─── Insight Agent Config ───────────────────────────────────────────────────

interface InsightPreset {
  id: string;
  label: string;
  desc: string;
  engine: string;
  model: string;
  systemPrompt: string;
}

const INSIGHT_PRESETS_DATA: Omit<InsightPreset, "label" | "desc">[] = [
  {
    id: "balanced",
    engine: "claude",
    model: "",
    systemPrompt: `You are a senior code quality analyst performing a targeted review.

## Rules
- ONLY report issues verifiable from the provided code snippets
- NEVER hallucinate file paths or line numbers — if uncertain, omit them
- Each finding must reference a specific snippet from the input data
- Assign severity based on production impact:
  - critical: data loss, security breach, crash in production
  - major: incorrect behavior, resource leak, missing error handling
  - minor: code smell, inconsistency, minor inefficiency
  - info: style, documentation gap, potential improvement
- Estimate files to change conservatively (round up)
- Respond in Korean for descriptions, English for technical terms`,
  },
  {
    id: "thorough",
    engine: "claude",
    model: "",
    systemPrompt: `You are a principal software engineer conducting a thorough code quality audit.

## Analysis Framework
1. For each code snippet, identify the SPECIFIC anti-pattern or vulnerability
2. Explain the CONCRETE risk — what could go wrong in production
3. Provide a MINIMAL fix suggestion (pseudocode, not full implementation)
4. Cross-reference related snippets if they share the same root cause

## Severity Criteria (be strict)
- critical: Exploitable vulnerability, data corruption, or guaranteed crash path
- major: Likely bug under realistic conditions, significant performance regression
- minor: Code smell that increases maintenance cost but won't cause incidents
- info: Improvement opportunity with no current risk

## Anti-hallucination
- ONLY reference files and line numbers present in the provided snippets
- If a snippet is ambiguous, note the uncertainty in your finding description
- Prefer fewer high-confidence findings over many speculative ones
- Group related issues into a single finding when they share root cause

Respond in Korean for descriptions, keep technical terms in English.`,
  },
  {
    id: "security",
    engine: "claude",
    model: "",
    systemPrompt: `You are a security engineer conducting a focused vulnerability assessment.

## OWASP Top 10 Checklist (prioritize these)
1. Injection (SQL, command, LDAP, XSS)
2. Broken Authentication / Session Management
3. Sensitive Data Exposure (secrets, tokens, PII in logs)
4. XML External Entities (XXE)
5. Broken Access Control
6. Security Misconfiguration (default credentials, verbose errors)
7. Cross-Site Scripting (XSS) — stored, reflected, DOM-based
8. Insecure Deserialization
9. Using Components with Known Vulnerabilities
10. Insufficient Logging & Monitoring

## Rules
- ONLY report issues verifiable from the provided snippets
- For each finding, identify the CWE number if applicable
- Severity = exploitability × impact (CVSS-like reasoning)
- critical: Remotely exploitable without authentication
- major: Exploitable with authenticated access or local context
- minor: Requires specific conditions unlikely in normal operation
- info: Defense-in-depth recommendation

Respond in Korean for descriptions, English for technical terms and CWE references.`,
  },
  {
    id: "gemini",
    engine: "gemini",
    model: "",
    systemPrompt: `You are a senior code quality analyst performing a targeted review.

## Rules
- ONLY report issues verifiable from the provided code snippets
- NEVER hallucinate file paths or line numbers — if uncertain, omit them
- Each finding must reference a specific snippet from the input data
- Assign severity: critical (production crash/security) > major (bug/leak) > minor (smell) > info (style)
- Estimate files to change conservatively
- Respond in Korean for descriptions, English for technical terms`,
  },
];

const DEFAULT_INSIGHT_CONFIG = {
  engine: "claude",
  model: "",
  systemPrompt: INSIGHT_PRESETS_DATA[0].systemPrompt,
  presetId: "balanced",
};

const INSIGHT_PRESET_I18N = {
  balanced: { labelKey: "runtime.insight_agent.preset.balanced_claude_label", descKey: "runtime.insight_agent.preset.balanced_claude_desc" },
  thorough: { labelKey: "runtime.insight_agent.preset.thorough_label", descKey: "runtime.insight_agent.preset.thorough_desc" },
  security: { labelKey: "runtime.insight_agent.preset.security_label", descKey: "runtime.insight_agent.preset.security_desc" },
  gemini: { labelKey: "runtime.insight_agent.preset.balanced_gemini_label", descKey: "runtime.insight_agent.preset.balanced_gemini_desc" },
} as const;

function InsightAgentConfig() {
  const { t } = useTranslation("settings");
  const INSIGHT_PRESETS: InsightPreset[] = INSIGHT_PRESETS_DATA.map((p) => {
    const i18n = INSIGHT_PRESET_I18N[p.id as keyof typeof INSIGHT_PRESET_I18N];
    return {
      ...p,
      label: i18n ? t(i18n.labelKey) : p.id,
      desc: i18n ? t(i18n.descKey) : "",
    };
  });
  const [config, setConfig] = useState(DEFAULT_INSIGHT_CONFIG);
  const [expanded, setExpanded] = useState(false);
  const engineModels = useChatStore((s) => s.engineModels);

  useEffect(() => {
    getSetting("insightAgentConfig", DEFAULT_INSIGHT_CONFIG).then(setConfig);
  }, []);

  const update = (patch: Partial<typeof config>) => {
    const next = { ...config, ...patch };
    setConfig(next);
    setSetting("insightAgentConfig", next);
  };

  const applyPreset = (preset: InsightPreset) => {
    update({
      engine: preset.engine,
      model: preset.model,
      systemPrompt: preset.systemPrompt,
      presetId: preset.id,
    });
  };

  const modelsForEngine = engineModels.filter((m) => m.engine === config.engine);

  return (
    <div className="rounded-lg border border-border/30 bg-background/50 p-4 space-y-3">
      <div>
        <h3 className="text-[13px] font-medium text-foreground">Insight Agent</h3>
        <p className="text-[11px] text-muted-foreground/60 mt-0.5">{t("runtime.insight_agent.description")}</p>
      </div>

      {/* Preset buttons */}
      <div className="space-y-1.5">
        <span className="text-[10px] text-muted-foreground/50">{t("runtime.insight_agent.preset_label")}</span>
        <div className="flex flex-wrap gap-1.5">
          {INSIGHT_PRESETS.map((p) => (
            <button
              key={p.id}
              onClick={() => applyPreset(p)}
              className={cn(
                "text-[10px] px-2 py-1 rounded border transition-colors",
                config.presetId === p.id
                  ? "border-accent bg-accent/10 text-accent"
                  : "border-border/30 text-muted-foreground/60 hover:text-foreground hover:border-border/50",
              )}
              title={p.desc}
            >
              {p.label}
            </button>
          ))}
        </div>
      </div>

      {/* Engine + Model */}
      <div className="flex gap-3">
        <div className="space-y-1 flex-1">
          <span className="text-[10px] text-muted-foreground/50">{t("runtime.insight_agent.engine_label")}</span>
          <select
            value={config.engine}
            onChange={(e) => update({ engine: e.target.value, model: "", presetId: "custom" })}
            className="w-full text-[11px] bg-transparent border border-border/30 rounded px-2 py-1 text-foreground"
          >
            <option value="claude">Claude</option>
            <option value="codex">Codex</option>
            <option value="gemini">Gemini</option>
            <option value="ollama">Ollama</option>
            <option value="lmstudio">LM Studio</option>
          </select>
        </div>
        <div className="space-y-1 flex-1">
          <span className="text-[10px] text-muted-foreground/50">{t("runtime.insight_agent.model_label")}</span>
          <select
            value={config.model}
            onChange={(e) => update({ model: e.target.value, presetId: "custom" })}
            className="w-full text-[11px] bg-transparent border border-border/30 rounded px-2 py-1 text-foreground"
          >
            <option value="">Engine default</option>
            {modelsForEngine.map((m) => (
              <option key={m.id} value={m.id}>
                {m.recommended ? "★ " : ""}{m.label}
              </option>
            ))}
          </select>
        </div>
      </div>

      {/* System prompt (collapsible) */}
      <div className="space-y-1">
        <button
          onClick={() => setExpanded(!expanded)}
          className="flex items-center gap-1 text-[10px] text-muted-foreground/50 hover:text-foreground"
        >
          {expanded ? <ChevronDown className="w-3 h-3" /> : <ChevronRight className="w-3 h-3" />}
          {t("runtime.insight_agent.system_prompt")} {config.presetId !== "custom" && `(${config.presetId})`}
        </button>
        {expanded && (
          <textarea
            value={config.systemPrompt}
            onChange={(e) => update({ systemPrompt: e.target.value, presetId: "custom" })}
            rows={10}
            className="w-full text-[10px] font-mono bg-muted/20 border border-border/20 rounded p-2 text-foreground/80 resize-y leading-relaxed"
          />
        )}
      </div>
    </div>
  );
}

function ContextBudgetControl() {
  const { t } = useTranslation("settings");
  const CONTEXT_MODES = [
    { id: "auto", label: "Auto", desc: t("runtime.context_budget.modes.auto_desc") },
    { id: "lite", label: "Lite", desc: t("runtime.context_budget.modes.lite_desc") },
    { id: "standard", label: "Standard", desc: t("runtime.context_budget.modes.standard_desc") },
    { id: "full", label: "Full", desc: t("runtime.context_budget.modes.full_desc") },
  ] as const;
  const [config, setConfig] = useState({ mode: "auto", totalCap: BUDGET_DEFAULT });
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    getSetting<{ mode: string; totalCap: number }>("contextBudgetConfig", { mode: "auto", totalCap: BUDGET_DEFAULT }).then((c) => {
      setConfig(c);
      setLoaded(true);
    });
  }, []);

  const update = (patch: Partial<typeof config>) => {
    const next = { ...config, ...patch };
    setConfig(next);
    setSetting("contextBudgetConfig", next);
  };

  if (!loaded) return null;

  const policyMode = config.mode === "auto" ? "full" : config.mode;
  const sections = SECTION_POLICY[policyMode] || SECTION_POLICY.full;

  return (
    <div className="rounded-lg border border-border/30 bg-background/50 p-4 space-y-4">
      <h3 className="text-[13px] font-medium text-foreground">Context Budget</h3>
      <div className="space-y-1.5">
        <label className="text-[11px] text-muted-foreground">Context Mode</label>
        <div className="flex gap-1">
          {CONTEXT_MODES.map((m) => (
            <button key={m.id} onClick={() => update({ mode: m.id })}
              className={cn("px-3 py-1.5 rounded-lg text-[11px] font-medium transition-colors border",
                config.mode === m.id ? "border-primary/40 bg-primary/8 text-foreground" : "border-border/20 text-muted-foreground hover:border-border/40")}>
              {m.label}
            </button>
          ))}
        </div>
        <p className="text-[10px] text-muted-foreground/50">{CONTEXT_MODES.find((m) => m.id === config.mode)?.desc}</p>
      </div>
      <div className="space-y-1.5">
        <div className="flex items-center justify-between">
          <label className="text-[11px] text-muted-foreground">Total Budget Cap</label>
          <span className="text-[11px] font-mono text-foreground/80">{(config.totalCap / 1000).toFixed(0)}k chars</span>
        </div>
        <input type="range" min={BUDGET_MIN} max={BUDGET_MAX} step={BUDGET_STEP} value={config.totalCap}
          onChange={(e) => update({ totalCap: Number(e.target.value) })} className="w-full accent-primary h-1.5 cursor-pointer" />
        <div className="flex justify-between text-[9px] text-muted-foreground/30">
          <span>{BUDGET_MIN / 1000}k</span><span>{BUDGET_DEFAULT / 1000}k (default)</span><span>{BUDGET_MAX / 1000}k</span>
        </div>
      </div>
      <div className="space-y-1.5">
        <label className="text-[11px] text-muted-foreground">{config.mode === "auto" ? t("runtime.context_budget.included_sections_auto") : t("runtime.context_budget.included_sections")}</label>
        <div className="flex flex-wrap gap-1">
          {["Project", "Context", "Persona", "Plan", "Findings", "Artifacts", "Skills", "rawq", "Cross-session", "Thread"].map((sec) => (
            <span key={sec} className={cn("px-1.5 py-0.5 rounded text-[9px] font-medium",
              sections.includes(sec) || sec === "Persona" || sec === "Thread" ? "bg-accent/60 text-foreground/60" : "bg-muted/30 text-muted-foreground/25 line-through")}>{sec}</span>
          ))}
        </div>
        <p className="text-[9px] text-muted-foreground/30">{t("runtime.context_budget.persona_thread_note")}</p>
      </div>
    </div>
  );
}

// ─── Context Hub Panel ───────────────────────────────────────────────────────

interface HubHealth { available: boolean; version: string | null; message: string }
interface HubSearchResult { id: string; title: string; source: string; snippet: string; score: number }
interface HubDocument { id: string; title: string; content: string; source: string }

function ContextHubPanel({ hubHealth }: { hubHealth: HubHealth | null }) {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<HubSearchResult[]>([]);
  const [searching, setSearching] = useState(false);
  const [selectedDoc, setSelectedDoc] = useState<HubDocument | null>(null);
  const [loadingDoc, setLoadingDoc] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [sentToContext, setSentToContext] = useState(false);

  const isAvailable = hubHealth?.available ?? false;

  const handleSearch = async () => {
    if (!query.trim() || !isAvailable) return;
    setSearching(true); setError(null); setResults([]); setSelectedDoc(null);
    try { setResults(await invoke<HubSearchResult[]>("context_hub_search", { query: query.trim(), sourceFilter: null, limit: 10 })); }
    catch (e) { setError(errorMessage(e)); }
    finally { setSearching(false); }
  };

  const handleGet = async (id: string) => {
    setLoadingDoc(true); setError(null);
    try { setSelectedDoc(await invoke<HubDocument>("context_hub_get", { documentId: id })); }
    catch (e) { setError(errorMessage(e)); }
    finally { setLoadingDoc(false); }
  };

  const handleCopy = () => { if (!selectedDoc) return; copyToClipboard(selectedDoc.content); setCopied(true); setTimeout(() => setCopied(false), 1500); };
  const handleSendToContext = () => {
    if (!selectedDoc) return;
    useChatStore.getState().setHandoffSource({ type: "knowledge", content: `[Knowledge: ${selectedDoc.title || selectedDoc.id} (${selectedDoc.source})]\n\n${selectedDoc.content}` });
    setSentToContext(true); setTimeout(() => setSentToContext(false), 2000);
  };
  const handleSaveAsArtifact = async () => {
    if (!selectedDoc) return;
    const { selectedConversationId } = useChatStore.getState();
    if (!selectedConversationId) return;
    try { await invoke("create_artifact", { conversationId: selectedConversationId, artifactType: "notes", title: selectedDoc.title || selectedDoc.id, content: selectedDoc.content }); }
    catch (e) { setError(errorMessage(e)); }
  };

  return (
    <div className="rounded-lg border border-border/30 bg-background/50 p-4 space-y-3">
      <div className="flex items-center gap-2">
        <h3 className="text-[13px] font-medium text-foreground flex-1">context-hub — Knowledge Sources</h3>
        {hubHealth && <span className={cn("text-[11px] px-2 py-0.5 rounded-md font-medium", isAvailable ? "text-status-approved bg-status-approved/10" : "text-muted-foreground bg-muted")}>{isAvailable ? "ready" : "unavailable"}</span>}
      </div>
      {hubHealth ? (
        <div className="space-y-1 text-[12px]">
          <div className="flex items-center gap-2"><span className="text-muted-foreground w-[80px]">Status</span><span className="text-foreground/80">{hubHealth.message}</span></div>
          <div className="flex items-center gap-2"><span className="text-muted-foreground w-[80px]">Policy</span><span className="text-foreground/80">bundled / local / private only</span></div>
        </div>
      ) : <p className="text-[12px] text-muted-foreground/50">Checking...</p>}

      <div className="flex gap-1.5">
        <div className="flex-1 relative">
          <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-muted-foreground/40" />
          <input value={query} onChange={(e) => setQuery(e.target.value)} onKeyDown={(e) => e.key === "Enter" && handleSearch()}
            placeholder={isAvailable ? "Search knowledge sources..." : "context-hub not available"} disabled={!isAvailable}
            className="w-full bg-background rounded-lg pl-8 pr-3 py-2 text-[12px] outline-none border border-border/30 focus:border-ring/40 disabled:opacity-40 disabled:cursor-not-allowed" />
        </div>
        <button onClick={handleSearch} disabled={!isAvailable || !query.trim() || searching}
          className="px-3 py-2 rounded-lg text-[11px] font-medium bg-primary/10 text-primary hover:bg-primary/20 transition-colors disabled:opacity-30 disabled:cursor-not-allowed">
          {searching ? "..." : "Search"}
        </button>
      </div>

      {error && <p className="text-[11px] text-destructive/70 px-1">{error}</p>}

      {results.length > 0 && (
        <div className="space-y-1 max-h-[200px] overflow-y-auto">
          {results.map((r) => (
            <button key={r.id} onClick={() => handleGet(r.id)}
              className={cn("w-full text-left rounded-md border px-3 py-2 transition-colors", selectedDoc?.id === r.id ? "border-primary/40 bg-primary/5" : "border-border/20 hover:border-border/40 hover:bg-accent/20")}>
              <div className="flex items-center gap-2">
                <FileText className="w-3.5 h-3.5 text-muted-foreground/50 shrink-0" />
                <span className="text-[12px] font-medium text-foreground/80 flex-1 truncate">{r.title || r.id}</span>
                <span className="text-[9px] text-muted-foreground/40 font-mono">{Math.round(r.score * 100)}%</span>
                <ChevronRight className="w-3 h-3 text-muted-foreground/30" />
              </div>
              <div className="flex items-center gap-2 mt-0.5">
                <span className="text-[9px] text-primary/50 bg-primary/5 px-1 rounded">{r.source}</span>
                {r.snippet && <span className="text-[10px] text-muted-foreground/40 truncate">{r.snippet}</span>}
              </div>
            </button>
          ))}
        </div>
      )}

      {loadingDoc && <p className="text-[11px] text-muted-foreground/50 px-1">Loading document...</p>}
      {selectedDoc && (
        <div className="rounded-md border border-border/30 bg-background p-3 space-y-2">
          <div className="flex items-center gap-2">
            <FileText className="w-3.5 h-3.5 text-primary/60 shrink-0" />
            <span className="text-[12px] font-medium text-foreground/80 flex-1">{selectedDoc.title || selectedDoc.id}</span>
            <span className="text-[9px] text-primary/50 bg-primary/5 px-1 rounded">{selectedDoc.source}</span>
            <button onClick={() => setSelectedDoc(null)} className="p-0.5 text-muted-foreground/40 hover:text-foreground transition-colors"><X className="w-3 h-3" /></button>
          </div>
          <pre className="text-[11px] text-foreground/70 whitespace-pre-wrap max-h-[300px] overflow-y-auto bg-accent/30 rounded p-2 font-mono leading-relaxed">{selectedDoc.content}</pre>
          <div className="flex items-center gap-1.5 pt-1 border-t border-border/20">
            <button onClick={handleCopy} className="flex items-center gap-1 px-2 py-1 rounded text-[10px] text-muted-foreground hover:text-foreground hover:bg-accent/30 transition-colors"><Copy className="w-3 h-3" />{copied ? "Copied!" : "Copy"}</button>
            <button onClick={handleSendToContext} className="flex items-center gap-1 px-2 py-1 rounded text-[10px] text-primary/70 hover:text-primary hover:bg-primary/10 transition-colors"><Send className="w-3 h-3" />{sentToContext ? "Queued" : "Send to Context"}</button>
            <button onClick={handleSaveAsArtifact} className="flex items-center gap-1 px-2 py-1 rounded text-[10px] text-muted-foreground hover:text-foreground hover:bg-accent/30 transition-colors"><Archive className="w-3 h-3" />Save as Artifact</button>
          </div>
        </div>
      )}
    </div>
  );
}

// ─── Runtime Section (main export) ───────────────────────────────────────────

// ─── PTY Mode Toggle ────────────────────────────────────────────────────────

function PtyModeToggle() {
  const { t } = useTranslation("settings");
  const [ptyEnabled, setPtyEnabled] = useState(true);

  useEffect(() => {
    getSetting<boolean>("ptyEnabled", true).then(setPtyEnabled);
  }, []);

  const toggle = async () => {
    const next = !ptyEnabled;
    setPtyEnabled(next);
    await setSetting("ptyEnabled", next);
    if (!next) {
      // Kill active PTY sessions when disabling
      try {
        await invoke("pty_kill_all");
        const { usePtyStore } = await import("@/stores/ptyStore");
        usePtyStore.getState().clearAllSessions();
      } catch { /* ok */ }
    }
  };

  return (
    <div className="rounded-lg border border-border/30 bg-background/50 p-4 space-y-2">
      <div className="flex items-center gap-2">
        <h3 className="text-[13px] font-medium text-foreground flex-1">PTY Interactive Mode</h3>
        <button
          onClick={toggle}
          className={cn(
            "text-[11px] px-2.5 py-1 rounded-md font-medium transition-colors",
            ptyEnabled
              ? "bg-status-approved/10 text-status-approved hover:bg-status-approved/20"
              : "bg-muted text-muted-foreground hover:bg-muted/80"
          )}
        >
          {ptyEnabled ? "Enabled" : "Disabled"}
        </button>
      </div>
      <div className="space-y-1 text-[12px]">
        <div className="flex items-center gap-2">
          <span className="text-muted-foreground w-[80px]">Mode</span>
          <span className="text-foreground/80">{ptyEnabled ? "PTY (stateful, multi-turn)" : "CLI -p (stateless, per-message)"}</span>
        </div>
        <p className="text-[11px] text-muted-foreground/50 mt-1">
          {t("runtime.pty.hint")}
        </p>
      </div>
    </div>
  );
}

export function RuntimeSection() {
  const { t } = useTranslation("settings");
  const rawqStatus = useChatStore((s) => s.rawqStatus);
  const engineModels = useChatStore((s) => s.engineModels);
  const loadEngineModels = useChatStore((s) => s.loadEngineModels);
  const [refreshing, setRefreshing] = useState(false);
  const [hubHealth, setHubHealth] = useState<HubHealth | null>(null);

  useEffect(() => { invoke<HubHealth>("context_hub_health").then(setHubHealth).catch((e) => console.debug("[hub-health]", e)); }, []);

  const handleRefreshModels = async () => { setRefreshing(true); await loadEngineModels(true); setRefreshing(false); };

  const engineGroups = engineModels.reduce<Record<string, number>>((acc, m) => { acc[m.engine] = (acc[m.engine] ?? 0) + 1; return acc; }, {});

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-[14px] font-[550] text-foreground mb-1">{t("runtime.heading")}</h2>
        <p className="text-[12px] text-muted-foreground mb-4">{t("runtime.description")}</p>
      </div>

      {/* rawq */}
      <div className="rounded-lg border border-border/30 bg-background/50 p-4 space-y-2">
        <div className="flex items-center gap-2">
          <h3 className="text-[13px] font-medium text-foreground flex-1">rawq — Code Search Engine</h3>
          {rawqStatus && <span className={cn("text-[11px] px-2 py-0.5 rounded-md font-medium",
            rawqStatus.status === "ready" || rawqStatus.status === "built" ? "text-status-approved bg-status-approved/10"
            : rawqStatus.status === "indexing" ? "text-primary bg-primary/10" : "text-muted-foreground bg-muted")}>{rawqStatus.status}</span>}
        </div>
        {rawqStatus ? (
          <div className="space-y-1 text-[12px]">
            <div className="flex items-center gap-2"><span className="text-muted-foreground w-[80px]">Status</span><span className="text-foreground/80">{rawqStatus.message}</span></div>
            {rawqStatus.files != null && <div className="flex items-center gap-2"><span className="text-muted-foreground w-[80px]">Files</span><span className="text-foreground/80">{rawqStatus.files.toLocaleString()}</span></div>}
            {rawqStatus.chunks != null && <div className="flex items-center gap-2"><span className="text-muted-foreground w-[80px]">Chunks</span><span className="text-foreground/80">{rawqStatus.chunks.toLocaleString()}</span></div>}
            <div className="flex items-center gap-2"><span className="text-muted-foreground w-[80px]">Available</span><span className="text-foreground/80">{rawqStatus.available ? "Yes" : "No"}</span></div>
          </div>
        ) : <p className="text-[12px] text-muted-foreground/50">No project selected</p>}
      </div>

      {/* Model Catalog */}
      <div className="rounded-lg border border-border/30 bg-background/50 p-4 space-y-2">
        <div className="flex items-center gap-2">
          <h3 className="text-[13px] font-medium text-foreground flex-1">Model Catalog</h3>
          <button onClick={handleRefreshModels} disabled={refreshing}
            className="text-[11px] px-2.5 py-1 rounded-md bg-primary/10 text-primary hover:bg-primary/20 transition-colors disabled:opacity-40 font-medium">
            {refreshing ? "Refreshing…" : "Refresh"}
          </button>
        </div>
        <div className="space-y-1 text-[12px]">
          <div className="flex items-center gap-2"><span className="text-muted-foreground w-[80px]">Total</span><span className="text-foreground/80">{engineModels.length} models</span></div>
          {Object.entries(engineGroups).map(([engine, count]) => (
            <div key={engine} className="flex items-center gap-2"><span className="text-muted-foreground w-[80px]">{engine}</span><span className="text-foreground/80">{count} models</span></div>
          ))}
        </div>
      </div>

      {/* PtyModeToggle 숨김 (s37): PTY 는 현재 메인/브랜치 send 경로에서
          기본 off 이고 내부 터미널(VTE) 패널 전용. 설정 토글은 혼선만 줌.
          메인 send 가 PTY 로 복귀하면 다시 노출. */}
      <ContextBudgetControl />
      <WorkflowSkillsConfig />
      <InsightAgentConfig />
      <ContextHubPanel hubHealth={hubHealth} />

      {/* Background / Daemon */}
      <div className="rounded-lg border border-border/30 bg-background/50 p-4 space-y-2">
        <h3 className="text-[13px] font-medium text-foreground">Background Execution</h3>
        <div className="space-y-1 text-[12px]">
          <div className="flex items-center gap-2"><span className="text-muted-foreground w-[80px]">Pattern</span><span className="text-foreground/80">start_* command + event listener (fire-and-forget)</span></div>
          <div className="flex items-center gap-2"><span className="text-muted-foreground w-[80px]">rawq daemon</span><span className="text-foreground/80">Auto-start, 30min idle timeout</span></div>
          <div className="flex items-center gap-2"><span className="text-muted-foreground w-[80px]">DB SSOT</span><span className="text-foreground/80">Event 유실 시 list_messages()로 복구</span></div>
        </div>
      </div>

      <AttachmentsCleanupPanel />
    </div>
  );
}

// ─── Attachments Cleanup ──────────────────────────────────────────────────────

function AttachmentsCleanupPanel() {
  const { t } = useTranslation("settings");
  const selectedProjectKey = useChatStore((s) => s.selectedProjectKey);
  const [olderThanDays, setOlderThanDays] = useState(30);
  const [running, setRunning] = useState(false);
  const [lastResult, setLastResult] = useState<{ deletedCount: number; freedBytes: number } | null>(null);
  const [error, setError] = useState<string | null>(null);

  const runCleanup = async () => {
    if (!selectedProjectKey) {
      setError(t("runtime.attachments.error.no_project"));
      return;
    }
    setRunning(true);
    setError(null);
    try {
      const project = await invoke<{ path?: string }>("get_project", { key: selectedProjectKey });
      if (!project.path) { setError(t("runtime.attachments.error.no_path")); return; }
      const result = await invoke<{ deletedCount: number; freedBytes: number }>("cleanup_attachments", {
        projectPath: project.path,
        olderThanDays,
      });
      setLastResult(result);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setRunning(false);
    }
  };

  const formatSize = (bytes: number) => bytes >= 1024 * 1024
    ? `${(bytes / 1024 / 1024).toFixed(1)}MB`
    : `${(bytes / 1024).toFixed(0)}KB`;

  return (
    <div className="rounded-lg border border-border/30 bg-background/50 p-4 space-y-3">
      <div>
        <h3 className="text-[13px] font-medium text-foreground">{t("runtime.attachments.heading")}</h3>
        <p className="text-[11px] text-muted-foreground mt-0.5">
          {t("runtime.attachments.description")}
        </p>
      </div>
      <div className="flex items-center gap-2 text-[12px]">
        <span className="text-muted-foreground">{t("runtime.attachments.older_than")}</span>
        <input
          type="number"
          min={0}
          max={3650}
          value={olderThanDays}
          onChange={(e) => setOlderThanDays(Math.max(0, parseInt(e.target.value || "0", 10)))}
          className="w-16 px-2 py-1 rounded border border-border/40 bg-background text-foreground"
        />
        <span className="text-muted-foreground">{t("runtime.attachments.days_suffix")}</span>
        <button
          onClick={runCleanup}
          disabled={running || !selectedProjectKey}
          className="ml-auto px-3 py-1 rounded bg-primary text-primary-foreground text-[12px] disabled:opacity-50"
        >
          {running ? t("runtime.attachments.running") : t("runtime.attachments.run")}
        </button>
      </div>
      {lastResult && (
        <div className="text-[11px] text-muted-foreground">
          {t("runtime.attachments.result", { count: lastResult.deletedCount, size: formatSize(lastResult.freedBytes) })}
        </div>
      )}
      {error && (
        <div className="text-[11px] text-destructive">❌ {error}</div>
      )}
    </div>
  );
}
