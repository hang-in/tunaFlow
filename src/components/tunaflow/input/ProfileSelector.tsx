import { useState, useRef, useEffect } from "react";
import { cn } from "@/lib/utils";
import { ChevronDown, Wrench } from "lucide-react";
import { AgentAvatar } from "../AgentAvatar";
import type { AgentProfile } from "@/types";

interface ProfileSelectorProps {
  profiles: AgentProfile[];
  selectedProfileId: string | null; // null = custom mode
  onSelectProfile: (id: string | null) => void;
}

export function ProfileSelector({ profiles, selectedProfileId, onSelectProfile }: ProfileSelectorProps) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [open]);

  const selected = selectedProfileId ? profiles.find((p) => p.id === selectedProfileId) : null;
  const isCustom = !selectedProfileId;

  return (
    <div className="relative" ref={ref}>
      <button
        onClick={() => setOpen((v) => !v)}
        className={cn(
          "flex items-center gap-1.5 px-2 py-1 rounded-md text-[11px] font-medium transition-colors",
          open ? "bg-accent text-foreground" : "text-muted-foreground/70 hover:text-foreground hover:bg-accent/50"
        )}
      >
        {isCustom ? (
          <>
            <Wrench className="w-3 h-3" />
            <span>Custom</span>
          </>
        ) : (
          <>
            <AgentAvatar engine={selected?.engine ?? "claude"} size="xs" />
            <span className="truncate max-w-[120px]">{selected?.label ?? "Agent"}</span>
          </>
        )}
        <ChevronDown className="w-3 h-3 text-muted-foreground/40" />
      </button>

      {open && (
        <div className="absolute left-0 top-full mt-1 w-[220px] bg-popover border border-border/40 rounded-lg shadow-xl overflow-hidden z-50">
          <div className="py-1">
            {profiles.map((p) => (
              <button
                key={p.id}
                onClick={() => { onSelectProfile(p.id); setOpen(false); }}
                className={cn(
                  "w-full flex items-center gap-2 px-3 py-1.5 text-[12px] text-left transition-colors",
                  selectedProfileId === p.id ? "bg-accent text-foreground" : "text-foreground/70 hover:bg-accent/50"
                )}
              >
                <AgentAvatar engine={p.engine} size="xs" />
                <span className="flex-1 truncate font-medium">{p.label}</span>
                <span className="text-[9px] text-muted-foreground/40">{p.engine}</span>
              </button>
            ))}
          </div>
          <div className="border-t border-border/30 py-1">
            <button
              onClick={() => { onSelectProfile(null); setOpen(false); }}
              className={cn(
                "w-full flex items-center gap-2 px-3 py-1.5 text-[12px] text-left transition-colors",
                isCustom ? "bg-accent text-foreground" : "text-foreground/70 hover:bg-accent/50"
              )}
            >
              <Wrench className="w-3.5 h-3.5 text-muted-foreground/40" />
              <span className="font-medium">Custom</span>
              <span className="text-[9px] text-muted-foreground/40">manual engine/model</span>
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
