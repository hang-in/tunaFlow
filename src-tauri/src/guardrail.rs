//! Runtime guardrails for ContextPack size, section truncation, and execution logging.
//! All values are character counts (not tokens). Token estimation: ~4 chars ≈ 1 token.

// ─── Section limits (characters) ─────────────────────────────────────────────

/// Maximum total system prompt size after all sections are assembled.
pub const MAX_TOTAL_PROMPT: usize = 60_000;

/// Per-section character limits for ContextPack sections.
/// Priority layers (structured) get full budget; auxiliary layers get smaller caps.
pub const MAX_SKILLS_SECTION: usize = 8_000;
pub const MAX_RAWQ_SECTION: usize = 4_000;
pub const MAX_CROSS_SESSION_SECTION: usize = 4_000; // tuned down from 6k — often repetitive
pub const MAX_CONTEXT_SECTION: usize = 6_000;       // tuned down from 8k — recent window is already compact
pub const MAX_PLAN_SECTION: usize = 2_000;
pub const MAX_FINDINGS_SECTION: usize = 3_000;
pub const MAX_ARTIFACTS_SECTION: usize = 2_000;
/// Dedicated caps for memory layers (don't reuse MAX_CONTEXT_SECTION)
pub const MAX_RETRIEVAL_SECTION: usize = 4_000;      // past conversation chunks — focused, not large
pub const MAX_COMPRESSED_MEMORY_SECTION: usize = 5_000; // topic-based summaries — detailed enough to preserve decisions

// ─── Execution defaults ──────────────────────────────────────────────────────

/// Default subprocess timeout in seconds (applied at the OS level via wait).
/// Currently advisory — actual enforcement depends on the CLI tool's own timeout.
#[allow(dead_code)]
pub const DEFAULT_TIMEOUT_SECS: u64 = 120;

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Truncate a section string to `max` characters (char-boundary safe).
/// Returns the original if within limit.
pub fn truncate_section(section: Option<String>, max: usize) -> Option<String> {
    section.map(|s| {
        if s.len() <= max {
            s
        } else {
            let end = s
                .char_indices()
                .map(|(i, _)| i)
                .take_while(|&i| i <= max)
                .last()
                .unwrap_or(0);
            format!("{}…[truncated]", &s[..end])
        }
    })
}

/// Enforce total character limit on the assembled system prompt.
/// Truncates from the end with a `[system prompt truncated]` marker.
pub fn enforce_total_limit(prompt: Option<String>, max: usize) -> Option<String> {
    prompt.map(|s| {
        if s.len() <= max {
            s
        } else {
            let marker = "\n\n…[system prompt truncated]";
            let budget = max.saturating_sub(marker.len());
            let end = s
                .char_indices()
                .map(|(i, _)| i)
                .take_while(|&i| i <= budget)
                .last()
                .unwrap_or(0);
            format!("{}{}", &s[..end], marker)
        }
    })
}

// ─── Dynamic budget allocation ─────────────────────────────────────────────

/// Section budget request for dynamic allocation
pub struct SectionBudget {
    pub name: &'static str,
    /// Actual content length (0 if section is empty)
    pub content_len: usize,
    /// Relative importance weight (higher = more budget)
    pub weight: f32,
    /// Minimum guaranteed characters (even under pressure)
    pub min_chars: usize,
    /// Maximum cap (prevent one section from dominating)
    pub max_chars: usize,
}

/// Allocate budget dynamically based on actual content sizes.
///
/// Empty sections return 0 budget (released to others).
/// Non-empty sections get min_chars guaranteed + proportional share of remainder.
pub fn allocate_budgets(total: usize, sections: &[SectionBudget]) -> Vec<(&'static str, usize)> {
    // Phase 1: identify non-empty sections, allocate minimums
    let active: Vec<(usize, &SectionBudget)> = sections.iter()
        .enumerate()
        .filter(|(_, s)| s.content_len > 0)
        .collect();

    let total_min: usize = active.iter().map(|(_, s)| s.min_chars).sum();
    let remaining = total.saturating_sub(total_min);

    // Phase 2: distribute remaining by weight (proportional)
    let total_weight: f32 = active.iter().map(|(_, s)| s.weight).sum();

    let mut result: Vec<(&'static str, usize)> = sections.iter()
        .map(|s| (s.name, 0usize))
        .collect();

    if total_weight <= 0.0 {
        return result;
    }

    for (idx, section) in &active {
        let base = section.min_chars;
        let share = if total_weight > 0.0 {
            ((remaining as f32) * section.weight / total_weight) as usize
        } else { 0 };
        let allocated = (base + share).min(section.max_chars).min(section.content_len + 200); // +200 for headers
        result[*idx].1 = allocated;
    }

    result
}

/// Standard fallback error message for agent failures.
pub fn fallback_error(engine: &str, err: &crate::errors::AppError) -> String {
    format!(
        "[{} error] {}",
        engine,
        match err {
            crate::errors::AppError::Agent(msg) => msg.clone(),
            other => format!("{}", other),
        }
    )
}

/// Log an agent execution result to stderr (visible in `tauri dev` console).
pub fn log_run(engine: &str, model: Option<&str>, duration_ms: u128, prompt_len: usize, result_ok: bool) {
    let status = if result_ok { "ok" } else { "err" };
    let est_tokens = prompt_len / 4;
    eprintln!(
        "[guardrail] engine={} model={} status={} duration={}ms prompt_chars={} est_tokens={}",
        engine,
        model.unwrap_or("-"),
        status,
        duration_ms,
        prompt_len,
        est_tokens,
    );
}
