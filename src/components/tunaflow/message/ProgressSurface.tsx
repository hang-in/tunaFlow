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

export function ProgressBlock({ content, maxLines = 8 }: { content: string; maxLines?: number }) {
  const lines = content.split("\n");
  const visible = lines.slice(-maxLines);
  const truncated = lines.length > maxLines;
  return (
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
  );
}

export function ProgressSummary({ content }: { content: string }) {
  const lines = content.split("\n").filter(Boolean);
  if (lines.length === 0) return null;
  return (
    <details className="mb-2">
      <summary className="cursor-pointer text-[9px] text-muted-foreground/40 hover:text-muted-foreground/60 transition-colors">
        {lines.length} steps
      </summary>
      <div className="mt-1 font-mono text-[10px] text-muted-foreground/50 leading-relaxed pl-2 border-l border-border/20">
        {lines.map((line, i) => <div key={i}>{line}</div>)}
      </div>
    </details>
  );
}
