import { useState } from "react";
import { cn, errorMessage } from "@/lib/utils";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { FolderPlus } from "lucide-react";

interface AddProjectFormProps {
  showAddProject: boolean;
  setShowAddProject: (v: boolean) => void;
  createProject: (input: { key: string; name: string; path: string; type: string; source: string }) => Promise<void>;
  selectProject: (key: string) => Promise<void>;
}

export function AddProjectForm({ showAddProject, setShowAddProject, createProject, selectProject }: AddProjectFormProps) {
  const [newProjectPath, setNewProjectPath] = useState("");
  const [newProjectName, setNewProjectName] = useState("");
  const [addingProject, setAddingProject] = useState(false);
  const [pathError, setPathError] = useState<string | null>(null);

  const handlePickFolder = async () => {
    try { const s = await open({ directory: true, multiple: false, title: "프로젝트 폴더 선택" }); if (s && typeof s === "string") { setNewProjectPath(s); setPathError(null); if (!newProjectName.trim()) setNewProjectName(s.split(/[\\/]/).pop() || ""); } } catch { /**/ }
  };

  const handleAddProject = async () => {
    const path = newProjectPath.trim();
    if (!path) { setPathError("경로를 입력하세요"); return; }
    setAddingProject(true); setPathError(null);
    try {
      const v = await invoke<{ valid: boolean; normalizedPath: string; error?: string }>("validate_project_path", { path });
      if (!v.valid) { setPathError(v.error || "유효하지 않은 경로"); setAddingProject(false); return; }
      const name = newProjectName.trim() || v.normalizedPath.split(/[\\/]/).pop() || "Project";
      const key = name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/(^-|-$)/g, "") || `proj-${Date.now()}`;
      await createProject({ key, name, path: v.normalizedPath, type: "project", source: "configured" });
      setNewProjectPath(""); setNewProjectName(""); setPathError(null); setShowAddProject(false);
      await selectProject(key);
    } catch (e) { const msg = errorMessage(e); if (msg.includes("이미")) setPathError(msg); }
    finally { setAddingProject(false); }
  };

  if (!showAddProject) {
    return (
      <button onClick={() => setShowAddProject(true)}
        className="w-full flex items-center gap-1.5 px-1 h-[22px] text-[10px] text-sidebar-foreground/35 hover:text-sidebar-foreground hover:bg-white/[0.04] rounded transition-colors">
        <FolderPlus className="w-3 h-3" /> Add project
      </button>
    );
  }

  return (
    <div className="space-y-1 pl-1">
      <div className="flex gap-1">
        <input placeholder="경로" value={newProjectPath} onChange={(e) => { setNewProjectPath(e.target.value); setPathError(null); }}
          className={cn("flex-1 bg-white/[0.04] rounded px-1.5 py-0.5 text-[10px] outline-none placeholder:text-sidebar-foreground/25 border focus:border-ring/30",
            pathError ? "border-destructive/40" : "border-white/[0.06]")} autoFocus />
        <button onClick={handlePickFolder} className="shrink-0 p-1 rounded bg-white/[0.04] text-sidebar-foreground/50 hover:text-sidebar-foreground transition-colors" title="폴더 선택">
          <FolderPlus className="w-3 h-3" />
        </button>
      </div>
      {pathError && <p className="text-[8px] text-destructive px-0.5">{pathError}</p>}
      <input placeholder="이름 (선택)" value={newProjectName} onChange={(e) => setNewProjectName(e.target.value)}
        onKeyDown={(e) => { if (e.key === "Enter") handleAddProject(); }}
        className="w-full bg-white/[0.04] rounded px-1.5 py-0.5 text-[10px] outline-none placeholder:text-sidebar-foreground/25 border border-white/[0.06] focus:border-ring/30" />
      <div className="flex gap-1">
        <button onClick={handleAddProject} disabled={!newProjectPath.trim() || addingProject}
          className="flex-1 px-2 py-0.5 rounded bg-primary/15 text-primary text-[10px] font-medium hover:bg-primary/25 transition-colors disabled:opacity-40">
          {addingProject ? "…" : "추가"}</button>
        <button onClick={() => { setShowAddProject(false); setPathError(null); }}
          className="px-2 py-0.5 rounded text-sidebar-foreground/40 text-[10px] hover:bg-white/[0.04] transition-colors">취소</button>
      </div>
    </div>
  );
}
