/**
 * Client-side crash reporter — Phase 4 Finding 4-3.
 *
 * Forwards uncaught errors and unhandled promise rejections into the
 * same `~/.tunaflow/crash-reports/<DATE>.log` sink used by Rust panics,
 * via the `log_js_error` tauri command. Intentionally fire-and-forget —
 * a reporting failure should never mask the underlying error.
 */
import { invoke } from "@tauri-apps/api/core";

function forward(message: string, source: string) {
  invoke("log_js_error", { message, source }).catch(() => { /* swallow */ });
}

export function installCrashReporter() {
  window.addEventListener("error", (e) => {
    const msg = e.error?.stack ?? e.message ?? String(e);
    forward(msg, `window.onerror @ ${e.filename ?? "unknown"}:${e.lineno ?? "?"}`);
  });
  window.addEventListener("unhandledrejection", (e) => {
    const reason = e.reason;
    const msg = reason?.stack ?? reason?.message ?? String(reason);
    forward(msg, "unhandledrejection");
  });
}

export interface CrashReportSummary {
  file: string;
  size: number;
  modifiedMs: number;
}

export async function listRecentCrashReports(limit = 10): Promise<CrashReportSummary[]> {
  try {
    return await invoke<CrashReportSummary[]>("list_recent_crash_reports", { limit });
  } catch {
    return [];
  }
}
