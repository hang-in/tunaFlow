//! Crash report commands — Phase 4 Finding 4-3.
//!
//! Thin Tauri wrappers around `bootstrap::crash` so the Settings badge
//! can count recent reports and the JS side can forward uncaught errors
//! into the same sink as Rust panics.

use crate::bootstrap::crash::{list_crash_reports, record_js_error, CrashReportSummary};
use crate::errors::AppError;

#[tauri::command]
pub fn list_recent_crash_reports(limit: Option<usize>) -> Result<Vec<CrashReportSummary>, AppError> {
    Ok(list_crash_reports(limit.unwrap_or(10)))
}

#[tauri::command]
pub fn log_js_error(message: String, source: String) -> Result<(), AppError> {
    record_js_error(&message, &source)?;
    Ok(())
}
