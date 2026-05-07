import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, Square, X, Copy } from "lucide-react";
import { detectPlatformDiagnostic } from "@/lib/platform";

/**
 * Custom Window Controls — Windows 11 native parity (46×32 sq buttons).
 * Rendered only on Windows by `TitleBar.tsx` (platform-detect gate); the
 * component itself has no `cfg` so unit tests can exercise it on any host.
 *
 * Plan: docs/plans/windowsTitlebarUnificationPlan_2026-04-29.md (§3.2, T-WT-2)
 * Decisions: Q-WT-1 (Windows native shape) / Q-WT-2 (status-rejected close hover)
 */
export function WindowControls() {
  // Issue #264 진단: WindowControls 가 mount 됐다는 사실 자체를 dev tools 에
  // 노출. 보고된 캡션바 누락이 (a) 컴포넌트 미마운트 vs (b) 마운트는 됐으나
  // 보이지 않음 중 어느 쪽인지 판단할 수 있게 한다. mount 1회만 fire.
  useEffect(() => {
    console.warn("[WindowControls] mounted", detectPlatformDiagnostic());
  }, []);

  const [isMaximized, setIsMaximized] = useState(false);

  useEffect(() => {
    let cleanup: (() => void) | undefined;
    const w = getCurrentWindow();
    w.isMaximized().then(setIsMaximized).catch(() => {});
    w.onResized(() => {
      w.isMaximized().then(setIsMaximized).catch(() => {});
    })
      .then((un) => { cleanup = un; })
      .catch(() => {});
    return () => { if (cleanup) cleanup(); };
  }, []);

  // Failure 시에만 console.error — 정상 click 은 noise 회피. 권한 누락 시
  // capabilities/default.json 에 core:window:allow-{minimize,toggle-maximize,close}
  // 가 빠진 회귀 단서 (issue #264 의 진짜 root cause 패턴).
  const onMinimize = () => {
    getCurrentWindow().minimize().catch((e) => console.error("[WindowControls] minimize err", e));
  };
  const onToggleMax = () => {
    getCurrentWindow().toggleMaximize().catch((e) => console.error("[WindowControls] toggleMaximize err", e));
  };
  const onClose = () => {
    getCurrentWindow().close().catch((e) => console.error("[WindowControls] close err", e));
  };

  // Tauri 2 의 `data-tauri-drag-region` 은 element + descendants 까지 mousedown
  // 을 drag 로 가로챈다. parent TitleBar 가 drag region 이라 button 의 click
  // 이 escalate 되어 동작하지 않는 회귀 (architect dev 검증, 2026-05-07).
  // mousedown 단계에서 stopPropagation 하면 drag 로 가지 않고 click 이 정상 fire.
  const stopDrag = (e: React.MouseEvent) => { e.stopPropagation(); };

  return (
    <div
      className="flex items-center h-full shrink-0"
      data-testid="window-controls"
    >
      <button
        type="button"
        aria-label="Minimize"
        onMouseDown={stopDrag}
        onClick={onMinimize}
        className="h-full w-[46px] flex items-center justify-center text-foreground/70 hover:bg-foreground/10 hover:text-foreground focus-visible:outline-none focus-visible:bg-foreground/10"
      >
        <Minus size={14} />
      </button>
      <button
        type="button"
        aria-label={isMaximized ? "Restore" : "Maximize"}
        onMouseDown={stopDrag}
        onClick={onToggleMax}
        className="h-full w-[46px] flex items-center justify-center text-foreground/70 hover:bg-foreground/10 hover:text-foreground focus-visible:outline-none focus-visible:bg-foreground/10"
      >
        {isMaximized ? <Copy size={12} /> : <Square size={12} />}
      </button>
      <button
        type="button"
        aria-label="Close"
        onMouseDown={stopDrag}
        onClick={onClose}
        className="h-full w-[46px] flex items-center justify-center text-foreground/70 hover:bg-status-rejected hover:text-white focus-visible:outline-none focus-visible:bg-status-rejected focus-visible:text-white"
      >
        <X size={14} />
      </button>
    </div>
  );
}
