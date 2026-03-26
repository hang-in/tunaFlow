import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { FolderOpen, Folder, ChevronRight, ChevronDown, File } from "lucide-react";
import { TreeRow, SectionHeader } from "./TreeRow";

// ─── Hook ─────────────────────────────────────────────────────────────────────

interface DirEntry { name: string; isDir: boolean; path: string; }

export function useDirectoryListing(path: string | null | undefined) {
  const [entries, setEntries] = useState<DirEntry[]>([]);
  const [expandedDirs, setExpandedDirs] = useState<Set<string>>(new Set());
  const [subEntries, setSubEntries] = useState<Map<string, DirEntry[]>>(new Map());
  useEffect(() => {
    if (!path) { setEntries([]); return; }
    invoke<DirEntry[]>("list_directory", { path }).then(setEntries).catch(() => setEntries([]));
    setExpandedDirs(new Set()); setSubEntries(new Map());
  }, [path]);
  const toggleDir = async (dirPath: string) => {
    setExpandedDirs((p) => { const n = new Set(p); n.has(dirPath) ? n.delete(dirPath) : n.add(dirPath); return n; });
    if (!subEntries.has(dirPath)) {
      try { const sub = await invoke<DirEntry[]>("list_directory", { path: dirPath }); setSubEntries((p) => new Map(p).set(dirPath, sub)); } catch { /**/ }
    }
  };
  return { entries, expandedDirs, subEntries, toggleDir };
}

// ─── Section ──────────────────────────────────────────────────────────────────

interface FilesSectionProps {
  filesOpen: boolean;
  setFilesOpen: (v: boolean) => void;
  projectPath: string | null | undefined;
}

export function FilesSection({ filesOpen, setFilesOpen, projectPath }: FilesSectionProps) {
  const { entries: fileEntries, expandedDirs, subEntries, toggleDir } = useDirectoryListing(projectPath);

  const renderFileEntry = (entry: DirEntry, depth: number) => {
    if (!entry.isDir) {
      return <TreeRow key={entry.path} depth={depth} icon={<File className="w-3 h-3 text-sidebar-foreground/20" />}
        label={<span className="text-sidebar-foreground/50">{entry.name}</span>} className="cursor-default" />;
    }
    const isOpen = expandedDirs.has(entry.path);
    const children = subEntries.get(entry.path) ?? [];
    return (
      <div key={entry.path}>
        <TreeRow depth={depth} isParent
          icon={isOpen ? <ChevronDown className="w-3 h-3" /> : <ChevronRight className="w-3 h-3" />}
          label={<span className="flex items-center gap-1">
            {isOpen ? <FolderOpen className="w-3.5 h-3.5 text-sidebar-foreground/50" /> : <Folder className="w-3.5 h-3.5 text-sidebar-foreground/35" />}
            <span>{entry.name}</span>
          </span>}
          onClick={() => toggleDir(entry.path)} />
        {isOpen && depth < 2 && children.map((child) => renderFileEntry(child, depth + 1))}
      </div>
    );
  };

  return (
    <>
      <SectionHeader title="Files" expanded={filesOpen} onToggle={() => setFilesOpen(!filesOpen)} />
      {filesOpen && (
        <>
          {!projectPath ? (
            <TreeRow depth={1} className="cursor-default" icon={<Folder className="w-3.5 h-3.5 text-sidebar-foreground/15" />}
              label={<span className="text-[10px] text-sidebar-foreground/25 italic">No project path</span>} />
          ) : fileEntries.length === 0 ? (
            <TreeRow depth={1} className="cursor-default" icon={<Folder className="w-3.5 h-3.5 text-sidebar-foreground/15" />}
              label={<span className="text-[10px] text-sidebar-foreground/25 italic">Empty</span>} />
          ) : fileEntries.map((entry) => renderFileEntry(entry, 1))}
        </>
      )}
    </>
  );
}
