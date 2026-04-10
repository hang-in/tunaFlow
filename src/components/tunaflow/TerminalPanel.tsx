import { useEffect, useRef, useCallback } from "react";
import { useChatStore } from "@/stores/chatStore";
import { RotateCcw } from "lucide-react";

// Lazy-loaded types (tauri-pty and xterm.js are Tauri-only, not available in test/browser env)
type XTerminal = import("@xterm/xterm").Terminal;
type XFitAddon = import("@xterm/addon-fit").FitAddon;
type IPty = { write: (data: string) => void; kill: () => void; resize: (cols: number, rows: number) => void; onData: (cb: (data: Uint8Array) => void) => void; onExit: (cb: (e: { exitCode: number }) => void) => void };

export function TerminalPanel() {
  const containerRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<XTerminal | null>(null);
  const ptyRef = useRef<IPty | null>(null);
  const fitRef = useRef<XFitAddon | null>(null);

  const cleanupRef = useRef<(() => void) | null>(null);

  const selectedProjectKey = useChatStore((s) => s.selectedProjectKey);
  const projects = useChatStore((s) => s.projects);
  const project = projects.find((p) => p.key === selectedProjectKey);
  const projectPath = project?.path;

  const startPty = useCallback(async (term: XTerminal) => {
    // Kill existing PTY if any
    if (ptyRef.current) {
      try { ptyRef.current.kill(); } catch { /* ignore */ }
      ptyRef.current = null;
    }

    if (!projectPath) return;

    try {
      const { spawn } = await import("tauri-pty");
      const pty = await spawn("claude", [], {
        cols: term.cols,
        rows: term.rows,
        cwd: projectPath,
      });

      ptyRef.current = pty;

      // PTY → Terminal (process output)
      pty.onData((data: Uint8Array) => {
        const str = new TextDecoder().decode(data);
        term.write(str);
      });

      // PTY exit
      pty.onExit((event: { exitCode: number }) => {
        term.write(`\r\n\x1b[90m[Process exited with code ${event.exitCode}]\x1b[0m\r\n`);
        ptyRef.current = null;
      });
    } catch (err) {
      term.write(`\r\n\x1b[31m[Failed to start claude: ${err}]\x1b[0m\r\n`);
    }
  }, [projectPath]);

  // Initialize terminal + PTY (dynamic imports for Tauri-only deps)
  useEffect(() => {
    if (!containerRef.current) return;
    let disposed = false;

    (async () => {
      const [{ Terminal }, { FitAddon }] = await Promise.all([
        import("@xterm/xterm"),
        import("@xterm/addon-fit"),
      ]);
      if (disposed || !containerRef.current) return;

      const term = new Terminal({
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
      term.open(containerRef.current);
      fit.fit();

      termRef.current = term;
      fitRef.current = fit;

      // Terminal → PTY (user input)
      term.onData((data: string) => {
        ptyRef.current?.write(data);
      });

      // Resize sync
      term.onResize(({ cols, rows }) => {
        ptyRef.current?.resize(cols, rows);
      });

      // Window resize
      const resizeHandler = () => fit.fit();
      window.addEventListener("resize", resizeHandler);

      // Cleanup closure
      const cleanup = () => {
        window.removeEventListener("resize", resizeHandler);
        if (ptyRef.current) {
          try { ptyRef.current.kill(); } catch { /* cleanup */ }
          ptyRef.current = null;
        }
        term.dispose();
        termRef.current = null;
        fitRef.current = null;
      };

      // Store cleanup for the effect's return
      cleanupRef.current = cleanup;

      // Start PTY
      startPty(term);
    })().catch(console.error);

    return () => {
      disposed = true;
      cleanupRef.current?.();
    };
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  // Restart PTY when project changes
  useEffect(() => {
    if (!termRef.current || !projectPath) return;
    termRef.current.clear();
    termRef.current.write(`\x1b[90m[Project: ${projectPath}]\x1b[0m\r\n`);
    startPty(termRef.current);
  }, [projectPath, startPty]);

  // Re-fit on visibility (tab switch)
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
      {/* Toolbar */}
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

      {/* Terminal container */}
      <div ref={containerRef} className="flex-1 min-h-0 p-1" />
    </div>
  );
}
