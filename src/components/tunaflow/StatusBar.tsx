import { Users, MessageSquare, GitBranch, Zap, Link2 } from "lucide-react";

interface StatusBarProps {
  mode: "chat" | "roundtable";
  branch?: { id: string; label: string } | null;
  agentCount?: number;
  activeSkills?: number;
  crossSessionCount?: number;
}

export function StatusBar({
  mode,
  branch,
  agentCount = 3,
  activeSkills = 0,
  crossSessionCount = 0,
}: StatusBarProps) {
  return (
    <div className="flex items-center gap-3 px-4 h-7 bg-card/50 border-b border-border/50 text-[10px] text-muted-foreground shrink-0">
      <span className="flex items-center gap-1.5">
        {mode === "roundtable" ? (
          <Users className="w-3 h-3" />
        ) : (
          <MessageSquare className="w-3 h-3" />
        )}
        {mode === "roundtable" ? "Roundtable" : "Chat"}
      </span>

      {branch && (
        <>
          <span className="w-px h-3 bg-border/50" />
          <span className="flex items-center gap-1 text-primary/80">
            <GitBranch className="w-3 h-3" />
            {branch.label}
          </span>
        </>
      )}

      {mode === "roundtable" && (
        <>
          <span className="w-px h-3 bg-border/50" />
          <span className="flex items-center gap-1">
            <span className="w-1.5 h-1.5 rounded-full bg-agent-claude" />
            {agentCount} agents
          </span>
        </>
      )}

      {activeSkills > 0 && (
        <>
          <span className="w-px h-3 bg-border/50" />
          <span className="flex items-center gap-1">
            <Zap className="w-3 h-3" />
            {activeSkills}
          </span>
        </>
      )}

      {crossSessionCount > 0 && (
        <>
          <span className="w-px h-3 bg-border/50" />
          <span className="flex items-center gap-1">
            <Link2 className="w-3 h-3" />
            {crossSessionCount} ctx
          </span>
        </>
      )}
    </div>
  );
}
