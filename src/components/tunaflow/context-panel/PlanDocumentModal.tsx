import { useState, useEffect } from "react";
import { X, Clock, ClipboardList } from "lucide-react";
import { cn } from "@/lib/utils";
import type { Plan, PlanEvent, PlanSubtask } from "@/types";
import * as planApi from "@/lib/api/plans";
import { PLAN_PHASE_CFG } from "./plans/constants";

interface PlanDocumentModalProps {
  plan: Plan;
  onClose: () => void;
}

export function PlanDocumentModal({ plan, onClose }: PlanDocumentModalProps) {
  const [subtasks, setSubtasks] = useState<PlanSubtask[]>([]);
  const [events, setEvents] = useState<PlanEvent[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    setLoading(true);
    Promise.all([
      planApi.listSubtasks(plan.id),
      planApi.listPlanEvents(plan.id),
    ]).then(([sts, evs]) => {
      setSubtasks(sts);
      setEvents(evs);
    }).catch(() => {}).finally(() => setLoading(false));
  }, [plan.id]);

  const phaseCfg = PLAN_PHASE_CFG[plan.phase];

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50" onClick={onClose}>
      <div className="bg-popover border border-border rounded-xl shadow-2xl w-[600px] max-h-[80vh] overflow-hidden flex flex-col" onClick={(e) => e.stopPropagation()}>
        {/* Header */}
        <div className="flex items-center gap-2 px-5 py-3 border-b border-border/40 shrink-0">
          <ClipboardList className="w-4 h-4 text-primary/60" />
          <span className="text-sm font-medium text-foreground flex-1">{plan.title}</span>
          {plan.revision > 0 && (
            <span className="text-[9px] font-mono text-muted-foreground/50 px-1.5 rounded bg-accent/50">rev.{plan.revision}</span>
          )}
          <span className={cn("text-[9px] font-semibold px-1.5 py-0.5 rounded-full border", phaseCfg.cls)}>
            {phaseCfg.label}
          </span>
          <button onClick={onClose} className="p-1 rounded text-muted-foreground/50 hover:text-foreground hover:bg-accent transition-colors">
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Body */}
        <div className="flex-1 overflow-y-auto px-5 py-4 space-y-4">
          {loading ? (
            <p className="text-xs text-muted-foreground">Loading...</p>
          ) : (
            <>
              {/* Description */}
              {plan.description && (
                <div>
                  <h4 className="text-[10px] text-muted-foreground/60 uppercase tracking-wide mb-1">Description</h4>
                  <p className="text-xs text-foreground/80 leading-relaxed whitespace-pre-wrap">{plan.description}</p>
                </div>
              )}

              {/* Expected Outcome */}
              {plan.expectedOutcome && (
                <div>
                  <h4 className="text-[10px] text-muted-foreground/60 uppercase tracking-wide mb-1">Expected Outcome</h4>
                  <p className="text-xs text-foreground/80 leading-relaxed whitespace-pre-wrap">{plan.expectedOutcome}</p>
                </div>
              )}

              {/* Subtasks */}
              <div>
                <h4 className="text-[10px] text-muted-foreground/60 uppercase tracking-wide mb-2">
                  Subtasks ({subtasks.length})
                </h4>
                <div className="space-y-2.5">
                  {subtasks.map((st, i) => (
                    <div key={st.id} className="rounded-md border border-border/40 bg-card/50 p-3">
                      <div className="flex items-start gap-2">
                        <span className="text-[10px] text-muted-foreground/40 font-mono shrink-0 mt-0.5 w-4 text-right">{i + 1}.</span>
                        <div className="flex-1 min-w-0">
                          <p className="text-[11px] font-medium text-foreground">{st.title}</p>
                          {st.details ? (
                            <p className="text-[10px] text-muted-foreground leading-relaxed mt-1 whitespace-pre-wrap">{st.details}</p>
                          ) : (
                            <p className="text-[10px] text-amber-600/50 italic mt-1">상세 설계 미작성</p>
                          )}
                          {st.ownerAgent && (
                            <span className="inline-block mt-1 text-[8px] text-muted-foreground/40 bg-accent/50 px-1 rounded">{st.ownerAgent}</span>
                          )}
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              </div>

              {/* Revision History */}
              {events.length > 0 && (
                <div>
                  <h4 className="text-[10px] text-muted-foreground/60 uppercase tracking-wide mb-2 flex items-center gap-1">
                    <Clock className="w-3 h-3" />Revision History
                  </h4>
                  <div className="space-y-1 border-l-2 border-border/30 pl-3">
                    {events.map((ev) => {
                      const d = new Date(ev.createdAt * 1000);
                      const ts = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")} ${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
                      return (
                        <div key={ev.id} className="text-[10px] text-muted-foreground/60">
                          <span className="text-muted-foreground/30 font-mono">{ts}</span>
                          {" — "}
                          <span>{ev.eventType.replace(/_/g, " ")}</span>
                          {ev.actor && <span className="text-foreground/40"> ({ev.actor})</span>}
                          {ev.detail && <span className="text-muted-foreground/40"> — {ev.detail.slice(0, 80)}</span>}
                        </div>
                      );
                    })}
                  </div>
                </div>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
