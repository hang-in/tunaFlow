import { useState, useEffect, useCallback, useRef } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { FileText, ChevronRight, ChevronDown, Folder, FolderOpen, Copy } from "lucide-react";
import { cn } from "@/lib/utils";
import { useFileViewer } from "../chat/fileViewerContext";
import { SidebarContextMenu, type ContextMenuState } from "./SidebarContextMenu";
import { copyToClipboard } from "@/lib/clipboard";
import { getSetting } from "@/lib/appStore";
import {
  DOCS_PANEL_SCOPE_KEY,
  DOCS_PANEL_SCOPE_DEFAULT,
  type DocsPanelScope,
} from "../settings/DocsScopeSection";

interface DocEntry {
  name: string;
  path: string;
  isDir: boolean;
  children?: DocEntry[];
}

interface DocsScanResult {
  entries: DocEntry[];
  fileCount: number;
  truncated: boolean;
}

/** Plan E (2026-04-29) — file count >200 시 1회 toast.
 *  Threshold 는 plan SSOT 고정. lazy load 는 Phase 2. */
const DOCS_PANEL_WARNING_THRESHOLD = 200;

const EMPTY_RESULT: DocsScanResult = { entries: [], fileCount: 0, truncated: false };

async function scanDocs(projectPath: string, scope: DocsPanelScope): Promise<DocsScanResult> {
  try {
    const result = await invoke<DocsScanResult>("list_project_docs", {
      projectPath,
      scope,
    });
    // Defensive: tests / stub envs may resolve to undefined.
    if (!result || !Array.isArray(result.entries)) return EMPTY_RESULT;
    return result;
  } catch (e) {
    console.warn("[docs] list_project_docs failed:", e);
    return EMPTY_RESULT;
  }
}

async function revealInFinder(path: string) {
  try {
    const { revealItemInDir } = await import("@tauri-apps/plugin-opener");
    await revealItemInDir(path);
  } catch (e) {
    console.debug("[opener]", e);
  }
}

// ─── DocsSection ─────────────────────────────────────────────────────────────

interface DocsSectionProps {
  projectPath: string | null | undefined;
}

export function DocsSection({ projectPath }: DocsSectionProps) {
  const { t } = useTranslation("sidebar");
  const { t: tSettings } = useTranslation("settings");
  const [docs, setDocs] = useState<DocEntry[]>([]);
  const [scope, setScope] = useState<DocsPanelScope>(DOCS_PANEL_SCOPE_DEFAULT);
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [ctxMenu, setCtxMenu] = useState<ContextMenuState | null>(null);
  const fileViewer = useFileViewer();
  /** Toast 1회만 — 같은 (project,scope) 조합에서 재표시 X. */
  const warnedRef = useRef<Set<string>>(new Set());

  // 1) 초기 scope 로드 + scope 변경 이벤트 구독.
  useEffect(() => {
    let alive = true;
    getSetting<DocsPanelScope>(DOCS_PANEL_SCOPE_KEY, DOCS_PANEL_SCOPE_DEFAULT).then((v) => {
      if (alive) {
        setScope(v === "tunaflow" ? "tunaflow" : "all");
      }
    });
    const onScopeChange = (e: Event) => {
      const detail = (e as CustomEvent<{ scope: DocsPanelScope }>).detail;
      if (detail?.scope) {
        setScope(detail.scope === "tunaflow" ? "tunaflow" : "all");
      }
    };
    window.addEventListener("tf:docs-scope-changed", onScopeChange);
    return () => {
      alive = false;
      window.removeEventListener("tf:docs-scope-changed", onScopeChange);
    };
  }, []);

  // 2) projectPath / scope 변경 시 다시 스캔.
  useEffect(() => {
    if (!projectPath) {
      setDocs([]);
      return;
    }
    let alive = true;
    scanDocs(projectPath, scope).then((result) => {
      if (!alive) return;
      setDocs(result.entries);
      // Task 03 — perf toast (Plan E 명시 threshold = 200, 1회).
      const warnKey = `${projectPath}::${scope}`;
      if (
        scope === "all" &&
        result.fileCount > DOCS_PANEL_WARNING_THRESHOLD &&
        !warnedRef.current.has(warnKey)
      ) {
        warnedRef.current.add(warnKey);
        toast.warning(
          tSettings("docs_scope.performance_warning", { count: result.fileCount }),
          { duration: 6000 },
        );
      }
    });
    return () => { alive = false; };
  }, [projectPath, scope, tSettings]);

  const toggle = useCallback((path: string) => {
    setExpanded((prev) => {
      const next = new Set(prev);
      next.has(path) ? next.delete(path) : next.add(path);
      return next;
    });
  }, []);

  const openCtx = (e: React.MouseEvent, entry: DocEntry) => {
    e.preventDefault();
    e.stopPropagation();
    setCtxMenu({
      x: e.clientX,
      y: e.clientY,
      items: [
        {
          label: t("action.show_in_finder"),
          icon: <FolderOpen className="w-3.5 h-3.5" />,
          onClick: () => revealInFinder(entry.path),
        },
        {
          label: t("action.copy_path"),
          icon: <Copy className="w-3.5 h-3.5" />,
          onClick: () => copyToClipboard(entry.path),
        },
      ],
    });
  };

  const renderEntry = (entry: DocEntry, depth: number) => {
    if (entry.isDir) {
      const isOpen = expanded.has(entry.path);
      return (
        <div key={entry.path}>
          <button
            onClick={() => toggle(entry.path)}
            onContextMenu={(e) => openCtx(e, entry)}
            className="w-full flex items-center gap-1 px-2 py-0.5 text-[11px] text-sidebar-foreground/60 hover:text-sidebar-foreground hover:bg-sidebar-accent/40 rounded transition-colors select-none"
            style={{ paddingLeft: `${8 + depth * 12}px` }}
          >
            {isOpen ? <ChevronDown className="w-3 h-3 shrink-0" /> : <ChevronRight className="w-3 h-3 shrink-0" />}
            {isOpen ? <FolderOpen className="w-3 h-3 shrink-0 text-sidebar-foreground/40" /> : <Folder className="w-3 h-3 shrink-0 text-sidebar-foreground/30" />}
            <span className="truncate">{entry.name}/</span>
          </button>
          {isOpen && entry.children?.map((child) => renderEntry(child, depth + 1))}
        </div>
      );
    }

    return (
      <button
        key={entry.path}
        onClick={() => fileViewer?.openFile(entry.path)}
        onContextMenu={(e) => openCtx(e, entry)}
        className="w-full flex items-center gap-1.5 px-2 py-0.5 text-[11px] text-sidebar-foreground/50 hover:text-sidebar-foreground hover:bg-sidebar-accent/40 rounded transition-colors select-none"
        style={{ paddingLeft: `${8 + depth * 12}px` }}
        title={entry.path}
      >
        <FileText className="w-3 h-3 shrink-0 text-sidebar-foreground/25" />
        <span className="truncate">{entry.name}</span>
      </button>
    );
  };

  if (!projectPath) return null;

  return (
    <div className="py-1">
      <div className="px-3 pb-1 text-[10px] text-sidebar-foreground/30 select-none">
        {t(scope === "all" ? "docs_scope.all" : "docs_scope.tunaflow")}
      </div>
      {docs.length === 0 ? (
        <p className="px-3 text-[10px] text-sidebar-foreground/25 italic">{t("empty.no_docs")}</p>
      ) : (
        docs.map((entry) => renderEntry(entry, 0))
      )}
      {ctxMenu && <SidebarContextMenu menu={ctxMenu} onClose={() => setCtxMenu(null)} />}
    </div>
  );
}
