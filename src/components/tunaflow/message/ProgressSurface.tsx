import { cn } from "@/lib/utils";

export function TypingIndicator() {
  return (
    <div className="flex items-center gap-1 py-1">
      <span className="typing-dot w-1.5 h-1.5 rounded-full bg-muted-foreground" />
      <span className="typing-dot w-1.5 h-1.5 rounded-full bg-muted-foreground" />
      <span className="typing-dot w-1.5 h-1.5 rounded-full bg-muted-foreground" />
    </div>
  );
}

/** Live thinking/tool block — shows during streaming with scrollable last N lines */
export function ThinkingBlock({ content, maxLines = 5 }: { content: string; maxLines?: number }) {
  const lines = content.split("\n").filter(Boolean);
  const visible = lines.slice(-maxLines);
  const truncated = lines.length > maxLines;
  return (
    <div className="rounded-md border border-border/30 bg-accent/30 px-3 py-2 mb-2">
      <div className="font-mono text-[11px] text-muted-foreground/70 leading-relaxed">
        {truncated && (
          <div className="text-[9px] text-muted-foreground/30 mb-1">… {lines.length - maxLines} lines above</div>
        )}
        {visible.map((line, i) => (
          <div key={i} className={cn(i === visible.length - 1 && "text-foreground/70")}>
            {line || "\u00A0"}
          </div>
        ))}
      </div>
      <div className="text-[9px] text-muted-foreground/30 mt-1">
        {lines.length} step{lines.length !== 1 ? "s" : ""} · thinking...
      </div>
    </div>
  );
}

/** Collapsed thinking summary — shows after response is complete */
export function ThinkingSummary({ content, elapsedMs }: { content: string; elapsedMs?: number }) {
  const lines = content.split("\n").filter(Boolean);
  if (lines.length === 0) return null;
  const collapsedLines = lines.slice(-3);
  const elapsed = elapsedMs != null ? formatElapsed(elapsedMs) : null;
  return (
    <details className="rounded-md border border-border/20 bg-accent/20 px-3 py-1.5 mb-2 group/thinking">
      <summary className="cursor-pointer text-[10px] text-muted-foreground/50 hover:text-muted-foreground/70 transition-colors flex items-center gap-1.5">
        <span className="font-medium">{lines.length} step{lines.length !== 1 ? "s" : ""}</span>
        {elapsed && <span>· {elapsed}</span>}
        <span className="text-[9px] text-muted-foreground/30 group-open/thinking:hidden">— {collapsedLines[collapsedLines.length - 1]}</span>
      </summary>
      <div className="mt-1.5 font-mono text-[10px] text-muted-foreground/50 leading-relaxed pl-2 border-l border-border/20">
        {lines.map((line, i) => <div key={i}>{line}</div>)}
      </div>
    </details>
  );
}

function formatElapsed(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  const s = ms / 1000;
  if (s < 60) return `${s.toFixed(1)}s`;
  const m = Math.floor(s / 60);
  const rem = Math.round(s % 60);
  return `${m}m ${rem}s`;
}

// Legacy exports for backward compatibility
export { ThinkingBlock as ProgressBlock };
export { ThinkingSummary as ProgressSummary };
