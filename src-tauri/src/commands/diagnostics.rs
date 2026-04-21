//! Runtime diagnostics — rate limit, process health.
//!
//! Rate limit data: 3-source fallback (duckbar → OMC plugin → abtop).
//! All sources are file-based caches written by external tools.

use serde::Serialize;
use crate::errors::AppError;

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitInfo {
    pub five_hour_pct: Option<f64>,
    pub five_hour_resets_at: Option<String>,
    pub weekly_pct: Option<f64>,
    pub weekly_resets_at: Option<String>,
    pub extra_usage_enabled: Option<bool>,
    pub extra_usage_pct: Option<f64>,
    pub source: String,
    pub stale: bool,
}

/// Read rate limit info from available cache files.
/// 3-source fallback: duckbar → OMC plugin → abtop.
#[tauri::command]
pub fn get_rate_limit_info() -> Result<Option<RateLimitInfo>, AppError> {
    let claude_dir = match dirs::home_dir() {
        Some(h) => h.join(".claude"),
        None => return Ok(None),
    };

    // Source 1: duckbar cache
    let duckbar_path = claude_dir.join(".duckbar-ratelimits-cache.json");
    if let Some(info) = read_duckbar_cache(&duckbar_path) {
        return Ok(Some(info));
    }

    // Source 2: oh-my-claudecode plugin cache
    let omc_path = claude_dir.join("plugins/oh-my-claudecode/.usage-cache.json");
    if let Some(info) = read_omc_cache(&omc_path) {
        return Ok(Some(info));
    }

    // Source 3: abtop StatusLine hook
    let abtop_path = claude_dir.join("abtop-rate-limits.json");
    if let Some(info) = read_abtop_cache(&abtop_path) {
        return Ok(Some(info));
    }

    Ok(None)
}

fn is_stale(cached_at: &str, max_age_secs: u64) -> bool {
    // Parse ISO 8601 timestamp and check age
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Try parsing as ISO 8601
    if let Ok(dt) = chrono_parse_approx(cached_at) {
        return now.saturating_sub(dt) > max_age_secs;
    }
    true // Can't parse = treat as stale
}

fn chrono_parse_approx(s: &str) -> Result<u64, ()> {
    // Simple ISO 8601 parser: "2026-04-16T07:52:37Z"
    // Try dateutil-style parse via string matching
    if s.len() >= 19 {
        let parts: Vec<&str> = s.split(&['T', '-', ':', 'Z', '+'][..]).collect();
        if parts.len() >= 6 {
            // Rough epoch calculation (not accounting for leap seconds etc)
            let y: u64 = parts[0].parse().map_err(|_| ())?;
            let m: u64 = parts[1].parse().map_err(|_| ())?;
            let d: u64 = parts[2].parse().map_err(|_| ())?;
            let h: u64 = parts[3].parse().map_err(|_| ())?;
            let min: u64 = parts[4].parse().map_err(|_| ())?;
            let sec: u64 = parts[5].parse().map_err(|_| ())?;
            // Days from epoch (approximate)
            let days = (y - 1970) * 365 + (y - 1969) / 4 + month_days(m) + d - 1;
            return Ok(days * 86400 + h * 3600 + min * 60 + sec);
        }
    }
    // Try parsing as unix timestamp
    s.parse::<u64>().map_err(|_| ())
}

fn month_days(m: u64) -> u64 {
    match m {
        1 => 0, 2 => 31, 3 => 59, 4 => 90, 5 => 120, 6 => 151,
        7 => 181, 8 => 212, 9 => 243, 10 => 273, 11 => 304, 12 => 334,
        _ => 0,
    }
}

fn read_duckbar_cache(path: &std::path::Path) -> Option<RateLimitInfo> {
    let text = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;

    let cached_at = v.get("_cachedAt").and_then(|v| v.as_str()).unwrap_or("");
    let stale = is_stale(cached_at, 600); // 10 min

    Some(RateLimitInfo {
        five_hour_pct: v.get("fiveHourPercent").and_then(|v| v.as_f64()),
        five_hour_resets_at: v.get("fiveHourResetsAt").and_then(|v| v.as_str()).map(String::from),
        weekly_pct: v.get("weeklyPercent").and_then(|v| v.as_f64()),
        weekly_resets_at: v.get("weeklyResetsAt").and_then(|v| v.as_str()).map(String::from),
        extra_usage_enabled: v.get("extraUsageEnabled").and_then(|v| v.as_bool()),
        extra_usage_pct: {
            let used = v.get("extraUsageUsed").and_then(|v| v.as_f64());
            let limit = v.get("extraUsageLimit").and_then(|v| v.as_f64());
            match (used, limit) {
                (Some(u), Some(l)) if l > 0.0 => Some((u / l) * 100.0),
                _ => None,
            }
        },
        source: "duckbar".into(),
        stale,
    })
}

fn read_omc_cache(path: &std::path::Path) -> Option<RateLimitInfo> {
    let text = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;

    Some(RateLimitInfo {
        five_hour_pct: v.get("fiveHourPct").and_then(|v| v.as_f64()),
        five_hour_resets_at: v.get("fiveHourResetsAt").and_then(|v| v.as_str()).map(String::from),
        weekly_pct: v.get("weeklyPct").and_then(|v| v.as_f64()),
        weekly_resets_at: v.get("weeklyResetsAt").and_then(|v| v.as_str()).map(String::from),
        extra_usage_enabled: None,
        extra_usage_pct: None,
        source: "omc".into(),
        stale: false, // OMC has its own freshness management
    })
}

fn read_abtop_cache(path: &std::path::Path) -> Option<RateLimitInfo> {
    let text = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;

    let updated = v.get("updated_at").and_then(|v| v.as_u64()).unwrap_or(0);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let stale = now.saturating_sub(updated) > 600;

    let five_hour = v.get("five_hour");
    let seven_day = v.get("seven_day");

    Some(RateLimitInfo {
        five_hour_pct: five_hour.and_then(|w| w.get("used_percentage")).and_then(|v| v.as_f64()),
        five_hour_resets_at: five_hour.and_then(|w| w.get("resets_at")).and_then(|v| v.as_str()).map(String::from),
        weekly_pct: seven_day.and_then(|w| w.get("used_percentage")).and_then(|v| v.as_f64()),
        weekly_resets_at: seven_day.and_then(|w| w.get("resets_at")).and_then(|v| v.as_str()).map(String::from),
        extra_usage_enabled: None,
        extra_usage_pct: None,
        source: "abtop".into(),
        stale,
    })
}
