import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Minus, Square, X, Copy } from "lucide-react";

/**
 * Custom Window Controls — Windows 11 native parity (46×32 sq buttons).
 * Rendered only on Windows by `TitleBar.tsx` (platform-detect gate); the
 * component itself has no `cfg` so unit tests can exercise it on any host.
 *
 * Plan: docs/plans/windowsTitlebarUnificationPlan_2026-04-29.md (§3.2, T-WT-2)
 * Decisions: Q-WT-1 (Windows native shape) / Q-WT-2 (status-rejected close hover)
 */
export function WindowControls() {
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

  const onMinimize = () => { getCurrentWindow().minimize().catch(() => {}); };
  const onToggleMax = () => { getCurrentWindow().toggleMaximize().catch(() => {}); };
  const onClose = () => { getCurrentWindow().close().catch(() => {}); };

  return (
    <div
      className="flex items-center h-full shrink-0"
      data-tauri-drag-region={false}
      data-testid="window-controls"
    >
      <button
        type="button"
        aria-label="Minimize"
        onClick={onMinimize}
        className="h-full w-[46px] flex items-center justify-center text-foreground/70 hover:bg-foreground/10 hover:text-foreground focus-visible:outline-none focus-visible:bg-foreground/10"
      >
        <Minus size={14} />
      </button>
      <button
        type="button"
        aria-label={isMaximized ? "Restore" : "Maximize"}
        onClick={onToggleMax}
        className="h-full w-[46px] flex items-center justify-center text-foreground/70 hover:bg-foreground/10 hover:text-foreground focus-visible:outline-none focus-visible:bg-foreground/10"
      >
        {isMaximized ? <Copy size={12} /> : <Square size={12} />}
      </button>
      <button
        type="button"
        aria-label="Close"
        onClick={onClose}
        className="h-full w-[46px] flex items-center justify-center text-foreground/70 hover:bg-status-rejected hover:text-white focus-visible:outline-none focus-visible:bg-status-rejected focus-visible:text-white"
      >
        <X size={14} />
      </button>
    </div>
  );
}
