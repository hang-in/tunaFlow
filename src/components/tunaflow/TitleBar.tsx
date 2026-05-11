import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { isWindows as detectIsWindows, detectPlatformDiagnostic } from "@/lib/platform";
import { useChatStore } from "@/stores/chatStore";
import { WindowControls } from "./WindowControls";

const isWindows = detectIsWindows();

// Issue #264 (Windows 캡션바 누락) 진단: PR #237 의 set_decorations(false) 가
// 호출되면 native chrome 이 사라지고 그 자리를 WindowControls 가 차지해야 한다.
// 사용자 환경에서 isWindows 가 false 로 평가되면 WindowControls 가 미마운트되어
// 캡션 영역 통째로 공백. 첫 mount 시 1회 진단 dump 으로 webview UA 의 실제
// 값과 detection 결과를 노출한다 (devtools console 에서 확인).
console.warn("[TitleBar] platform diag", { isWindows, ...detectPlatformDiagnostic() });

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
      className="h-[32px] shrink-0 flex items-center select-none bg-sidebar"
    >
      {/* Outer container is **not** a drag region. Tauri 2 의 drag region 은
          native 단에서 mousedown 을 가로채므로 button 같은 child 는 React
          synthetic stopPropagation 으로 가려도 click 이 fire 되지 않는다
          (architect dev 검증 2026-05-07, issue #264). 대신 좌패딩 / 정보 row /
          중앙 spacer 각각의 sub-section 에만 attribute 부착해 button 영역은
          drag region 의 descendant 가 아니게 만든다. */}

      {/* Left padding — reserves traffic-light area on mac, minimal on Windows.
          h-full 필수: outer 가 `flex items-center` 라 자식이 stretch 안 되어
          명시 height 없으면 hit area 0 → drag.js 의 e.target 이 절대 본 div
          가 못 됨. issue #264 후속 (Windows 캡션 hit area 누락). */}
      <div
        data-tauri-drag-region
        className={isWindows ? "h-full w-[12px] shrink-0" : "h-full w-[72px] shrink-0"}
      />

      {/* Info row — selectedProjectKey 가 있을 때만 (즉 메인 AppShell 분기)
          렌더. splash / ProjectStartup 화면에서는 카드 자체가 24px logo 를
          이미 표시하므로 chrome 의 11px "tunaFlow" 가 redundant + 깔끔한
          시작 화면 디자인 의도를 깨뜨려 숨긴다. 캡션바의 드래그 / 닫기 /
          더블클릭 maximize 동작은 좌측 패딩 + 가운데 spacer + WindowControls
          (Windows) 만으로 모두 보장된다 (#264 후속). */}
      {selectedProjectKey && (
        <div data-tauri-drag-region className="h-full flex items-center gap-0 shrink-0">
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
      )}

      {/* Center — large drag region. h-full 없으면 빈 div 의 height 0 →
          캡션바의 가장 큰 영역이 클릭 hit 0 이 되어 드래그·더블클릭 모두 실패. */}
      <div data-tauri-drag-region className="h-full flex-1" />

      {/* Right side: Windows custom controls / mac empty padding */}
      {isWindows ? <WindowControls /> : <div className="w-[72px] shrink-0" />}
    </div>
  );
}
