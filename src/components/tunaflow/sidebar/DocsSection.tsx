import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { FileText, ChevronRight, ChevronDown, Folder, FolderOpen, Copy } from "lucide-react";
import { cn } from "@/lib/utils";
import { useFileViewer } from "../chat/fileViewerContext";
import { SidebarContextMenu, type ContextMenuState } from "./SidebarContextMenu";
import { copyToClipboard } from "@/lib/clipboard";

interface DocEntry {
  name: string;
  path: string;
  isDir: boolean;
  children?: DocEntry[];
}

/** Scan project for .md files, return a tree structure */
async function scanDocs(projectPath: string): Promise<DocEntry[]> {
  try {
    const entries = await invoke<{ name: string; isDir: boolean; path: string }[]>(
      "list_directory", { path: projectPath }
    );

    const tree: DocEntry[] = [];
    for (const entry of entries) {
      if (entry.isDir) {
        if (["docs", ".github"].includes(entry.name)) {
          const children = await scanDocsDir(entry.path, 0);
          if (children.length > 0) {
            tree.push({ name: entry.name, path: entry.path, isDir: true, children });
          }
        }
      } else if (entry.name.endsWith(".md")) {
        tree.push({ name: entry.name, path: entry.path, isDir: false });
      }
    }
    return tree;
  } catch {
    return [];
  }
}

async function scanDocsDir(dirPath: string, depth: number): Promise<DocEntry[]> {
  if (depth > 3) return [];
  try {
    const entries = await invoke<{ name: string; isDir: boolean; path: string }[]>(
      "list_directory", { path: dirPath }
    );
    const result: DocEntry[] = [];
    for (const entry of entries) {
      if (entry.isDir) {
        const children = await scanDocsDir(entry.path, depth + 1);
        if (children.length > 0) {
          result.push({ name: entry.name, path: entry.path, isDir: true, children });
        }
      } else if (entry.name.endsWith(".md")) {
        result.push({ name: entry.name, path: entry.path, isDir: false });
      }
    }
    return result;
  } catch {
    return [];
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
  const [docs, setDocs] = useState<DocEntry[]>([]);
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [ctxMenu, setCtxMenu] = useState<ContextMenuState | null>(null);
  const fileViewer = useFileViewer();

  useEffect(() => {
    if (!projectPath) { setDocs([]); return; }
    scanDocs(projectPath).then(setDocs);
  }, [projectPath]);

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
      {docs.length === 0 ? (
        <p className="px-3 text-[10px] text-sidebar-foreground/25 italic">{t("empty.no_docs")}</p>
      ) : (
        docs.map((entry) => renderEntry(entry, 0))
      )}
      {ctxMenu && <SidebarContextMenu menu={ctxMenu} onClose={() => setCtxMenu(null)} />}
    </div>
  );
}
