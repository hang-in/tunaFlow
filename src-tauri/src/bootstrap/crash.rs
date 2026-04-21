//! Crash reporting — Phase 4 Finding 4-3.
//!
//! Two surfaces:
//!
//!   1. `install_panic_hook()` chains on top of the existing hook and
//!      appends one line per panic to `~/.tunaflow/crash-reports/<DATE>.log`.
//!      The pre-existing hook still runs, so backtraces still land in
//!      stderr / the terminal.
//!   2. `list_crash_reports()` returns the newest N reports so the UI
//!      can surface a badge in Settings.
//!
//! Intentionally minimal — no network, no serialization of sensitive
//! state. Beta users forwarding a bug report attach the file manually.

use serde::Serialize;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Directory that holds one file per day: `YYYY-MM-DD.log`.
fn crash_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|p| p.join(".tunaflow").join("crash-reports"))
}

/// Chain our panic logger on top of whatever is currently installed
/// (usually the default "print + abort on unwind" hook).
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if let Err(e) = write_panic(info) {
            eprintln!("[crash] failed to persist panic report: {e}");
        }
        previous(info);
    }));
}

fn write_panic(info: &std::panic::PanicHookInfo<'_>) -> std::io::Result<()> {
    let Some(dir) = crash_dir() else {
        return Ok(()); // no home dir — nothing to do
    };
    fs::create_dir_all(&dir)?;

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);

    // "YYYY-MM-DD" filename. chrono is already a transitive dep; using it
    // here avoids reinventing date formatting.
    let date = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(now_ms)
        .map(|dt| dt.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "unknown-date".to_string());

    let path = dir.join(format!("{date}.log"));
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;

    let location = info
        .location()
        .map(|l| format!("{}:{}", l.file(), l.line()))
        .unwrap_or_else(|| "<unknown>".to_string());

    let message = if let Some(s) = info.payload().downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = info.payload().downcast_ref::<String>() {
        s.clone()
    } else {
        "<non-string payload>".to_string()
    };

    writeln!(
        file,
        "--- {}Z panic ---\nlocation: {}\nmessage: {}\n",
        chrono::DateTime::<chrono::Utc>::from_timestamp_millis(now_ms)
            .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S").to_string())
            .unwrap_or_default(),
        location,
        message,
    )?;
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct CrashReportSummary {
    pub file: String,
    pub size: u64,
    pub modified_ms: i64,
}

/// Return up to `limit` most recent crash report files.
pub fn list_crash_reports(limit: usize) -> Vec<CrashReportSummary> {
    let Some(dir) = crash_dir() else {
        return vec![];
    };
    let Ok(entries) = fs::read_dir(&dir) else {
        return vec![];
    };

    let mut out: Vec<CrashReportSummary> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            if !meta.is_file() {
                return None;
            }
            let modified_ms = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);
            Some(CrashReportSummary {
                file: e.path().to_string_lossy().into_owned(),
                size: meta.len(),
                modified_ms,
            })
        })
        .collect();

    out.sort_by(|a, b| b.modified_ms.cmp(&a.modified_ms));
    out.truncate(limit);
    out
}

/// JS-side hook (`window.onerror` / `unhandledrejection`) funnels into
/// this command. We reuse the same directory so the Settings badge only
/// needs to look in one place.
pub fn record_js_error(message: &str, source: &str) -> std::io::Result<()> {
    let Some(dir) = crash_dir() else {
        return Ok(());
    };
    fs::create_dir_all(&dir)?;
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let date = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(now_ms)
        .map(|dt| dt.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "unknown-date".to_string());
    let path = dir.join(format!("{date}.log"));
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(
        file,
        "--- {}Z js-error ({}) ---\n{}\n",
        chrono::DateTime::<chrono::Utc>::from_timestamp_millis(now_ms)
            .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S").to_string())
            .unwrap_or_default(),
        source,
        message,
    )?;
    Ok(())
}
