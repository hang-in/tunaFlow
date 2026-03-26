import { Zap, Link2 } from "lucide-react";

interface ContextBadgesProps {
  activeSkills: string[];
  crossSessionIds: string[];
}

export function ContextBadges({ activeSkills, crossSessionIds }: ContextBadgesProps) {
  if (activeSkills.length === 0 && crossSessionIds.length === 0) return null;

  return (
    <div className="flex items-center gap-1.5 mb-1 flex-wrap">
      {activeSkills.length > 0 && (
        <span className="inline-flex items-center gap-1 text-[9px] font-medium text-status-draft/60 bg-status-draft/6 px-1 py-0.5 rounded">
          <Zap className="w-2.5 h-2.5" />
          {activeSkills.length} skill{activeSkills.length > 1 ? "s" : ""}
        </span>
      )}
      {crossSessionIds.length > 0 && (
        <span className="inline-flex items-center gap-1 text-[9px] font-medium text-primary/50 bg-primary/6 px-1 py-0.5 rounded">
          <Link2 className="w-2.5 h-2.5" />
          {crossSessionIds.length} linked
        </span>
      )}
    </div>
  );
}
