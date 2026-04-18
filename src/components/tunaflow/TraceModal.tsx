import { useEffect, useState } from "react";
import { X } from "lucide-react";
import { cn } from "@/lib/utils";
import { TracePanel } from "./context-panel/TracePanel";
import { QualityDashboard } from "./context-panel/QualityDashboard";

interface TraceModalProps {
  onClose: () => void;
}

type Tab = "trace" | "quality";

export function TraceModal({ onClose }: TraceModalProps) {
  const [tab, setTab] = useState<Tab>("trace");

  // ESC to close
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    document.addEventListener("keydown", handler);
    return () => document.removeEventListener("keydown", handler);
  }, [onClose]);

  return (
    <div className="fixed inset-0 z-[70] flex items-center justify-center">
      {/* Backdrop */}
      <div className="absolute inset-0 bg-black/30 backdrop-blur-[1px]" onClick={onClose} />

      {/* Modal */}
      <div className="relative bg-card border border-border/40 rounded-lg shadow-2xl w-[80vw] max-w-[900px] max-h-[80vh] overflow-hidden flex flex-col">
        {/* Header with tabs */}
        <div className="flex items-center gap-1 px-4 h-10 border-b border-border/30 shrink-0">
          <span className="text-[13px] font-medium text-foreground mr-2">Runtime</span>
          {(["trace", "quality"] as Tab[]).map((t) => (
            <button
              key={t}
              onClick={() => setTab(t)}
              className={cn(
                "px-2 py-1 rounded text-[11px] transition-colors",
                tab === t
                  ? "bg-accent text-foreground"
                  : "text-muted-foreground/60 hover:text-foreground hover:bg-accent/40",
              )}
            >
              {t === "trace" ? "Trace" : "Quality"}
            </button>
          ))}
          <button
            onClick={onClose}
            className="ml-auto p-1 rounded hover:bg-accent text-muted-foreground hover:text-foreground transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto p-4">
          {tab === "trace" ? <TracePanel /> : <QualityDashboard />}
        </div>
      </div>
    </div>
  );
}
