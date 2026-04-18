/**
 * ConventionsSection.tsx — Phase 1 (feat/conventions-context-sync 브랜치)
 *
 * project별 + persona별 conventions(static ContextPack layer)를 편집하는 UI.
 * 저장된 내용은 sync_project_conventions로 CLAUDE.md/AGENTS.md/GEMINI.md에 반영된다.
 *
 * env TUNAFLOW_CONVENTIONS_SYNC=1 일 때만 ContextPack에서 정적 layer가 생략되고
 * 이 파일들이 진짜로 적용된다. (env 안 켜져 있어도 편집/sync는 가능)
 */
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useChatStore } from "@/stores/chatStore";
import { Loader2, RefreshCw, Trash2, Plus } from "lucide-react";
import { cn } from "@/lib/utils";

interface ConventionRow {
  id: number;
  layer: string;
  personaLabel: string | null;
  content: string;
  sourceId: string | null;
  revision: number;
  updatedAt: number;
}

interface SyncReport {
  rowsWritten: number;
  splitFiles: number;
  entryFiles: number;
  truncated: string[];
}

const LAYER_OPTIONS = [
  { id: "platform", label: "Platform / Project meta", hint: "프로젝트 path, 빌드 명령어, 언어 등" },
  { id: "agent_role", label: "Agent role", hint: "에이전트 역할 지침 (architect/coder/reviewer 등)" },
  { id: "persona", label: "Persona", hint: "페르소나 행동 지침" },
  { id: "user_profile", label: "User profile", hint: "사용자 정보, 선호" },
  { id: "plan_doc", label: "Plan document", hint: "현재 진행 중인 plan 본문 (revision 기반)" },
  { id: "findings", label: "Findings", hint: "이전 review verdict의 findings 본문" },
  { id: "artifact", label: "Artifact", hint: "참고 산출물 (result.md 등)" },
];

export function ConventionsSection() {
  const projects = useChatStore((s) => s.projects);
  const selectedProjectKey = useChatStore((s) => s.selectedProjectKey);
  const [projectKey, setProjectKey] = useState<string>(selectedProjectKey ?? "");
  const [personaFilter, setPersonaFilter] = useState<string>("");
  const [rows, setRows] = useState<ConventionRow[]>([]);
  const [loading, setLoading] = useState(false);
  const [syncing, setSyncing] = useState(false);
  const [report, setReport] = useState<SyncReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [syncEnabled, setSyncEnabled] = useState<boolean>(false);
  const [togglingSync, setTogglingSync] = useState(false);

  // 새 행 폼
  const [newLayer, setNewLayer] = useState<string>("platform");
  const [newPersona, setNewPersona] = useState<string>("");
  const [newSource, setNewSource] = useState<string>("");
  const [newContent, setNewContent] = useState<string>("");

  const project = projects.find((p) => p.key === projectKey);

  const reload = async () => {
    if (!projectKey) { setRows([]); return; }
    setLoading(true);
    setError(null);
    try {
      const result = await invoke<ConventionRow[]>("list_project_conventions", {
        projectKey,
        personaLabel: personaFilter || null,
      });
      setRows(result);
    } catch (e) {
      setError(String(e));
    }
    setLoading(false);
  };

  useEffect(() => { reload(); /* eslint-disable-next-line react-hooks/exhaustive-deps */ }, [projectKey, personaFilter]);

  // Load per-project toggle state (Phase 2 — in-app switch replaces env flag)
  useEffect(() => {
    if (!projectKey) { setSyncEnabled(false); return; }
    invoke<boolean>("get_project_conventions_sync", { projectKey })
      .then(setSyncEnabled)
      .catch(() => setSyncEnabled(false));
  }, [projectKey]);

  const handleToggleSync = async (next: boolean) => {
    if (!projectKey) return;
    setTogglingSync(true);
    try {
      await invoke("set_project_conventions_sync", { projectKey, enabled: next });
      setSyncEnabled(next);
    } catch (e) {
      setError(String(e));
    } finally {
      setTogglingSync(false);
    }
  };

  const handleAdd = async () => {
    if (!projectKey || !newContent.trim()) return;
    setError(null);
    try {
      await invoke("set_project_convention", {
        projectKey,
        layer: newLayer,
        personaLabel: newPersona.trim() || null,
        sourceId: newSource.trim() || null,
        content: newContent,
        revision: 1,
      });
      setNewContent("");
      setNewSource("");
      await reload();
    } catch (e) {
      setError(String(e));
    }
  };

  const handleDelete = async (row: ConventionRow) => {
    setError(null);
    try {
      await invoke("delete_project_convention", {
        projectKey,
        layer: row.layer,
        personaLabel: row.personaLabel,
        sourceId: row.sourceId,
      });
      await reload();
    } catch (e) {
      setError(String(e));
    }
  };

  const handleSync = async () => {
    if (!projectKey || !project?.path) {
      setError("project 경로가 없습니다");
      return;
    }
    setSyncing(true);
    setError(null);
    setReport(null);
    try {
      const result = await invoke<SyncReport>("sync_project_conventions", {
        projectKey,
        projectPath: project.path,
        personaLabel: personaFilter || null,
        engines: ["claude", "codex", "gemini"],
      });
      setReport(result);
    } catch (e) {
      setError(String(e));
    }
    setSyncing(false);
  };

  return (
    <div className="space-y-4">
      <div>
        <h2 className="text-[14px] font-[550] text-foreground mb-1">Conventions Context Sync</h2>
        <p className="text-[12px] text-muted-foreground mb-2">
          정적 ContextPack layer(platform / agent role / persona / user profile)를
          CLAUDE.md / AGENTS.md / GEMINI.md로 sync 합니다. 편집 후 "Sync Now" 로 파일 갱신.
        </p>
        {projectKey && (
          <label className="flex items-start gap-2 mt-2 p-2 rounded border border-border/50 bg-card/50 cursor-pointer hover:bg-card transition-colors">
            <input
              type="checkbox"
              checked={syncEnabled}
              onChange={(e) => handleToggleSync(e.target.checked)}
              disabled={togglingSync}
              className="mt-0.5"
            />
            <div className="flex-1">
              <div className="text-[12px] font-medium text-foreground">
                ContextPack 에서 정적 레이어 생략
                {togglingSync && <Loader2 className="inline-block w-3 h-3 ml-1.5 animate-spin text-muted-foreground" />}
              </div>
              <p className="text-[10.5px] text-muted-foreground/80 mt-0.5 leading-relaxed">
                켜면 매 turn 정적 레이어를 재송신하지 않습니다. CLI 가 CLAUDE.md/AGENTS.md 를
                자동 prepending 하며 Anthropic API 경로에서는 prompt cache 적중으로 비용 절감.
                먼저 아래에서 해당 프로젝트의 conventions 를 추가하고 "Sync Now" 로 파일을 만들어야 실제 효과.
              </p>
            </div>
          </label>
        )}
      </div>

      {/* Project + persona filter */}
      <div className="flex items-center gap-2">
        <select
          value={projectKey}
          onChange={(e) => setProjectKey(e.target.value)}
          className="text-[12px] bg-input border border-border rounded px-2 py-1 outline-none"
        >
          <option value="">— Project 선택 —</option>
          {projects.map((p) => (
            <option key={p.key} value={p.key}>{p.key}</option>
          ))}
        </select>
        <input
          value={personaFilter}
          onChange={(e) => setPersonaFilter(e.target.value)}
          placeholder="Persona 필터 (비우면 공통 only)"
          className="flex-1 text-[12px] bg-input border border-border rounded px-2 py-1 outline-none"
        />
        <button
          onClick={reload}
          disabled={!projectKey || loading}
          className="px-2 py-1 rounded text-[11px] bg-card hover:bg-accent border border-border disabled:opacity-40"
          title="다시 불러오기"
        >
          {loading ? <Loader2 className="w-3 h-3 animate-spin" /> : <RefreshCw className="w-3 h-3" />}
        </button>
        <button
          onClick={handleSync}
          disabled={!projectKey || !project?.path || syncing}
          className="px-3 py-1 rounded text-[11px] font-medium bg-primary/15 text-primary hover:bg-primary/25 disabled:opacity-40"
        >
          {syncing ? "Syncing…" : "Sync Now"}
        </button>
      </div>

      {error && (
        <div className="text-[11px] text-destructive bg-destructive/10 border border-destructive/30 rounded px-2 py-1.5">
          {error}
        </div>
      )}

      {report && (
        <div className="text-[11px] text-status-approved bg-status-approved/8 border border-status-approved/30 rounded px-2 py-1.5">
          ✓ Sync 완료 — entry files: {report.entryFiles}, split files: {report.splitFiles}
          {report.truncated.length > 0 && ` (${report.truncated.length}개 truncated)`}
        </div>
      )}

      {/* Rows list */}
      <div className="space-y-2">
        {rows.length === 0 && !loading && projectKey && (
          <p className="text-[11px] text-muted-foreground/60">아직 등록된 conventions가 없습니다. 아래에서 추가하세요.</p>
        )}
        {rows.map((row) => (
          <div key={row.id} className="rounded border border-border/40 bg-card p-2 space-y-1">
            <div className="flex items-center gap-2 text-[10px]">
              <span className="px-1.5 py-0.5 rounded bg-primary/10 text-primary font-medium">{row.layer}</span>
              {row.personaLabel && (
                <span className="px-1.5 py-0.5 rounded bg-accent text-muted-foreground">persona: {row.personaLabel}</span>
              )}
              {row.sourceId && (
                <span className="px-1.5 py-0.5 rounded bg-accent text-muted-foreground">src: {row.sourceId}</span>
              )}
              <span className="text-muted-foreground/50">rev {row.revision}</span>
              <span className="flex-1" />
              <button
                onClick={() => handleDelete(row)}
                className="p-1 text-destructive/60 hover:text-destructive hover:bg-destructive/10 rounded"
                title="삭제"
              >
                <Trash2 className="w-3 h-3" />
              </button>
            </div>
            <pre className={cn(
              "text-[10px] text-foreground/80 whitespace-pre-wrap font-mono leading-snug",
              "max-h-32 overflow-y-auto"
            )}>{row.content}</pre>
          </div>
        ))}
      </div>

      {/* Add new */}
      {projectKey && (
        <div className="rounded border border-primary/30 bg-primary/5 p-2 space-y-2">
          <div className="text-[11px] font-[550] text-primary">새 convention 추가</div>
          <div className="flex items-center gap-2">
            <select
              value={newLayer}
              onChange={(e) => setNewLayer(e.target.value)}
              className="text-[11px] bg-input border border-border rounded px-2 py-1 outline-none"
            >
              {LAYER_OPTIONS.map((l) => (
                <option key={l.id} value={l.id}>{l.label}</option>
              ))}
            </select>
            <input
              value={newPersona}
              onChange={(e) => setNewPersona(e.target.value)}
              placeholder="Persona (비우면 공통)"
              className="flex-1 text-[11px] bg-input border border-border rounded px-2 py-1 outline-none"
            />
            <input
              value={newSource}
              onChange={(e) => setNewSource(e.target.value)}
              placeholder="Source ID (선택)"
              className="flex-1 text-[11px] bg-input border border-border rounded px-2 py-1 outline-none"
            />
          </div>
          <p className="text-[10px] text-muted-foreground/60">{LAYER_OPTIONS.find((l) => l.id === newLayer)?.hint}</p>
          <textarea
            value={newContent}
            onChange={(e) => setNewContent(e.target.value)}
            placeholder="Markdown 본문…"
            rows={5}
            className="w-full text-[11px] bg-input border border-border rounded px-2 py-1.5 outline-none font-mono leading-snug"
          />
          <div className="flex justify-end">
            <button
              onClick={handleAdd}
              disabled={!newContent.trim()}
              className="flex items-center gap-1 px-3 py-1 rounded text-[11px] font-medium bg-status-approved/15 text-status-approved hover:bg-status-approved/25 disabled:opacity-40"
            >
              <Plus className="w-3 h-3" />추가
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
