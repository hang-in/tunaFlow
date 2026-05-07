//! Query expansion via Claude Haiku subprocess + 7-day DB cache.
//!
//! Ported from `seCall/crates/secall-core/src/search/query_expand.rs` to
//! tunaFlow's schema (query_cache table, Phase A). Semantics preserved:
//!
//! 1. Normalize the incoming query (trim + lowercase).
//! 2. If `TUNAFLOW_QUERY_EXPANSION` is not ON, return original (opt-in).
//! 3. Consult `query_cache` — if fresh (< 7 days), return `query + cached`.
//! 4. Otherwise call `claude -p <prompt> --model claude-haiku-4-5-20251001`.
//!    - Success: store into cache, return `query + expansion`.
//!    - Failure / missing binary: log, return original query (safe fallback).
//!
//! The expansion prompt asks for keywords, synonyms, tech terms, and EN↔KO
//! translations — exactly secall's working formulation. Result is whitespace
//! joined keywords with no explanatory prose.

use rusqlite::{params, Connection};

use crate::db::migrations::now_epoch_ms;
use crate::errors::AppError;
use crate::no_console::NoConsole;

const CACHE_TTL_MS: i64 = 7 * 24 * 60 * 60 * 1000; // 7 days
const HAIKU_MODEL: &str = "claude-haiku-4-5-20251001";

/// Is the expansion feature enabled? Default OFF (opt-in) — expansion involves
/// a subprocess call that adds latency on cache miss. Flip this via
/// `TUNAFLOW_QUERY_EXPANSION=on|1|true` once rollout is validated.
pub fn query_expansion_enabled() -> bool {
    match std::env::var("TUNAFLOW_QUERY_EXPANSION") {
        Ok(v) => matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "on" | "yes"),
        Err(_) => false,
    }
}

/// Canonical form of a query used as the cache key. Trim + lowercase, but we
/// intentionally do NOT strip punctuation / normalize whitespace further —
/// users phrase things differently and those variations can carry intent.
pub fn normalize_query(query: &str) -> String {
    query.trim().to_lowercase()
}

/// Expand a search query. Returns `"<original> <expansions>"` when the feature
/// is on and the call succeeds. Returns the original query unchanged on any
/// failure path.
///
/// `conn` is used for caching. Pass `None` to skip the cache (tests / one-off).
pub fn expand_query(query: &str, conn: Option<&Connection>) -> Result<String, AppError> {
    expand_query_inner(query, conn, query_expansion_enabled(), InvokeClaude::Real)
}

/// Strategy for the outbound Claude call — swappable in tests so we don't need
/// to manipulate PATH or env vars in parallel test threads.
///
/// `Empty` / `Stub` are constructed only from `#[cfg(test)]` call sites; the
/// production path uses `Real`. Variants stay on the public-ish enum (not
/// gated behind `cfg(test)`) so the type signature is identical across
/// test/release builds.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub(crate) enum InvokeClaude {
    /// Spawn `claude -p ...` as a subprocess (production path).
    Real,
    /// Return `"BYPASS"` — used by tests to exercise the bypass/empty branch.
    Empty,
    /// Return the given fixed expansion — used by tests to exercise the happy
    /// path without a live subprocess.
    Stub(&'static str),
}

/// Inner implementation with explicit feature flag + invocation strategy.
/// This form is testable without env-var mutation (safe under parallel tests).
pub(crate) fn expand_query_inner(
    query: &str,
    conn: Option<&Connection>,
    enabled: bool,
    invoker: InvokeClaude,
) -> Result<String, AppError> {
    if !enabled {
        return Ok(query.to_string());
    }
    if query.trim().is_empty() {
        return Ok(query.to_string());
    }

    let key = normalize_query(query);

    // 1. Cache hit?
    if let Some(conn) = conn {
        if let Some(cached) = read_cache_if_fresh(conn, &key) {
            return Ok(format!("{query} {cached}"));
        }
    }

    // 2. Call out to Claude (or swapped strategy in tests). On failure fall
    // back to the original query — expansion is optional.
    let invocation_result = match invoker {
        InvokeClaude::Real => invoke_claude_haiku(&key),
        InvokeClaude::Empty => Ok(String::new()),
        InvokeClaude::Stub(s) => Ok(s.to_string()),
    };
    let expansion = match invocation_result {
        Ok(s) if !s.trim().is_empty() => s,
        Ok(_) => return Ok(query.to_string()),
        Err(e) => {
            eprintln!("[query_expand] expansion skipped: {e}");
            return Ok(query.to_string());
        }
    };

    // 3. Cache the result for next time (best-effort; ignore write errors).
    if let Some(conn) = conn {
        if let Err(e) = write_cache(conn, &key, &expansion) {
            eprintln!("[query_expand] cache write failed (non-fatal): {e:?}");
        }
    }

    Ok(format!("{query} {expansion}"))
}

// ─── DB helpers ──────────────────────────────────────────────────────────────

fn read_cache_if_fresh(conn: &Connection, key: &str) -> Option<String> {
    let row: Option<(String, i64)> = conn
        .query_row(
            "SELECT expanded, cached_at FROM query_cache WHERE query = ?1",
            [key],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();
    let (expanded, cached_at) = row?;
    let age = now_epoch_ms() - cached_at;
    if age >= 0 && age < CACHE_TTL_MS {
        Some(expanded)
    } else {
        None
    }
}

fn write_cache(conn: &Connection, key: &str, expanded: &str) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO query_cache (query, expanded, cached_at) \
         VALUES (?1, ?2, ?3) \
         ON CONFLICT(query) DO UPDATE SET expanded = excluded.expanded, cached_at = excluded.cached_at",
        params![key, expanded, now_epoch_ms()],
    )?;
    Ok(())
}

// ─── Subprocess ──────────────────────────────────────────────────────────────

/// Call `claude -p <prompt> --model claude-haiku-4-5-20251001` and return
/// stdout. Stderr + non-zero exit are treated as expansion failure (caller
/// falls back to original query). `Command::output` returns an io error when
/// the binary isn't on PATH, which is the same failure path — so a separate
/// "is claude installed?" probe is unnecessary.
fn invoke_claude_haiku(query: &str) -> Result<String, AppError> {
    let prompt = build_expansion_prompt(query);
    let output = std::process::Command::new("claude")
        .no_console()
        .args(["-p", &prompt, "--model", HAIKU_MODEL])
        .output()
        .map_err(|e| AppError::Agent(format!("claude subprocess: {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::Agent(format!(
            "claude exit={:?} stderr={}",
            output.status.code(),
            stderr.trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// The prompt is intentionally kept in sync with secall's working formulation —
/// asks for keywords only, no explanation. Model output is inserted verbatim.
fn build_expansion_prompt(query: &str) -> String {
    format!(
        "다음 검색 쿼리를 확장해주세요. \
         원본 쿼리의 키워드, 동의어, 관련 기술 용어, 영어/한국어 변환을 포함하세요. \
         결과는 공백으로 구분된 키워드만 출력하세요. 설명 없이 키워드만.\n\n\
         쿼리: {query}"
    )
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn open_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE query_cache (
                query      TEXT PRIMARY KEY,
                expanded   TEXT NOT NULL,
                cached_at  INTEGER NOT NULL
             );",
        )
        .unwrap();
        conn
    }

    #[test]
    fn normalize_trims_and_lowercases() {
        assert_eq!(normalize_query("  Plan  "), "plan");
        assert_eq!(normalize_query("플랜"), "플랜");
        assert_eq!(normalize_query("MixedCase"), "mixedcase");
    }

    #[test]
    fn disabled_returns_original_untouched() {
        let conn = open_test_db();
        let out = expand_query_inner("hello world", Some(&conn), false, InvokeClaude::Stub("ignored")).unwrap();
        assert_eq!(out, "hello world");
    }

    #[test]
    fn empty_query_returns_original_even_when_enabled() {
        let conn = open_test_db();
        assert_eq!(
            expand_query_inner("", Some(&conn), true, InvokeClaude::Stub("x")).unwrap(),
            ""
        );
        assert_eq!(
            expand_query_inner("   ", Some(&conn), true, InvokeClaude::Stub("x")).unwrap(),
            "   "
        );
    }

    #[test]
    fn cache_hit_short_circuits_invoker() {
        let conn = open_test_db();
        // Pre-seed fresh cache entry. If the cache is consulted, the stub's
        // "SHOULD NOT BE CALLED" text must NOT appear in the output.
        let key = normalize_query("플랜");
        conn.execute(
            "INSERT INTO query_cache (query, expanded, cached_at) VALUES (?1, ?2, ?3)",
            params![key, "plan 계획 roadmap", now_epoch_ms()],
        )
        .unwrap();
        let out = expand_query_inner(
            "플랜",
            Some(&conn),
            true,
            InvokeClaude::Stub("SHOULD NOT BE CALLED"),
        )
        .unwrap();
        assert_eq!(out, "플랜 plan 계획 roadmap");
        assert!(!out.contains("SHOULD NOT BE CALLED"));
    }

    #[test]
    fn cache_stale_is_ignored_and_refreshed() {
        let conn = open_test_db();
        let key = normalize_query("플랜");
        let stale = now_epoch_ms() - (CACHE_TTL_MS + 1);
        conn.execute(
            "INSERT INTO query_cache (query, expanded, cached_at) VALUES (?1, ?2, ?3)",
            params![key, "stale expansion", stale],
        )
        .unwrap();
        let out = expand_query_inner(
            "플랜",
            Some(&conn),
            true,
            InvokeClaude::Stub("fresh expansion"),
        )
        .unwrap();
        assert!(out.contains("fresh expansion"));
        assert!(!out.contains("stale expansion"), "stale cache must not be used");
    }

    #[test]
    fn successful_expansion_is_cached_for_next_call() {
        let conn = open_test_db();
        let _ = expand_query_inner(
            "플랜",
            Some(&conn),
            true,
            InvokeClaude::Stub("plan 계획"),
        )
        .unwrap();
        // Second call with Empty stub must still return the expansion — via cache.
        let out2 = expand_query_inner("플랜", Some(&conn), true, InvokeClaude::Empty).unwrap();
        assert_eq!(out2, "플랜 plan 계획");
    }

    #[test]
    fn empty_expansion_falls_back_to_original() {
        let conn = open_test_db();
        let out = expand_query_inner("플랜", Some(&conn), true, InvokeClaude::Empty).unwrap();
        assert_eq!(out, "플랜");
        // And the cache must NOT be polluted with an empty entry.
        let cached = read_cache_if_fresh(&conn, &normalize_query("플랜"));
        assert!(cached.is_none(), "empty expansion must not cache");
    }

    #[test]
    fn no_cache_still_works_with_enabled() {
        // When conn=None the function can't cache, but still returns expansion.
        let out = expand_query_inner(
            "plan",
            None,
            true,
            InvokeClaude::Stub("plan 계획 roadmap"),
        )
        .unwrap();
        assert_eq!(out, "plan plan 계획 roadmap");
    }

    #[test]
    fn write_cache_roundtrip() {
        let conn = open_test_db();
        write_cache(&conn, "foo", "bar baz").unwrap();
        let got = read_cache_if_fresh(&conn, "foo").unwrap();
        assert_eq!(got, "bar baz");
    }

    #[test]
    fn write_cache_upserts_on_conflict() {
        let conn = open_test_db();
        write_cache(&conn, "foo", "old").unwrap();
        write_cache(&conn, "foo", "new").unwrap();
        let got = read_cache_if_fresh(&conn, "foo").unwrap();
        assert_eq!(got, "new");
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM query_cache WHERE query = 'foo'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn read_cache_returns_none_for_missing_key() {
        let conn = open_test_db();
        assert!(read_cache_if_fresh(&conn, "missing").is_none());
    }

    #[test]
    fn prompt_contains_query_verbatim_and_keywords() {
        let prompt = build_expansion_prompt("플랜");
        assert!(prompt.contains("쿼리: 플랜"));
        assert!(prompt.contains("동의어"));
        assert!(prompt.contains("영어/한국어"));
    }
}
