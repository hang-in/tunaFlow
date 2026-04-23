import { useState, useRef, useEffect } from "react";
import { Bell, Volume2, VolumeX } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { useNotificationStore, type AppNotification } from "@/stores/notificationStore";

function timeAgo(ts: number, t: TFunction): string {
  const diff = Math.floor((Date.now() - ts) / 1000);
  if (diff < 60) return t("notification_bell.just_now");
  if (diff < 3600) return t("notification_bell.minutes_ago", { count: Math.floor(diff / 60) });
  if (diff < 86400) return t("notification_bell.hours_ago", { count: Math.floor(diff / 3600) });
  return t("notification_bell.days_ago", { count: Math.floor(diff / 86400) });
}

const ENGINE_COLORS: Record<string, string> = {
  claude: "text-agent-claude bg-agent-claude/10",
  codex: "text-agent-codex bg-agent-codex/10",
  gemini: "text-agent-gemini bg-agent-gemini/10",
  opencode: "text-primary bg-primary/10",
};

function NotificationItem({ n, onNavigate }: { n: AppNotification; onNavigate: (convId: string) => void }) {
  const { t } = useTranslation("common");
  const typeDots = {
    completed: "bg-status-approved",
    error: "bg-status-rejected",
    info: "bg-muted-foreground/50",
  };

  return (
    <button
      onClick={() => n.conversationId && onNavigate(n.conversationId)}
      className={cn(
        "w-full text-left px-3 py-2 hover:bg-accent/50 transition-colors",
        !n.read && "bg-accent/20",
      )}
    >
      <div className="flex items-start gap-2">
        <span className={cn("w-1.5 h-1.5 rounded-full mt-1.5 shrink-0", typeDots[n.type])} />
        <div className="flex-1 min-w-0">
          {/* Row 1: title + engine badge + time */}
          <div className="flex items-center gap-1.5">
            <span className="text-[10px] font-medium text-foreground/80 truncate">{n.title}</span>
            {n.engine && (
              <span className={cn("text-[8px] font-semibold px-1 py-0 rounded shrink-0", ENGINE_COLORS[n.engine] ?? "text-muted-foreground bg-muted")}>
                {n.engine}
              </span>
            )}
            <span className="text-[8px] text-muted-foreground/30 ml-auto shrink-0">{timeAgo(n.timestamp, t)}</span>
          </div>
          {/* Row 2: conversation title + body */}
          <div className="flex items-center gap-1 mt-0.5">
            {n.conversationTitle && (
              <span className="text-[9px] text-muted-foreground/50 truncate max-w-[100px] shrink-0">
                {n.conversationTitle}
              </span>
            )}
            {n.conversationTitle && <span className="text-[8px] text-muted-foreground/25 shrink-0">·</span>}
            <p className="text-[9px] text-muted-foreground/50 truncate">{n.body}</p>
          </div>
          {/* Row 3: response preview */}
          {n.preview && (
            <p className="text-[9px] text-foreground/40 truncate mt-0.5 italic">{n.preview}</p>
          )}
        </div>
      </div>
    </button>
  );
}

export function NotificationBell() {
  const { t } = useTranslation("common");
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const notifications = useNotificationStore((s) => s.notifications);
  const unreadCount = useNotificationStore((s) => s.unreadCount);
  const markAllRead = useNotificationStore((s) => s.markAllRead);
  const clearAll = useNotificationStore((s) => s.clearAll);
  const soundEnabled = useNotificationStore((s) => s.soundEnabled);
  const toggleSound = useNotificationStore((s) => s.toggleSound);

  // Load persisted sound setting on mount
  useEffect(() => {
    useNotificationStore.getState().loadSoundSetting();
  }, []);

  const selectConversation = useChatStore((s) => s.selectConversation);
  const openThread = useChatStore((s) => s.openThread);

  // Close on outside click
  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [open]);

  const handleOpen = () => {
    setOpen(!open);
    if (!open) markAllRead();
  };

  const handleNavigate = (convId: string) => {
    setOpen(false);
    if (convId.startsWith("branch:")) {
      const branchId = convId.slice("branch:".length);
      openThread(branchId);
    } else {
      selectConversation(convId);
    }
  };

  return (
    <div ref={ref} className="relative">
      <button
        onClick={handleOpen}
        className="flex items-center justify-center w-7 h-7 rounded hover:bg-accent/40 transition-colors relative"
        title="Notifications"
      >
        <Bell className="w-3.5 h-3.5 text-prose-muted" />
        {unreadCount > 0 && (
          <span className="absolute top-0.5 right-0.5 min-w-[10px] h-2.5 flex items-center justify-center rounded-full bg-status-rejected text-[7px] font-bold text-white px-0.5 leading-none z-10">
            {unreadCount > 9 ? "9+" : unreadCount}
          </span>
        )}
      </button>

      {open && (
        <div className="absolute right-0 top-full mt-1 w-[320px] max-h-[400px] bg-card border border-border/40 rounded-lg shadow-2xl z-[100] overflow-hidden flex flex-col">
          {/* Header */}
          <div className="flex items-center justify-between px-3 py-2 border-b border-border/30 shrink-0">
            <span className="text-[11px] font-medium text-foreground/80">Notifications</span>
            <div className="flex items-center gap-2">
              <button
                onClick={toggleSound}
                title={soundEnabled ? t("notification_bell.sound_off") : t("notification_bell.sound_on")}
                className="text-muted-foreground/40 hover:text-foreground transition-colors"
              >
                {soundEnabled
                  ? <Volume2 className="w-3 h-3" />
                  : <VolumeX className="w-3 h-3" />}
              </button>
              {notifications.length > 0 && (
                <button onClick={clearAll} className="text-[9px] text-muted-foreground/40 hover:text-foreground transition-colors">
                  Clear all
                </button>
              )}
            </div>
          </div>

          {/* List */}
          <div className="flex-1 overflow-y-auto">
            {notifications.length === 0 ? (
              <div className="text-center py-8">
                <Bell className="w-4 h-4 text-muted-foreground/20 mx-auto mb-2" />
                <p className="text-[10px] text-muted-foreground/40">No notifications</p>
              </div>
            ) : (
              <div className="divide-y divide-border/20">
                {notifications.map((n) => (
                  <NotificationItem key={n.id} n={n} onNavigate={handleNavigate} />
                ))}
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
