/**
 * Lightweight platform detection cached at module load.
 *
 * Avoids pulling in `@tauri-apps/plugin-os` (extra plugin = bundle bloat) by
 * sniffing `navigator.userAgent` once. The Tauri webview reliably surfaces a
 * "Mac" / "Macintosh" token on macOS. Any false positive is harmless — the
 * macOS-only Tauri command (`notification_send_native`) returns Err on other
 * OS, so the worst case is a single extra invoke that surfaces in the console.
 *
 * Used by `notificationStore.ts` to route macOS to the native ObjC bridge
 * (`docs/plans/nativeNotificationPlan_2026-04-29.md` Path B) while keeping
 * Windows/Linux on `tauri-plugin-notification` (zero behavior change).
 */

function detect(): "macos" | "other" {
  if (typeof navigator === "undefined") return "other";
  const ua = navigator.userAgent || "";
  return /Mac|Macintosh|MacIntel/i.test(ua) ? "macos" : "other";
}

const cached = detect();

export function isMacOS(): boolean {
  return cached === "macos";
}
