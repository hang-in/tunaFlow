import { useEffect, useRef, useCallback, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useChatStore } from "@/stores/chatStore";
import { usePtyStore } from "@/stores/ptyStore";
import { RotateCcw } from "lucide-react";
import { getSetting } from "@/lib/appStore";

interface TerminalSettings {
  fontFamily: string;
  fontSize: number;
  lineHeight: number;
}
const DEFAULT_TERMINAL_SETTINGS: TerminalSettings = {
  fontFamily: "'JetBrains Mono', 'Consolas', monospace",
  fontSize: 12,
  lineHeight: 1.3,
};

// Lazy-loaded types
type XTerminal = import("@xterm/xterm").Terminal;
type XFitAddon = import("@xterm/addon-fit").FitAddon;

/**
 * Debug/monitoring terminal view.
 * Shows Claude's PTY output. Does NOT manage PTY lifecycle —
 * sessions are managed at project level by projectSlice.
 */
export function TerminalPanel() {
  const containerRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<XTerminal | null>(null);
  const fitRef = useRef<XFitAddon | null>(null);
  const cleanupRef = useRef<(() => void) | null>(null);

  const selectedProjectKey = useChatStore((s) => s.selectedProjectKey);
  const projects = useChatStore((s) => s.projects);
  const project = projects.find((p) => p.key === selectedProjectKey);
  const projectPath = project?.path;

  // Get Claude session from ptyStore (project-level managed)
  const claudeSession = usePtyStore((s) => s.sessions.get("claude"));

  // Initialize xterm.js and connect to existing PTY session
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

      let Unicode11Addon: any, WebLinksAddon: any;
      try { Unicode11Addon = (await import("@xterm/addon-unicode11")).Unicode11Addon; } catch { /* ok */ }
      try { WebLinksAddon = (await import("@xterm/addon-web-links")).WebLinksAddon; } catch { /* ok */ }

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
      term.open(containerRef.current);
      fit.fit();

      termRef.current = term;
      fitRef.current = fit;

      // User input → Claude PTY stdin (debug: direct interaction)
      term.onData((data: string) => {
        const sid = usePtyStore.getState().getSession("claude");
        if (sid !== null) {
          invoke("pty_write", { sessionId: sid, data }).catch(console.error);
        }
      });

      // Resize
      term.onResize(({ cols, rows }) => {
        const sid = usePtyStore.getState().getSession("claude");
        if (sid !== null) {
          invoke("pty_resize", { sessionId: sid, cols, rows }).catch(console.error);
        }
      });

      // Listen for PTY output from Claude session
      const ulOutput = await listen<{ sessionId: number; data: string }>("pty:output", (e) => {
        const sid = usePtyStore.getState().getSession("claude");
        if (sid !== null && e.payload.sessionId === sid) {
          term.write(e.payload.data);
        }
      });

      const resizeHandler = () => fit.fit();
      window.addEventListener("resize", resizeHandler);

      cleanupRef.current = () => {
        window.removeEventListener("resize", resizeHandler);
        ulOutput();
      };

      // Show session status
      const sessions = usePtyStore.getState().sessions;
      for (const [engine, session] of sessions) {
        term.write(`\x1b[90m[${engine} session ${session.sessionId} active]\x1b[0m\r\n`);
      }
      if (sessions.size === 0) {
        term.write(`\x1b[33m[No PTY sessions — select a project]\x1b[0m\r\n`);
      }
    })().catch(console.error);

    return () => {
      disposed = true;
      cleanupRef.current?.();
      termRef.current?.dispose();
      termRef.current = null;
      fitRef.current = null;
    };
  }, [claudeSession?.sessionId]); // Re-attach when Claude session changes

  // Re-fit when terminal becomes visible (project/session change)
  useEffect(() => {
    const timer = setTimeout(() => fitRef.current?.fit(), 50);
    return () => clearTimeout(timer);
  }, [selectedProjectKey, claudeSession?.sessionId]);

  const handleRestart = useCallback(async () => {
    // Kill and re-spawn Claude PTY only
    const sid = usePtyStore.getState().getSession("claude");
    if (sid !== null && projectPath) {
      await invoke("pty_kill", { sessionId: sid }).catch(() => {});
      usePtyStore.getState().clearSession("claude");
      try {
        const newSid = await invoke<number>("pty_spawn", {
          file: "claude", args: [], cwd: projectPath, cols: 80, rows: 24,
        });
        usePtyStore.getState().setSession("claude", newSid, projectPath);
      } catch (err) {
        console.error("[pty] restart failed:", err);
      }
    }
  }, [projectPath]);

  if (!selectedProjectKey || !projectPath) {
    return (
      <div className="flex items-center justify-center h-full text-prose-faint text-tf-sm">
        Terminal을 사용하려면 프로젝트를 선택하세요
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full overflow-hidden">
      <div className="flex items-center gap-2 px-3 py-1.5 border-b border-border/20 shrink-0">
        <span className="text-tf-xs text-prose-muted font-mono truncate flex-1">{projectPath}</span>
        <button
          onClick={handleRestart}
          className="flex items-center gap-1 text-tf-micro px-1.5 py-0.5 rounded text-prose-faint hover:text-foreground hover:bg-muted/30 transition-colors"
          title="Claude 재시작"
        >
          <RotateCcw className="w-3 h-3" />
          재시작
        </button>
      </div>
      <div ref={containerRef} className="flex-1 min-h-0 p-1" />
    </div>
  );
}
