import { Clock } from "lucide-react";
import type { PlanEvent } from "@/types";

export function EventTimeline({ events }: { events: PlanEvent[] }) {
  if (events.length === 0) return null;
  return (
    <div className="mt-2 pt-2 border-t border-border/20">
      <div className="flex items-center gap-1 mb-1">
        <Clock className="w-3 h-3 text-muted-foreground/40" />
        <span className="text-[9px] text-muted-foreground/50 uppercase tracking-wide">Timeline</span>
      </div>
      <div className="space-y-0.5">
        {events.map((ev) => {
          const d = new Date(ev.createdAt * 1000);
          const ts = `${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")} ${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
          return (
            <div key={ev.id} className="flex items-start gap-1.5 text-[9px] text-muted-foreground/60">
              <span className="shrink-0 text-muted-foreground/40">{ts}</span>
              <span>
                {ev.eventType.replace(/_/g, " ")}
                {ev.actor && <span className="text-foreground/50"> ({ev.actor})</span>}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}
