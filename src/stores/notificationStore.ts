import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { getSetting, setSetting } from "@/lib/appStore";
import { isMacOS } from "@/lib/platform";

export type NotificationType = "completed" | "error" | "info";

export interface AppNotification {
  id: string;
  type: NotificationType;
  title: string;
  body: string;
  engine?: string;
  conversationTitle?: string;
  preview?: string;
  conversationId?: string;
  timestamp: number;
  read: boolean;
}

export interface NotifyMeta {
  engine?: string;
  conversationTitle?: string;
  preview?: string;
}

/** Mirrors `src-tauri/src/notification.rs::NotificationAuthStatus`. */
export type NotificationAuthStatus = "notDetermined" | "denied" | "authorized";

interface NotificationState {
  notifications: AppNotification[];
  unreadCount: number;
  soundEnabled: boolean;
  /** macOS native bridge 의 권한 상태 캐시. macOS 외 OS 에서는 항상 'authorized'
   *  로 두고 (기존 plugin path 가 OS 권한을 자체 처리). */
  permissionStatus: NotificationAuthStatus;
  /** denied 안내 토스트는 세션당 1회만 — 사용자가 매 알림마다 spam 받지 않도록. */
  deniedToastShownThisSession: boolean;
  addNotification: (n: Omit<AppNotification, "id" | "timestamp" | "read">) => void;
  markAllRead: () => void;
  clearAll: () => void;
  toggleSound: () => void;
  loadSoundSetting: () => void;
  setPermissionStatus: (s: NotificationAuthStatus) => void;
  markDeniedToastShown: () => void;
  /** macOS Settings 섹션의 "지금 권한 요청" 버튼이 호출. Returns final status. */
  requestPermissionFromUI: () => Promise<NotificationAuthStatus>;
  /** Settings 섹션이 마운트될 때 현재 status 를 가져온다. */
  refreshPermissionStatus: () => Promise<NotificationAuthStatus>;
}

const MAX_NOTIFICATIONS = 50;

export const useNotificationStore = create<NotificationState>((set, get) => ({
  notifications: [],
  unreadCount: 0,
  soundEnabled: true,
  // macOS 외에서는 권한 검사 path 자체가 안 돌아가므로 'authorized' 로 시작.
  permissionStatus: isMacOS() ? "notDetermined" : "authorized",
  deniedToastShownThisSession: false,

  addNotification: (n) => {
    const notification: AppNotification = {
      ...n,
      id: crypto.randomUUID(),
      timestamp: Date.now(),
      read: false,
    };
    set((state) => {
      const next = [notification, ...state.notifications].slice(0, MAX_NOTIFICATIONS);
      return { notifications: next, unreadCount: state.unreadCount + 1 };
    });
  },

  markAllRead: () =>
    set((state) => ({
      notifications: state.notifications.map((n) => ({ ...n, read: true })),
      unreadCount: 0,
    })),

  clearAll: () => set({ notifications: [], unreadCount: 0 }),

  toggleSound: () => {
    const next = !get().soundEnabled;
    set({ soundEnabled: next });
    setSetting("notificationSound", next);
  },

  loadSoundSetting: () => {
    getSetting("notificationSound", true).then((v) => set({ soundEnabled: v }));
  },

  setPermissionStatus: (s) => set({ permissionStatus: s }),

  markDeniedToastShown: () => set({ deniedToastShownThisSession: true }),

  requestPermissionFromUI: async () => {
    if (!isMacOS()) return get().permissionStatus;
    try {
      const granted = await invoke<boolean>("notification_request_permission");
      const status: NotificationAuthStatus = granted ? "authorized" : "denied";
      set({ permissionStatus: status });
      return status;
    } catch (e) {
      console.error("[notify] permission request failed", e);
      return get().permissionStatus;
    }
  },

  refreshPermissionStatus: async () => {
    if (!isMacOS()) return "authorized";
    try {
      const status = await invoke<NotificationAuthStatus>("notification_get_status");
      set({ permissionStatus: status });
      return status;
    } catch (e) {
      console.error("[notify] status fetch failed", e);
      return get().permissionStatus;
    }
  },
}));

// ─── Notification sound via Web Audio API (no external file needed) ──────────

let audioCtx: AudioContext | null = null;

function playNotificationSound(type: NotificationType) {
  try {
    if (!audioCtx) audioCtx = new AudioContext();
    const ctx = audioCtx;

    const osc = ctx.createOscillator();
    const gain = ctx.createGain();
    osc.connect(gain);
    gain.connect(ctx.destination);

    if (type === "error") {
      // Low double beep for errors
      osc.frequency.setValueAtTime(400, ctx.currentTime);
      osc.frequency.setValueAtTime(300, ctx.currentTime + 0.15);
      gain.gain.setValueAtTime(0.15, ctx.currentTime);
      gain.gain.exponentialRampToValueAtTime(0.01, ctx.currentTime + 0.3);
      osc.start(ctx.currentTime);
      osc.stop(ctx.currentTime + 0.3);
    } else {
      // Pleasant chime for completed/info
      osc.frequency.setValueAtTime(880, ctx.currentTime);
      osc.frequency.setValueAtTime(1100, ctx.currentTime + 0.08);
      gain.gain.setValueAtTime(0.12, ctx.currentTime);
      gain.gain.exponentialRampToValueAtTime(0.01, ctx.currentTime + 0.25);
      osc.start(ctx.currentTime);
      osc.stop(ctx.currentTime + 0.25);
    }
  } catch (e) {
    console.debug("[notify-sound]", e);
  }
}

// ─── Central notify function ─────────────────────────────────────────────────

/**
 * 옵션 D 권한 UX: 첫 알림 시 native dialog 자동 prompt.
 *
 *  - `notDetermined` → request_permission 호출 → 응답에 따라 발송 또는 silent skip
 *  - `denied` → silent skip + 세션당 1회 토스트
 *  - `authorized` → 즉시 발송
 *
 * macOS 외 OS 는 기존 `tauri-plugin-notification` path 그대로.
 */
async function ensureMacOSPermissionAndSend(title: string, body: string): Promise<void> {
  const store = useNotificationStore.getState();
  let status = store.permissionStatus;

  if (status === "notDetermined") {
    // refresh 한 번 먼저 — 다른 세션에서 결정됐을 수도 있음.
    status = await store.refreshPermissionStatus();
  }
  if (status === "notDetermined") {
    // 사용자가 처음 보는 알림 → native dialog 띄움.
    status = await store.requestPermissionFromUI();
  }

  if (status === "authorized") {
    await invoke("notification_send_native", { title, body });
    return;
  }

  if (status === "denied") {
    // 세션당 1회 토스트 — sonner 동적 import 로 store 가 UI lib 와 결합되지 않게.
    const after = useNotificationStore.getState();
    if (!after.deniedToastShownThisSession) {
      after.markDeniedToastShown();
      try {
        const { toast } = await import("sonner");
        const { default: i18n } = await import("@/locales");
        toast.error(i18n.t("notifications.toast.denied", { ns: "settings" }));
      } catch (e) {
        // i18n 또는 sonner 초기화 전이면 console fallback.
        console.warn(
          "[notify] OS notification denied — enable in System Settings → Notifications.",
          e,
        );
      }
    } else {
      console.debug("[notify] OS notification denied (toast already shown).");
    }
    return;
  }

  // notDetermined 가 다시 떨어지는 비상 case → silent skip.
  console.debug("[notify] permission still notDetermined; skipping send.");
}

/** Send OS notification (if app not focused) + add to history + play sound. */
export async function notify(
  type: NotificationType,
  title: string,
  body: string,
  conversationId?: string,
  meta?: NotifyMeta,
): Promise<void> {
  // Add to in-app history
  useNotificationStore.getState().addNotification({
    type, title, body, conversationId,
    engine: meta?.engine,
    conversationTitle: meta?.conversationTitle,
    preview: meta?.preview,
  });

  // Play sound if enabled
  if (useNotificationStore.getState().soundEnabled) {
    playNotificationSound(type);
  }

  // OS notification only when app is not focused
  if (document.hidden) {
    try {
      if (isMacOS()) {
        // Native UNUserNotificationCenter bridge — no osascript / Script Editor.
        await ensureMacOSPermissionAndSend(title, body);
      } else {
        const { sendNotification, isPermissionGranted } = await import("@tauri-apps/plugin-notification");
        const granted = await isPermissionGranted();
        if (granted) {
          sendNotification({ title, body });
        }
      }
    } catch (e) {
      console.debug("[notify]", e);
    }
  }
}
