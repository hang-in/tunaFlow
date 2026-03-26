import { cn } from "@/lib/utils";
import { ChevronRight, ChevronDown } from "lucide-react";

// ─── Tree primitives ─────────────────────────────────────────────────────────

export const INDENT_PX = 20;
export const BASE_PAD = 10;
export const GUIDE_START = 18;

export function TreeRow({
  depth, active, isParent, icon, label, suffix, actions, onClick, className,
}: {
  depth: number; active?: boolean; isParent?: boolean; icon: React.ReactNode; label: React.ReactNode;
  suffix?: React.ReactNode; actions?: React.ReactNode; onClick?: () => void; className?: string;
}) {
  return (
    <div onClick={onClick}
      style={{ paddingLeft: BASE_PAD + depth * INDENT_PX }}
      className={cn(
        "group flex items-center h-[22px] cursor-pointer select-none transition-colors text-left pr-1.5 relative",
        active ? "bg-white/10 text-sidebar-foreground"
          : isParent ? "text-sidebar-foreground/80 hover:bg-white/[0.05] hover:text-sidebar-foreground"
          : "text-sidebar-foreground/60 hover:bg-white/[0.04] hover:text-sidebar-foreground/80",
        className,
      )}>
      {depth > 0 && Array.from({ length: depth }).map((_, i) => (
        <span key={i} className="absolute top-0 bottom-0 w-px"
          style={{ left: GUIDE_START + i * INDENT_PX, backgroundColor: "rgba(255,255,255,0.12)" }} />
      ))}
      <span className="shrink-0 w-4 flex items-center justify-center">{icon}</span>
      <span className="flex-1 min-w-0 text-[11px] truncate pl-1 pr-1">{label}</span>
      {suffix}
      {actions && (
        <span className="shrink-0 flex items-center gap-0.5 opacity-0 group-hover:opacity-100 transition-opacity">
          {actions}
        </span>
      )}
    </div>
  );
}

export function SectionHeader({ title, expanded, onToggle, actions, className }: {
  title: string; expanded: boolean; onToggle: () => void; actions?: React.ReactNode; className?: string;
}) {
  return (
    <div onClick={onToggle}
      className={cn("group flex items-center h-[22px] px-2 mt-3 first:mt-1 cursor-pointer select-none hover:bg-white/[0.04] transition-colors", className)}>
      {expanded ? <ChevronDown className="w-3 h-3 text-sidebar-foreground/40 shrink-0" />
        : <ChevronRight className="w-3 h-3 text-sidebar-foreground/40 shrink-0" />}
      <span className="text-[10px] font-semibold uppercase tracking-wider text-sidebar-foreground/50 pl-1 flex-1">{title}</span>
      {actions && <span className="shrink-0 flex items-center gap-0.5 opacity-0 group-hover:opacity-100 transition-opacity">{actions}</span>}
    </div>
  );
}
