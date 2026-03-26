import { cn } from "@/lib/utils";
import { normalizeEngine } from "@/lib/utils";
import { User } from "lucide-react";

const ENGINE_ICONS: Record<string, string> = {
  claude: "/_resource/claude.png",
  codex: "/_resource/gpt.png",
  gemini: "/_resource/gemini.png",
  opencode: "/_resource/opencode.png",
};

interface AgentAvatarProps {
  engine?: string | null;
  isUser?: boolean;
  size?: "sm" | "md";
  className?: string;
}

export function AgentAvatar({ engine, isUser, size = "md", className }: AgentAvatarProps) {
  const dim = size === "sm" ? "w-6 h-6" : "w-8 h-8";
  const iconDim = size === "sm" ? "w-3 h-3" : "w-4 h-4";

  if (isUser) {
    return (
      <div className={cn(dim, "rounded-full bg-foreground/8 flex items-center justify-center shrink-0", className)}>
        <User className={cn(iconDim, "text-foreground/50")} />
      </div>
    );
  }

  const normalized = normalizeEngine(engine ?? undefined);
  const iconSrc = normalized ? ENGINE_ICONS[normalized] : null;

  if (iconSrc) {
    return (
      <img src={iconSrc} alt={normalized ?? "agent"}
        className={cn(dim, "rounded-full object-cover shrink-0", className)} />
    );
  }

  return (
    <div className={cn(dim, "rounded-full bg-accent flex items-center justify-center shrink-0 text-foreground/60 text-[11px] font-medium", className)}>
      ?
    </div>
  );
}
