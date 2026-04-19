import { useState, useRef, useEffect } from "react";
import { cn } from "@/lib/utils";
import type { ToolStep } from "@/lib/toolSteps";
import { formatStep, stepIcon } from "@/lib/toolSteps";
import { ChevronDown, ChevronRight, Loader2 } from "lucide-react";

function formatElapsed(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  const s = ms / 1000;
  if (s < 60) return `${s.toFixed(1)}s`;
  const m = Math.floor(s / 60);
  const rem = Math.round(s - m * 60);
  return `${m}m${rem}s`;
}

const LINE_HEIGHT = 20; // px per step line
const DEFAULT_LINES = 3;
const MAX_LINES = 5;

interface ToolStepsViewProps {
  steps: ToolStep[];
  isStreaming: boolean;
  durationMs?: number;
}

export function ToolStepsView({ steps, isStreaming, durationMs }: ToolStepsViewProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const [expanded, setExpanded] = useState(false);

  // Auto-scroll to bottom during streaming
  useEffect(() => {
    if (isStreaming && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [steps.length, isStreaming]);

  if (steps.length === 0) return null;

  // Completed — show last step, expandable
  if (!isStreaming) {
    const lastStep = steps[steps.length - 1];

    return (
      <div className="mb-1.5">
        <button
          onClick={() => setExpanded((prev) => !prev)}
          className="flex items-center gap-1.5 text-[10px] text-muted-foreground/60 hover:text-muted-foreground transition-colors"
        >
          {expanded ? <ChevronDown className="w-3 h-3" /> : <ChevronRight className="w-3 h-3" />}
          {!expanded && lastStep && (
            <span className="truncate">{stepIcon(lastStep)} {formatStep(lastStep)} <span className="ml-1 opacity-60">+{steps.length - 1}</span></span>
          )}
          {expanded && <span>{steps.length} steps</span>}
        </button>
        {expanded && (
          <div className="mt-1 pl-4 space-y-0.5 max-h-40 overflow-y-auto">
            {steps.map((step, i) => (
              <StepLine key={i} step={step} showOutput />
            ))}
          </div>
        )}
      </div>
    );
  }

  // Streaming — live view with scroll
  const visibleHeight = Math.min(steps.length, DEFAULT_LINES) * LINE_HEIGHT;
  const maxHeight = MAX_LINES * LINE_HEIGHT;

  return (
    <div className="mb-1.5">
      <div
        ref={scrollRef}
        className="overflow-y-auto font-mono"
        style={{
          height: visibleHeight,
          maxHeight,
        }}
      >
        {steps.map((step, i) => (
          <StepLine key={i} step={step} elapsedMs={durationMs} isLastRunning={isStreaming && step.status === "running" && i === steps.length - 1} />
        ))}
      </div>
    </div>
  );
}

function StepLine({ step, showOutput, elapsedMs, isLastRunning }: { step: ToolStep; showOutput?: boolean; elapsedMs?: number; isLastRunning?: boolean }) {
  const icon = stepIcon(step);
  const text = formatStep(step);
  const [outputOpen, setOutputOpen] = useState(false);
  const hasOutput = showOutput && step.output;

  return (
    <div>
      <div
        className={cn(
          "flex items-center gap-1.5 text-[10px] leading-[20px] whitespace-nowrap overflow-hidden text-ellipsis",
          step.status === "running" && "text-primary/80",
          step.status === "done" && "text-muted-foreground/50",
          step.status === "error" && "text-status-rejected/70",
          hasOutput && "cursor-pointer hover:text-muted-foreground/70",
        )}
        onClick={hasOutput ? () => setOutputOpen((v) => !v) : undefined}
      >
        <span className="shrink-0 w-3 text-center">
          {step.status === "running"
            ? <Loader2 className="w-2.5 h-2.5 animate-spin inline" />
            : icon}
        </span>
        <span className="truncate">{text}</span>
        {isLastRunning && elapsedMs !== undefined && elapsedMs > 500 && (
          <span className="shrink-0 ml-1 text-[9px] text-primary/60 font-mono tabular-nums">
            {formatElapsed(elapsedMs)}
          </span>
        )}
        {hasOutput && (
          <span className="shrink-0 text-[8px] opacity-40 ml-1">
            {outputOpen ? "▾" : "▸"}
          </span>
        )}
      </div>
      {outputOpen && step.output && (
        <pre className="ml-5 mt-0.5 mb-1 text-[9px] leading-tight text-muted-foreground/40 max-h-32 overflow-y-auto whitespace-pre-wrap break-all border-l border-border/20 pl-2">
          {step.output}
        </pre>
      )}
    </div>
  );
}
