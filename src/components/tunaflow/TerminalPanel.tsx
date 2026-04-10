import { useEffect, useRef, useCallback, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useChatStore } from "@/stores/chatStore";
import { usePtyStore } from "@/stores/ptyStore";
import { RotateCcw } from "lucide-react";

// Lazy-loaded types (xterm.js is DOM-dependent)
type XTerminal = import("@xterm/xterm").Terminal;
type XFitAddon = import("@xterm/addon-fit").FitAddon;

export function TerminalPanel() {
  const containerRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<XTerminal | null>(null);
  const fitRef = useRef<XFitAddon | null>(null);
  const cleanupRef = useRef<(() => void) | null>(null);
  const sessionIdRef = useRef<number | null>(null);
  const [status, setStatus] = useState<"idle" | "starting" | "running" | "exited">("idle");

  const selectedProjectKey = useChatStore((s) => s.selectedProjectKey);
  const projects = useChatStore((s) => s.projects);
  const project = projects.find((p) => p.key === selectedProjectKey);
  const projectPath = project?.path;

  const startPty = useCallback(async (term: XTerminal) => {
    // Kill existing session
    if (sessionIdRef.current !== null) {
      try { await invoke("pty_kill", { sessionId: sessionIdRef.current }); } catch { /* ignore */ }
      sessionIdRef.current = null;
    }

    if (!projectPath) return;

    setStatus("starting");
    term.write(`\x1b[90m[Starting claude in ${projectPath}...]\x1b[0m\r\n`);

    try {
      // Listen for PTY output events
      const unlisten = await listen<{ sessionId: number; data: string }>("pty:output", (e) => {
        if (e.payload.sessionId === sessionIdRef.current) {
          term.write(e.payload.data);
        }
      });

      const unlistenExit = await listen<{ sessionId: number; exitCode: number | null }>("pty:exit", (e) => {
        if (e.payload.sessionId === sessionIdRef.current) {
          term.write(`\r\n\x1b[90m[Process exited]\x1b[0m\r\n`);
          sessionIdRef.current = null;
          setStatus("exited");
          usePtyStore.getState().clearSession();
        }
      });

      // Spawn PTY via Rust command
      const sessionId = await invoke<number>("pty_spawn", {
        file: "claude",
        args: [] as string[],
        cwd: projectPath,
        cols: term.cols,
        rows: term.rows,
      });

      sessionIdRef.current = sessionId;
      setStatus("running");
      usePtyStore.getState().setSession(sessionId, projectPath);

      // Store cleanup
      const prevCleanup = cleanupRef.current;
      cleanupRef.current = () => {
        unlisten();
        unlistenExit();
        prevCleanup?.();
      };
    } catch (err) {
      console.error("[pty] spawn failed:", err);
      term.write(`\r\n\x1b[31m[Failed to start: ${String(err)}]\x1b[0m\r\n`);
      setStatus("exited");
    }
  }, [projectPath]);

  // Initialize xterm.js + connect PTY
  useEffect(() => {
    if (!containerRef.current) return;
    let disposed = false;

    (async () => {
      const [{ Terminal }, { FitAddon }, { Unicode11Addon }, { WebLinksAddon }] = await Promise.all([
        import("@xterm/xterm"),
        import("@xterm/addon-fit"),
        import("@xterm/addon-unicode11"),
        import("@xterm/addon-web-links"),
      ]);
      // @ts-ignore — CSS import has no type declaration
      try { await import("@xterm/xterm/css/xterm.css"); } catch { /* ok */ }

      if (disposed || !containerRef.current) return;

      const term = new Terminal({
        allowProposedApi: true,
        theme: {
          background: "#0d0f17",
          foreground: "#e0e0e0",
          cursor: "#a78bfa",
          selectionBackground: "#a78bfa40",
        },
        fontFamily: "'JetBrains Mono', 'Consolas', monospace",
        fontSize: 13,
        lineHeight: 1.4,
        cursorBlink: true,
        convertEol: true,
      });

      const fit = new FitAddon();
      term.loadAddon(fit);
      try { term.loadAddon(new Unicode11Addon()); term.unicode.activeVersion = "11"; } catch { /* unicode11 not critical */ }
      try { term.loadAddon(new WebLinksAddon()); } catch { /* links not critical */ }
      term.open(containerRef.current);
      fit.fit();

      termRef.current = term;
      fitRef.current = fit;

      // User input → PTY stdin via Tauri invoke
      term.onData((data: string) => {
        if (sessionIdRef.current !== null) {
          invoke("pty_write", { sessionId: sessionIdRef.current, data }).catch(console.error);
        }
      });

      // Resize → PTY
      term.onResize(({ cols, rows }) => {
        if (sessionIdRef.current !== null) {
          invoke("pty_resize", { sessionId: sessionIdRef.current, cols, rows }).catch(console.error);
        }
      });

      // Window resize
      const resizeHandler = () => fit.fit();
      window.addEventListener("resize", resizeHandler);

      cleanupRef.current = () => {
        window.removeEventListener("resize", resizeHandler);
      };

      // Start PTY
      startPty(term);
    })().catch(console.error);

    return () => {
      disposed = true;
      cleanupRef.current?.();
      if (sessionIdRef.current !== null) {
        invoke("pty_kill", { sessionId: sessionIdRef.current }).catch(() => {});
        sessionIdRef.current = null;
      }
      termRef.current?.dispose();
      termRef.current = null;
      fitRef.current = null;
    };
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  // Restart PTY when project changes
  useEffect(() => {
    if (!termRef.current || !projectPath) return;
    termRef.current.clear();
    startPty(termRef.current);
  }, [projectPath, startPty]);

  // Re-fit on visibility
  useEffect(() => {
    const timer = setTimeout(() => fitRef.current?.fit(), 50);
    return () => clearTimeout(timer);
  });

  const handleRestart = useCallback(() => {
    if (!termRef.current) return;
    termRef.current.clear();
    startPty(termRef.current);
  }, [startPty]);

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
        <span className="text-tf-micro text-prose-disabled">{status}</span>
        <button
          onClick={handleRestart}
          className="flex items-center gap-1 text-tf-micro px-1.5 py-0.5 rounded text-prose-faint hover:text-foreground hover:bg-muted/30 transition-colors"
          title="재시작"
        >
          <RotateCcw className="w-3 h-3" />
          재시작
        </button>
      </div>
      <div ref={containerRef} className="flex-1 min-h-0 p-1" />
    </div>
  );
}
