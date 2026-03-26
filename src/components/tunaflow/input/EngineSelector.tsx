import { useRef, useEffect, useState } from "react";
import { cn } from "@/lib/utils";
import { ChevronDown } from "lucide-react";

type Engine = "claude" | "codex" | "gemini" | "opencode";

const ENGINE_LIST: { id: Engine; label: string; color: string }[] = [
  { id: "claude", label: "Claude", color: "text-agent-claude" },
  { id: "codex", label: "Codex", color: "text-agent-codex" },
  { id: "gemini", label: "Gemini", color: "text-agent-gemini" },
  { id: "opencode", label: "OpenCode", color: "text-agent-opencode" },
];

interface EngineSelectorProps {
  engine: Engine;
  setEngine: (e: Engine) => void;
}

export type { Engine };
export { ENGINE_LIST };

export function EngineSelector({ engine, setEngine }: EngineSelectorProps) {
  const [showDropdown, setShowDropdown] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);

  // Close dropdown on click outside
  useEffect(() => {
    if (!showDropdown) return;
    const handle = (e: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
        setShowDropdown(false);
      }
    };
    document.addEventListener("mousedown", handle);
    return () => document.removeEventListener("mousedown", handle);
  }, [showDropdown]);

  const selectedEngine = ENGINE_LIST.find((e) => e.id === engine)!;

  return (
    <div className="relative" ref={dropdownRef}>
      <button
        onClick={() => setShowDropdown(!showDropdown)}
        className="flex items-center gap-1 text-[10px] text-muted-foreground/60 hover:text-foreground transition-colors px-1.5 py-0.5 rounded hover:bg-accent/50"
      >
        <span className={cn("w-1.5 h-1.5 rounded-full", `bg-agent-${engine}`)} />
        <span className="font-medium">{selectedEngine.label}</span>
        <ChevronDown className="w-2.5 h-2.5" />
      </button>
      {showDropdown && (
        <div className="absolute bottom-full left-0 mb-1.5 bg-popover border border-border/40 rounded-md shadow-lg p-0.5 min-w-[120px] z-50">
          {ENGINE_LIST.map((eng) => (
            <button
              key={eng.id}
              onClick={() => { setEngine(eng.id); setShowDropdown(false); }}
              className={cn(
                "w-full flex items-center gap-1.5 px-2 py-1 rounded text-[10px] transition-colors",
                engine === eng.id ? "text-foreground bg-accent" : "text-muted-foreground hover:text-foreground hover:bg-accent"
              )}
            >
              <span className={cn("w-1.5 h-1.5 rounded-full shrink-0", engine === eng.id ? `bg-agent-${eng.id}` : "bg-muted")} />
              {eng.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
