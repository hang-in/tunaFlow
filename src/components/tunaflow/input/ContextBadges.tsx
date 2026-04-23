import { useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Zap, Link2, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { cn } from "@/lib/utils";

interface ContextBadgesProps {
  activeSkills: string[];
  crossSessionIds: string[];
}

function shortName(name: string): string {
  const idx = name.indexOf("-");
  return idx > 0 ? name.slice(idx + 1) : name;
}

function SkillsModal({ skills, onClose }: { skills: string[]; onClose: () => void }) {
  const { t } = useTranslation("common");
  return createPortal(
    <div
      className="fixed inset-0 z-[200] flex items-end justify-center pb-24 px-4"
      onClick={onClose}
    >
      <div
        className="bg-card border border-border/40 rounded-lg shadow-2xl p-3 w-full max-w-xs"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between mb-2">
          <span className="text-[10px] font-medium text-foreground/70 flex items-center gap-1">
            <Zap className="w-2.5 h-2.5 text-status-draft/60" />
            {t("context_badges.active_skills", { count: skills.length })}
          </span>
          <button onClick={onClose} className="text-muted-foreground/40 hover:text-foreground">
            <X className="w-3 h-3" />
          </button>
        </div>
        <div className="flex flex-wrap gap-1">
          {skills.map((name) => (
            <span key={name} className="text-[9px] text-status-draft/60 bg-status-draft/8 px-1.5 py-0.5 rounded">
              {shortName(name)}
            </span>
          ))}
        </div>
      </div>
    </div>,
    document.body,
  );
}

export function ContextBadges({ activeSkills, crossSessionIds }: ContextBadgesProps) {
  const { t } = useTranslation("common");
  const containerRef = useRef<HTMLDivElement>(null);
  const [isOverflow, setIsOverflow] = useState(false);
  const [modalOpen, setModalOpen] = useState(false);

  useLayoutEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const check = () => setIsOverflow(el.scrollWidth > el.clientWidth + 1);
    check();
    const ro = new ResizeObserver(check);
    ro.observe(el);
    return () => ro.disconnect();
  }, [activeSkills]);

  if (activeSkills.length === 0 && crossSessionIds.length === 0) return null;

  return (
    <div className="flex items-center gap-1 mb-1 min-w-0">
      {activeSkills.length > 0 && (
        <>
          {/* Clipping container — badges overflow-hidden here */}
          <div ref={containerRef} className="flex items-center gap-1 overflow-hidden min-w-0 flex-1">
            <Zap className="w-2.5 h-2.5 text-status-draft/50 shrink-0" />
            {activeSkills.map((name) => (
              <span
                key={name}
                className="text-[8px] text-status-draft/50 bg-status-draft/6 px-1 py-px rounded shrink-0"
              >
                {shortName(name)}
              </span>
            ))}
          </div>
          {/* Overflow indicator — outside clip container, always visible when needed */}
          {isOverflow && (
            <button
              onClick={() => setModalOpen(true)}
              className={cn(
                "shrink-0 text-[9px] font-mono text-status-draft/40 hover:text-status-draft/70 px-0.5 transition-colors",
              )}
              title={t("context_badges.show_all")}
            >
              …
            </button>
          )}
        </>
      )}
      {crossSessionIds.length > 0 && (
        <span className="inline-flex items-center gap-1 text-[9px] font-medium text-primary/50 bg-primary/6 px-1 py-0.5 rounded shrink-0">
          <Link2 className="w-2.5 h-2.5" />
          {crossSessionIds.length} linked
        </span>
      )}
      {modalOpen && <SkillsModal skills={activeSkills} onClose={() => setModalOpen(false)} />}
    </div>
  );
}
