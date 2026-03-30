import { useState, useEffect } from "react";
import { copyToClipboard } from "@/lib/clipboard";
import { invoke } from "@tauri-apps/api/core";
import { cn } from "@/lib/utils";
import { X, Search, FileText, ChevronRight, Copy, Send, Archive } from "lucide-react";
import { useChatStore } from "@/stores/chatStore";
import { getSetting, setSetting } from "@/lib/appStore";

// ─── Context Budget Control ──────────────────────────────────────────────────

const CONTEXT_MODES = [
  { id: "auto", label: "Auto", desc: "대화 상태에 따라 자동 선택 (기본)" },
  { id: "lite", label: "Lite", desc: "프로젝트 + 대화 컨텍스트만 포함" },
  { id: "standard", label: "Standard", desc: "Lite + Plan, Findings, Artifacts" },
  { id: "full", label: "Full", desc: "Standard + Skills, rawq, Cross-session" },
] as const;

const SECTION_POLICY: Record<string, string[]> = {
  lite: ["Project", "Context"],
  standard: ["Project", "Context", "Plan", "Findings", "Artifacts"],
  full: ["Project", "Context", "Plan", "Findings", "Artifacts", "Skills", "rawq", "Cross-session"],
};

const BUDGET_MIN = 20_000;
const BUDGET_MAX = 120_000;
const BUDGET_STEP = 10_000;
const BUDGET_DEFAULT = 60_000;

function ContextBudgetControl() {
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
        <label className="text-[11px] text-muted-foreground">Included Sections {config.mode === "auto" ? "(Auto 모드: 최대 범위)" : ""}</label>
        <div className="flex flex-wrap gap-1">
          {["Project", "Context", "Persona", "Plan", "Findings", "Artifacts", "Skills", "rawq", "Cross-session", "Thread"].map((sec) => (
            <span key={sec} className={cn("px-1.5 py-0.5 rounded text-[9px] font-medium",
              sections.includes(sec) || sec === "Persona" || sec === "Thread" ? "bg-accent/60 text-foreground/60" : "bg-muted/30 text-muted-foreground/25 line-through")}>{sec}</span>
          ))}
        </div>
        <p className="text-[9px] text-muted-foreground/30">Persona는 설정 시 항상 포함. Thread는 branch에서 자동 포함. rawq는 코드 신호 감지 시 mode 무관 포함.</p>
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
    catch (e) { setError(String(e)); }
    finally { setSearching(false); }
  };

  const handleGet = async (id: string) => {
    setLoadingDoc(true); setError(null);
    try { setSelectedDoc(await invoke<HubDocument>("context_hub_get", { documentId: id })); }
    catch (e) { setError(String(e)); }
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
    catch (e) { setError(String(e)); }
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

export function RuntimeSection() {
  const rawqStatus = useChatStore((s) => s.rawqStatus);
  const engineModels = useChatStore((s) => s.engineModels);
  const loadEngineModels = useChatStore((s) => s.loadEngineModels);
  const [refreshing, setRefreshing] = useState(false);
  const [hubHealth, setHubHealth] = useState<HubHealth | null>(null);

  useEffect(() => { invoke<HubHealth>("context_hub_health").then(setHubHealth).catch(() => {}); }, []);

  const handleRefreshModels = async () => { setRefreshing(true); await loadEngineModels(true); setRefreshing(false); };

  const engineGroups = engineModels.reduce<Record<string, number>>((acc, m) => { acc[m.engine] = (acc[m.engine] ?? 0) + 1; return acc; }, {});

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-[14px] font-[550] text-foreground mb-1">Runtime</h2>
        <p className="text-[12px] text-muted-foreground mb-4">런타임 환경 상태를 확인하고 관리합니다.</p>
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

      <ContextBudgetControl />
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
    </div>
  );
}
