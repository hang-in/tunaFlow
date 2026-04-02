import { useState, useRef, useEffect } from "react";
import { cn } from "@/lib/utils";
import type { ToolStep } from "@/lib/toolSteps";
import { formatStep, stepIcon } from "@/lib/toolSteps";
import { ChevronDown, ChevronRight } from "lucide-react";

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

  // Completed — collapsed summary
  if (!isStreaming) {
    const doneCount = steps.filter((s) => s.status === "done").length;
    const errorCount = steps.filter((s) => s.status === "error").length;
    const durationStr = durationMs ? `${(durationMs / 1000).toFixed(1)}s` : "";

    return (
      <div className="mb-1.5">
        <button
          onClick={() => setExpanded((prev) => !prev)}
          className="flex items-center gap-1.5 text-[10px] text-muted-foreground/60 hover:text-muted-foreground transition-colors"
        >
          {expanded ? <ChevronDown className="w-3 h-3" /> : <ChevronRight className="w-3 h-3" />}
          <span>
            {steps.length} steps
            {errorCount > 0 && <span className="text-status-rejected ml-1">({errorCount} failed)</span>}
            {durationStr && <span className="ml-1">· {durationStr}</span>}
          </span>
        </button>
        {expanded && (
          <div className="mt-1 pl-4 space-y-0.5">
            {steps.map((step, i) => (
              <StepLine key={i} step={step} />
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
          <StepLine key={i} step={step} />
        ))}
      </div>
    </div>
  );
}

function StepLine({ step }: { step: ToolStep }) {
  const icon = stepIcon(step);
  const text = formatStep(step);

  return (
    <div
      className={cn(
        "flex items-center gap-1.5 text-[10px] leading-[20px] whitespace-nowrap overflow-hidden text-ellipsis",
        step.status === "running" && "text-primary/70 animate-pulse",
        step.status === "done" && "text-muted-foreground/50",
        step.status === "error" && "text-status-rejected/70",
      )}
    >
      <span className="shrink-0 w-3 text-center">{icon}</span>
      <span className="truncate">{text}</span>
    </div>
  );
}
