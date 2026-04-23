/**
 * Floating (popup) terminal panel — draggable, resizable independent window.
 * Rendered at AppShell level so it's accessible from any context (main/branch/RT).
 * Only mounts when terminalOpen && terminalMode === "float".
 */
import { useEffect, useRef, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "react-i18next";
import { useChatStore } from "@/stores/chatStore";
import { usePtyStore } from "@/stores/ptyStore";
import { cn } from "@/lib/utils";
import { X, PanelBottomOpen } from "lucide-react";
import { getSetting } from "@/lib/appStore";

interface TerminalSettings {
  fontFamily: string;
  fontSize: number;
  lineHeight: number;
}
const DEFAULT_TERMINAL_SETTINGS: TerminalSettings = {
  fontFamily: "'JetBrains Mono', 'D2Coding', monospace",
  fontSize: 12,
  lineHeight: 1.3,
};

// Min size: 80cols × 24rows at 12px JetBrains Mono (charW≈7.2px, lineH≈15.6px)
// 80×7.2 = 576px + padding → 600px width
// 24×15.6 = 374px + header 36px + padding → 420px height
// Default width: Claude Code uses ~120cols → 120×7.2 + padding ≈ 880px → 900px
const MIN_W = 600;
const MIN_H = 420;
const DEFAULT_W = 900;
const DEFAULT_H = 480;

type XTerminal = import("@xterm/xterm").Terminal;
type XFitAddon = import("@xterm/addon-fit").FitAddon;

function useContextLabel() {
  const { t } = useTranslation("common");
  const threadBranchId = useChatStore((s) => s.threadBranchId);
  const branches = useChatStore((s) => s.branches);
  if (!threadBranchId) return t("terminal.context_main_chat");
  const branch = branches.find((b) => b.id === threadBranchId);
  if (!branch) return t("terminal.context_branch");
  const label = branch.customLabel || branch.label;
  return branch.mode === "roundtable"
    ? t("terminal.context_rt_labeled", { label })
    : t("terminal.context_branch_labeled", { label });
}

export function TerminalFloatingPanel() {
  const { t } = useTranslation("common");
  const setTerminalMode = usePtyStore((s) => s.setTerminalMode);
  const toggleTerminal = usePtyStore((s) => s.toggleTerminal);
  const claudeSession = usePtyStore((s) => s.sessions.get("claude"));
  const sessions = usePtyStore((s) => s.sessions);

  const contextLabel = useContextLabel();
  const engineLabel = sessions.size > 0 ? [...sessions.keys()].join(" · ") : "no session";

  // Position & size
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);

  const [pos, setPos] = useState(() => ({
    x: Math.max(0, Math.floor((window.innerWidth - DEFAULT_W) / 2)),
    y: Math.max(0, window.innerHeight - DEFAULT_H - 56), // above status bar
  }));
  const [size, setSize] = useState({ w: DEFAULT_W, h: DEFAULT_H });

  // Terminal refs
  const containerRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<XTerminal | null>(null);
  const fitRef = useRef<XFitAddon | null>(null);
  const cleanupRef = useRef<(() => void) | null>(null);

  // Drag (move)
  const dragRef = useRef<{ startX: number; startY: number; startPX: number; startPY: number } | null>(null);
  const handleDragStart = useCallback((e: React.MouseEvent) => {
    if ((e.target as HTMLElement).closest("button")) return;
    e.preventDefault();
    dragRef.current = { startX: e.clientX, startY: e.clientY, startPX: pos.x, startPY: pos.y };
    const onMove = (ev: MouseEvent) => {
      if (!dragRef.current) return;
      const nx = dragRef.current.startPX + (ev.clientX - dragRef.current.startX);
      const ny = dragRef.current.startPY + (ev.clientY - dragRef.current.startY);
      setPos({
        x: Math.max(0, Math.min(window.innerWidth - MIN_W, nx)),
        y: Math.max(0, Math.min(window.innerHeight - MIN_H - 28, ny)),
      });
    };
    const onUp = () => {
      dragRef.current = null;
      document.removeEventListener("mousemove", onMove);
      document.removeEventListener("mouseup", onUp);
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
    };
    document.addEventListener("mousemove", onMove);
    document.addEventListener("mouseup", onUp);
    document.body.style.cursor = "move";
    document.body.style.userSelect = "none";
  }, [pos]);

  // Resize (bottom-right corner)
  const resizeRef = useRef<{ startX: number; startY: number; startW: number; startH: number } | null>(null);
  const handleResizeStart = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    resizeRef.current = { startX: e.clientX, startY: e.clientY, startW: size.w, startH: size.h };
    const onMove = (ev: MouseEvent) => {
      if (!resizeRef.current) return;
      const nw = Math.max(MIN_W, resizeRef.current.startW + (ev.clientX - resizeRef.current.startX));
      const nh = Math.max(MIN_H, resizeRef.current.startH + (ev.clientY - resizeRef.current.startY));
      setSize({ w: nw, h: nh });
      setTimeout(() => fitRef.current?.fit(), 20);
    };
    const onUp = () => {
      resizeRef.current = null;
      document.removeEventListener("mousemove", onMove);
      document.removeEventListener("mouseup", onUp);
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
      setTimeout(() => fitRef.current?.fit(), 50);
    };
    document.addEventListener("mousemove", onMove);
    document.addEventListener("mouseup", onUp);
    document.body.style.cursor = "se-resize";
    document.body.style.userSelect = "none";
  }, [size]);

  // Initialize xterm
  useEffect(() => {
    if (!containerRef.current) return;
    let disposed = false;

    (async () => {
      const [{ Terminal }, { FitAddon }] = await Promise.all([
        import("@xterm/xterm"),
        import("@xterm/addon-fit"),
      ]);
      // @ts-ignore
      try { await import("@xterm/xterm/css/xterm.css"); } catch { /* ok */ }

      let Unicode11Addon: any, WebLinksAddon: any, WebglAddon: any;
      try { Unicode11Addon = (await import("@xterm/addon-unicode11")).Unicode11Addon; } catch { /* ok */ }
      try { WebLinksAddon = (await import("@xterm/addon-web-links")).WebLinksAddon; } catch { /* ok */ }
      try { WebglAddon = (await import("@xterm/addon-webgl")).WebglAddon; } catch { /* ok */ }

      if (disposed || !containerRef.current) return;

      const settings = await getSetting<TerminalSettings>("terminalSettings", DEFAULT_TERMINAL_SETTINGS);
      const term = new Terminal({
        allowProposedApi: true,
        theme: {
          background: "#0d0f17",
          foreground: "#e0e0e0",
          cursor: "#a78bfa",
          selectionBackground: "#a78bfa40",
        },
        fontFamily: settings.fontFamily || DEFAULT_TERMINAL_SETTINGS.fontFamily,
        fontSize: settings.fontSize || DEFAULT_TERMINAL_SETTINGS.fontSize,
        lineHeight: settings.lineHeight || DEFAULT_TERMINAL_SETTINGS.lineHeight,
        cursorBlink: true,
        convertEol: true,
      });

      const fit = new FitAddon();
      term.loadAddon(fit);
      try { if (Unicode11Addon) { term.loadAddon(new Unicode11Addon()); term.unicode.activeVersion = "11"; } } catch { /* ok */ }
      try { if (WebLinksAddon) { term.loadAddon(new WebLinksAddon()); } } catch { /* ok */ }

      await Promise.allSettled([
        document.fonts.load(`400 ${settings.fontSize || 12}px "JetBrains Mono"`),
        document.fonts.load(`400 ${settings.fontSize || 12}px "D2Coding"`),
      ]);

      term.open(containerRef.current);
      if (WebglAddon) {
        try {
          const webgl = new WebglAddon();
          webgl.onContextLoss(() => { webgl.dispose(); });
          term.loadAddon(webgl);
        } catch { /* fallback to DOM renderer */ }
      }
      fit.fit();
      term.focus();

      const sid = usePtyStore.getState().getSession("claude");
      if (sid !== null) {
        invoke("pty_resize", { sessionId: sid, cols: term.cols, rows: term.rows }).catch(() => {});
      }

      termRef.current = term;
      fitRef.current = fit;

      term.onData((data: string) => {
        const sid = usePtyStore.getState().getSession("claude");
        if (sid !== null) invoke("pty_write", { sessionId: sid, data }).catch(console.error);
      });
      term.onResize(({ cols, rows }) => {
        const sid = usePtyStore.getState().getSession("claude");
        if (sid !== null) invoke("pty_resize", { sessionId: sid, cols, rows }).catch(() => {});
      });

      const ulOutput = await listen<{ sessionId: number; data: string }>("pty:output", (e) => {
        const sid = usePtyStore.getState().getSession("claude");
        if (sid !== null && e.payload.sessionId === sid) term.write(e.payload.data);
      });

      const resizeHandler = () => { fit.fit(); };
      window.addEventListener("resize", resizeHandler);
      cleanupRef.current = () => { window.removeEventListener("resize", resizeHandler); ulOutput(); };

      // Replay screen buffer
      const storeSessions = usePtyStore.getState().sessions;
      if (storeSessions.size === 0) {
        term.write(`\x1b[33m[No PTY sessions — select a project]\x1b[0m\r\n`);
      } else {
        for (const [engine, session] of storeSessions) {
          const screen = await invoke<string>("pty_get_screen", { sessionId: session.sessionId }).catch(() => "");
          if (screen?.trim()) {
            term.write(screen);
          } else {
            term.write(`\x1b[90m[${engine} session ${session.sessionId} active]\x1b[0m\r\n`);
          }
        }
      }
    })().catch(console.error);

    return () => {
      disposed = true;
      cleanupRef.current?.();
      termRef.current?.dispose();
      termRef.current = null;
      fitRef.current = null;
    };
  }, [claudeSession?.sessionId]);

  // Re-fit on size change
  useEffect(() => {
    const t = setTimeout(() => fitRef.current?.fit(), 50);
    return () => clearTimeout(t);
  }, [size]);

  return (
    <div
      className="fixed z-[60] flex flex-col rounded-lg border border-border/40 shadow-[0_8px_32px_-4px_rgba(0,0,0,0.6)] overflow-hidden"
      style={{ left: pos.x, top: pos.y, width: size.w, height: size.h }}
    >
      {/* Header — drag handle */}
      {/* eslint-disable-next-line jsx-a11y/no-static-element-interactions */}
      <div
        className="flex items-center gap-2 px-3 h-9 shrink-0 bg-[#0d0f17] border-b border-border/20 cursor-move select-none"
        onMouseDown={handleDragStart}
      >
        {/* Context */}
        <span className="text-[11px] font-medium text-foreground/80 truncate">{contextLabel}</span>
        <span className="text-[11px] text-prose-faint">·</span>
        <span className="text-[11px] text-prose-muted font-mono truncate">{engineLabel}</span>

        <div className="flex-1" />

        {/* Switch to docked */}
        <button
          onClick={() => setTerminalMode("docked")}
          className="flex items-center justify-center w-5 h-5 rounded text-prose-faint hover:text-foreground hover:bg-muted/30 transition-colors"
          title={t("terminal.dock_to_bottom")}
        >
          <PanelBottomOpen className="w-3 h-3" />
        </button>

        {/* Close */}
        <button
          onClick={toggleTerminal}
          className="flex items-center justify-center w-5 h-5 rounded text-prose-faint hover:text-foreground hover:bg-muted/30 transition-colors"
          title={t("terminal.close")}
        >
          <X className="w-3 h-3" />
        </button>
      </div>

      {/* Terminal body */}
      {/* eslint-disable-next-line jsx-a11y/click-events-have-key-events, jsx-a11y/no-static-element-interactions */}
      <div
        ref={containerRef}
        className="flex-1 min-h-0 p-1 bg-[#0d0f17]"
        onMouseDown={() => termRef.current?.focus()}
        onContextMenu={(e) => { e.preventDefault(); setMenu({ x: e.clientX, y: e.clientY }); }}
      />
      {menu && (
        <TerminalContextMenu
          x={menu.x} y={menu.y}
          onClear={() => { termRef.current?.clear(); setMenu(null); }}
          onClose={() => setMenu(null)}
        />
      )}

      {/* Bottom-right resize handle */}
      {/* eslint-disable-next-line jsx-a11y/no-static-element-interactions */}
      <div
        className="absolute bottom-0 right-0 w-4 h-4 cursor-se-resize"
        onMouseDown={handleResizeStart}
        style={{
          background: "linear-gradient(135deg, transparent 50%, rgba(255,255,255,0.08) 50%)",
        }}
      />
    </div>
  );
}

function TerminalContextMenu({ x, y, onClear, onClose }: { x: number; y: number; onClear: () => void; onClose: () => void }) {
  const { t } = useTranslation("common");
  useEffect(() => {
    const handler = () => onClose();
    window.addEventListener("mousedown", handler);
    return () => window.removeEventListener("mousedown", handler);
  }, [onClose]);

  return (
    <div
      className="fixed z-[100] min-w-[120px] py-1 rounded-md border border-border/40 bg-popover shadow-lg text-xs"
      style={{ left: x, top: y }}
      onMouseDown={(e) => e.stopPropagation()}
    >
      <button
        onClick={onClear}
        className="w-full text-left px-3 py-1.5 hover:bg-accent text-foreground transition-colors"
      >
        {t("terminal.clear_screen")}
      </button>
    </div>
  );
}
