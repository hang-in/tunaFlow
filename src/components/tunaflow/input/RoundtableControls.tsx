import { cn } from "@/lib/utils";
import { Users } from "lucide-react";
import { ROUNDTABLE_PARTICIPANTS } from "@/lib/constants";
import type { RtMode } from "@/types";

const RT_MODES: { id: RtMode; label: string; title: string }[] = [
  { id: "sequential", label: "Sequential", title: "Each agent sees prior agents' replies within the round" },
  { id: "deliberative", label: "Deliberative", title: "Round 1 is independent; Round 2+ reflects on all prior-round answers" },
];

interface RoundtableControlsProps {
  rtMode: RtMode;
  setRtMode: (m: RtMode) => void;
  activeParticipants: Set<string>;
  toggleParticipant: (name: string) => void;
}

export function RoundtableControls({ rtMode, setRtMode, activeParticipants, toggleParticipant }: RoundtableControlsProps) {
  return (
    <>
      <span className="flex items-center gap-1 text-[10px] px-1.5 py-0.5 rounded font-medium bg-agent-gemini/8 text-agent-gemini/70 shrink-0">
        <Users className="w-2.5 h-2.5" />
        RT
      </span>
      <span className="h-3 w-px bg-border/30" />
      <select
        value={rtMode}
        onChange={(e) => setRtMode(e.target.value as RtMode)}
        className="bg-transparent rounded px-1 py-0.5 text-[10px] outline-none text-muted-foreground/60 shrink-0"
        title={RT_MODES.find((m) => m.id === rtMode)?.title}
      >
        {RT_MODES.map((m) => (
          <option key={m.id} value={m.id}>{m.label}</option>
        ))}
      </select>
      <span className="h-3 w-px bg-border/30" />
      <div className="flex items-center gap-0.5 flex-wrap">
        {ROUNDTABLE_PARTICIPANTS.map((p) => {
          const active = activeParticipants.has(p.name);
          return (
            <button
              key={p.name}
              onClick={() => toggleParticipant(p.name)}
              className={cn(
                "text-[9px] px-1 py-0.5 rounded transition-colors",
                active
                  ? "text-foreground/70 bg-accent font-medium"
                  : "text-muted-foreground/30 line-through"
              )}
            >
              {p.name}
            </button>
          );
        })}
      </div>
    </>
  );
}
