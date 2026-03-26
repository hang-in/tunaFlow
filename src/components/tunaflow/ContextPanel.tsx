import { useState } from "react";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { ClipboardList, FileText, Activity, Zap, StickyNote } from "lucide-react";

import { ArtifactsPanel } from "./context-panel/ArtifactsPanel";
import { MemosPanel } from "./context-panel/MemosPanel";
import { SkillsPanel } from "./context-panel/SkillsPanel";
import { PlansPanel } from "./context-panel/PlansPanel";
import { TracePanel } from "./context-panel/TracePanel";
import { HarnessSummary } from "./context-panel/HarnessSummary";

/** Workspace modes — Plan / Artifacts / Trace (Phase 1 MVP) */
type WorkspaceMode = "plan" | "artifacts" | "trace";

const MODE_TABS: { id: WorkspaceMode; label: string; icon: React.ReactNode }[] = [
  { id: "plan", label: "Plan", icon: <ClipboardList className="w-3.5 h-3.5" /> },
  { id: "artifacts", label: "Artifacts", icon: <FileText className="w-3.5 h-3.5" /> },
  { id: "trace", label: "Trace", icon: <Activity className="w-3.5 h-3.5" /> },
];

export function ContextPanel() {
  const [mode, setMode] = useState<WorkspaceMode>("plan");
  const [memosOpen, setMemosOpen] = useState(false);
  const [skillsOpen, setSkillsOpen] = useState(false);
  const { artifacts, memos, selectedConversationId, activeBranchId, parentConversationId } = useChatStore();
  const canonicalConvId = activeBranchId && parentConversationId
    ? parentConversationId
    : selectedConversationId;

  return (
    <aside className="flex flex-col w-full h-full bg-sidebar overflow-hidden">

      {/* Mode bar */}
      <div className="flex items-center gap-0.5 px-2 h-9 border-b border-border/40 shrink-0">
        {MODE_TABS.map((tab) => (
          <button
            key={tab.id}
            onClick={() => setMode(tab.id)}
            className={cn(
              "flex items-center gap-1.5 px-2 py-1 rounded text-[10px] font-medium transition-colors",
              mode === tab.id
                ? "bg-accent text-foreground"
                : "text-muted-foreground/70 hover:text-foreground hover:bg-accent/50"
            )}
          >
            {tab.icon}
            {tab.label}
            {tab.id === "artifacts" && artifacts.length > 0 && (
              <span className="text-[8px] bg-primary/10 text-primary/70 px-1 rounded">
                {artifacts.length}
              </span>
            )}
          </button>
        ))}
      </div>

      {/* Panel content */}
      <div className="flex-1 overflow-y-auto p-3">
        {/* ─── Plan mode ─── */}
        {mode === "plan" && (
          <>
            {canonicalConvId && <HarnessSummary conversationId={canonicalConvId} />}
            <h3 className="text-[9px] font-semibold text-muted-foreground/50 uppercase tracking-widest mb-2.5">Plans</h3>
            <PlansPanel />
          </>
        )}

        {/* ─── Artifacts mode (with Memos + Skills collapsible) ─── */}
        {mode === "artifacts" && (
          <>
            <h3 className="text-[9px] font-semibold text-muted-foreground/50 uppercase tracking-widest mb-2.5">Artifacts</h3>
            <ArtifactsPanel />

            {/* Memos — collapsible section */}
            <div className="mt-4 border-t border-border/30 pt-3">
              <button
                onClick={() => setMemosOpen(!memosOpen)}
                className="flex items-center gap-1.5 w-full text-left mb-2"
              >
                <StickyNote className="w-3 h-3 text-muted-foreground" />
                <span className="text-[9px] font-semibold text-muted-foreground/50 uppercase tracking-widest flex-1">
                  Memos
                </span>
                {memos.length > 0 && (
                  <span className="text-[9px] bg-accent text-muted-foreground px-1 rounded-full">
                    {memos.length}
                  </span>
                )}
                <span className="text-[10px] text-muted-foreground">{memosOpen ? "▾" : "▸"}</span>
              </button>
              {memosOpen && <MemosPanel />}
            </div>

            {/* Skills — collapsible section */}
            <div className="mt-3 border-t border-border/30 pt-3">
              <button
                onClick={() => setSkillsOpen(!skillsOpen)}
                className="flex items-center gap-1.5 w-full text-left mb-2"
              >
                <Zap className="w-3 h-3 text-muted-foreground" />
                <span className="text-[9px] font-semibold text-muted-foreground/50 uppercase tracking-widest flex-1">
                  Skills
                </span>
                <span className="text-[10px] text-muted-foreground">{skillsOpen ? "▾" : "▸"}</span>
              </button>
              {skillsOpen && <SkillsPanel />}
            </div>
          </>
        )}

        {/* ─── Trace mode ─── */}
        {mode === "trace" && (
          <>
            <h3 className="text-[9px] font-semibold text-muted-foreground/50 uppercase tracking-widest mb-2.5">Trace</h3>
            <TracePanel />
          </>
        )}
      </div>
    </aside>
  );
}
