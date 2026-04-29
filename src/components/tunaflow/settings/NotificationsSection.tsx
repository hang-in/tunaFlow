import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { invoke } from "@tauri-apps/api/core";
import { useNotificationStore } from "@/stores/notificationStore";
import { isMacOS } from "@/lib/platform";

/**
 * Notifications settings — macOS native bridge 의 권한 상태 표시 + 수동 prompt.
 *
 * 옵션 D 권한 UX (`docs/plans/nativeNotificationPlan_2026-04-29.md`):
 * - 첫 알림 시 자동 prompt (notify() 안에서) + Settings 토글 병행
 * - denied 시 macOS 시스템 설정 deep link 안내
 *
 * macOS 외 OS 에서는 hint message 만 보이고 컨트롤은 비활성화 — 기존
 * tauri-plugin-notification path 가 OS 권한을 자체 처리.
 */
export function NotificationsSection() {
  const { t } = useTranslation("settings");
  const permissionStatus = useNotificationStore((s) => s.permissionStatus);
  const refreshPermissionStatus = useNotificationStore((s) => s.refreshPermissionStatus);
  const requestPermissionFromUI = useNotificationStore((s) => s.requestPermissionFromUI);

  const macOS = isMacOS();
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!macOS) return;
    void refreshPermissionStatus();
  }, [macOS, refreshPermissionStatus]);

  const onRequest = async () => {
    setBusy(true);
    try {
      await requestPermissionFromUI();
    } finally {
      setBusy(false);
    }
  };

  const onOpenSystemSettings = async () => {
    try {
      const { openUrl } = await import("@tauri-apps/plugin-opener");
      await openUrl("x-apple.systempreferences:com.apple.preference.notifications");
    } catch (e) {
      console.error("[notify] failed to open system settings", e);
    }
  };

  const onSendTest = async () => {
    setBusy(true);
    try {
      if (macOS) {
        await invoke("notification_send_native", {
          title: "tunaFlow",
          body: t("notifications.toast.test_sent"),
        });
      } else {
        const { sendNotification, isPermissionGranted, requestPermission } = await import(
          "@tauri-apps/plugin-notification"
        );
        let granted = await isPermissionGranted();
        if (!granted) {
          granted = (await requestPermission()) === "granted";
        }
        if (granted) {
          sendNotification({ title: "tunaFlow", body: t("notifications.toast.test_sent") });
        }
      }
      toast.success(t("notifications.toast.test_sent"));
    } catch (e) {
      toast.error(t("notifications.toast.test_failed", { error: String(e) }));
    } finally {
      setBusy(false);
    }
  };

  const statusLabel =
    permissionStatus === "authorized"
      ? t("notifications.status.authorized")
      : permissionStatus === "denied"
        ? t("notifications.status.denied")
        : t("notifications.status.notDetermined");

  const statusColor =
    permissionStatus === "authorized"
      ? "text-emerald-500"
      : permissionStatus === "denied"
        ? "text-red-500"
        : "text-amber-500";

  return (
    <div className="space-y-4">
      <div>
        <h3 className="text-xs font-semibold text-muted-foreground uppercase tracking-wider">
          {t("notifications.heading")}
        </h3>
        <p className="text-[12px] text-muted-foreground mt-1">{t("notifications.description")}</p>
      </div>

      {/* Status row */}
      <div className="flex items-center justify-between rounded-md border border-border/40 px-3 py-2">
        <span className="text-xs text-muted-foreground">{t("notifications.status.label")}</span>
        <span className={`text-xs font-medium ${statusColor}`}>{statusLabel}</span>
      </div>

      {/* Actions */}
      <div className="flex flex-wrap gap-2">
        <button
          type="button"
          onClick={onRequest}
          disabled={!macOS || busy || permissionStatus === "authorized"}
          className="text-xs px-3 py-1.5 rounded-md border border-border/40 hover:bg-accent disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
          {t("notifications.actions.request")}
        </button>
        <button
          type="button"
          onClick={onSendTest}
          disabled={busy}
          className="text-xs px-3 py-1.5 rounded-md border border-border/40 hover:bg-accent disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
          {t("notifications.actions.test")}
        </button>
        {macOS && permissionStatus === "denied" && (
          <button
            type="button"
            onClick={onOpenSystemSettings}
            disabled={busy}
            className="text-xs px-3 py-1.5 rounded-md border border-border/40 hover:bg-accent disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            {t("notifications.actions.open_system_settings")}
          </button>
        )}
      </div>

      {permissionStatus === "denied" && (
        <p className="text-[11px] text-muted-foreground">{t("notifications.denied_help")}</p>
      )}

      {!macOS && (
        <p className="text-[11px] text-muted-foreground/70">
          {t("notifications.status.macos_only_hint")}
        </p>
      )}
    </div>
  );
}
