import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { isWindows as detectIsWindows } from "@/lib/platform";
import { useChatStore } from "@/stores/chatStore";
import { WindowControls } from "./WindowControls";

const isWindows = detectIsWindows();

/**
 * Unified title bar (mac / Windows).
 * - mac: traffic-light overlay (`titleBarStyle: "Overlay"` + `hiddenTitle:
 *   true` in tauri.conf.json) — left padding reserves the traffic-light area.
 * - Windows: native decorations dropped in `bootstrap/window.rs` (T-WT-1);
 *   custom `<WindowControls />` rendered top-right; left padding is minimal.
 *
 * Information row (tunaFlow / projectName / gitBranch) is **left-aligned on
 * both OSes** (Q-WT-4) — center is reserved as drag-region for window move.
 *
 * Plan: docs/plans/windowsTitlebarUnificationPlan_2026-04-29.md (§3.3, T-WT-3)
 */
export function TitleBar() {
  const selectedProjectKey = useChatStore((s) => s.selectedProjectKey);
  const projects = useChatStore((s) => s.projects);
  const project = projects.find((p) => p.key === selectedProjectKey);
  const projectName = project?.name ?? "";

  const [gitBranch, setGitBranch] = useState<string | null>(null);
  useEffect(() => {
    if (!project?.path) { setGitBranch(null); return; }
    invoke<{ isRepo: boolean; branch: string | null; dirty: boolean }>("get_git_status", { projectPath: project.path })
      .then((s) => setGitBranch(s.isRepo ? s.branch : null))
      .catch(() => setGitBranch(null));
    const interval = setInterval(() => {
      if (!project?.path) return;
      invoke<{ isRepo: boolean; branch: string | null; dirty: boolean }>("get_git_status", { projectPath: project.path })
        .then((s) => setGitBranch(s.isRepo ? s.branch : null))
        .catch(() => {});
    }, 30_000);
    return () => clearInterval(interval);
  }, [project?.path]);

  return (
    <div
      data-tauri-drag-region
      className="h-[32px] shrink-0 flex items-center select-none bg-sidebar"
    >
      {/* Left padding — reserves traffic-light area on mac, minimal on Windows */}
      <div
        data-tauri-drag-region
        className={isWindows ? "w-[12px] shrink-0" : "w-[72px] shrink-0"}
      />

      {/* Info row — left-aligned on both OSes (Q-WT-4) */}
      <div data-tauri-drag-region className="flex items-center gap-0 shrink-0">
        <span data-tauri-drag-region className="text-[11px] font-bold text-foreground/70 tracking-wide">
          tunaFlow
        </span>

        {projectName && (
          <>
            <span data-tauri-drag-region className="mx-2 text-[6px] text-muted-foreground/30">●</span>
            <span data-tauri-drag-region className="text-[11px] font-medium text-foreground/45 truncate max-w-[160px]">
              {projectName}
            </span>
          </>
        )}

        {gitBranch && (
          <>
            <span data-tauri-drag-region className="mx-2 text-[6px] text-muted-foreground/25">●</span>
            <span data-tauri-drag-region className="text-[10px] font-mono text-muted-foreground/35 truncate max-w-[180px]">
              {gitBranch}
            </span>
          </>
        )}
      </div>

      {/* Center — large drag region */}
      <div data-tauri-drag-region className="flex-1" />

      {/* Right side: Windows custom controls / mac empty padding */}
      {isWindows ? <WindowControls /> : <div className="w-[72px] shrink-0" />}
    </div>
  );
}
