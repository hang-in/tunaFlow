use rusqlite::params;

use crate::agents::{loader, rawq};

/// Controls how much context is assembled into the system prompt.
///
/// | Mode     | Includes                                    | Use case                     |
/// |----------|---------------------------------------------|------------------------------|
/// | Lite     | project path + base prompt + context summary | 일반 대화, 단순 질문          |
/// | Standard | Lite + plan + findings + artifacts           | follow-up, branch, plan 작업 |
/// | Full     | Standard + rawq + cross-session + skills     | 코드 분석, 전체 검토          |
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum ContextMode {
    Lite,
    Standard,
    Full,
}

/// Maximum number of rawq code search results.
const RAWQ_MAX_RESULTS: usize = 5;
/// Maximum number of prior messages for non-Claude lite context prefix.
pub const LITE_CONTEXT_MESSAGES_LIMIT: i64 = 4;
/// Maximum total characters for the lite context prefix.
const LITE_CONTEXT_MAX_CHARS: usize = 4000;

/// Truncate a string to `max` bytes (character boundary safe).
fn truncate_str(s: &str, max: usize) -> String {
    if s.len() > max {
        let end = s
            .char_indices()
            .map(|(i, _)| i)
            .take_while(|&i| i <= max)
            .last()
            .unwrap_or(0);
        format!("{}…", &s[..end])
    } else {
        s.to_string()
    }
}

fn format_section(header: &str, rows: &[(String, String)], max_chars: usize) -> String {
    let mut out = format!("## {}\n", header);
    for (role, content) in rows {
        out.push_str(&format!("\n[{}] {}\n", role, truncate_str(content, max_chars)));
    }
    out
}

fn format_section_with_authors(
    header: &str,
    rows: &[(String, String, Option<String>, Option<String>)],
    max_chars: usize,
) -> String {
    let mut out = format!("## {}\n", header);
    for (role, content, engine, persona) in rows {
        let author_tag = match (role.as_str(), persona, engine) {
            ("assistant", Some(p), Some(e)) if !p.is_empty() => format!("assistant:{} ({})", p, e),
            ("assistant", None, Some(e)) if !e.is_empty() => format!("assistant ({})", e),
            ("assistant", Some(p), _) if !p.is_empty() => format!("assistant:{}", p),
            _ => role.clone(),
        };
        // Apply markdown lightening to reduce token waste in long assistant messages
        let lightened = if role == "assistant" && content.len() > 200 {
            lighten_markdown(&truncate_str(content, max_chars))
        } else {
            truncate_str(content, max_chars)
        };
        out.push_str(&format!("\n[{}] {}\n", author_tag, lightened));
    }
    out
}

// ─── Algorithm helpers ──────────────────────────────────────────────────────

/// Jaccard similarity between two strings (word-level).
/// Returns 0.0–1.0. Used for detecting near-duplicate blocks.
fn jaccard_similarity(a: &str, b: &str) -> f64 {
    let words_a: std::collections::HashSet<&str> = a.split_whitespace().collect();
    let words_b: std::collections::HashSet<&str> = b.split_whitespace().collect();
    if words_a.is_empty() && words_b.is_empty() { return 1.0; }
    let intersection = words_a.intersection(&words_b).count();
    let union = words_a.union(&words_b).count();
    if union == 0 { return 0.0; }
    intersection as f64 / union as f64
}

/// Fold near-duplicate entries in a list of (label, content) pairs.
/// If Jaccard similarity > threshold, keep the first and replace subsequent with "[similar to above]".
const JACCARD_FOLD_THRESHOLD: f64 = 0.8;

fn fold_similar_blocks(blocks: &mut Vec<String>) {
    if blocks.len() < 2 { return; }
    let mut i = 1;
    while i < blocks.len() {
        if jaccard_similarity(&blocks[i - 1], &blocks[i]) > JACCARD_FOLD_THRESHOLD {
            blocks[i] = format!("[similar to previous entry — folded]");
        }
        i += 1;
    }
}

/// Strip heavy markdown formatting to save tokens.
/// Preserves code blocks and meaningful structure.
fn lighten_markdown(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut in_code_block = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            in_code_block = !in_code_block;
            result.push_str(line);
            result.push('\n');
            continue;
        }
        if in_code_block {
            result.push_str(line);
            result.push('\n');
            continue;
        }
        // Strip bold/italic markers
        let cleaned = line
            .replace("**", "")
            .replace("__", "")
            .replace("*", "")
            .replace("_", " ");
        // Collapse multiple spaces
        let collapsed: String = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
        result.push_str(&collapsed);
        result.push('\n');
    }
    result
}

/// Fold import/use/from/require blocks in code snippets.
/// Replaces consecutive import lines with a summary.
fn fold_import_block(snippet: &str) -> String {
    let lines: Vec<&str> = snippet.lines().collect();
    let mut result: Vec<String> = Vec::new();
    let mut import_buf: Vec<String> = Vec::new();

    let flush_imports = |buf: &mut Vec<String>, out: &mut Vec<String>| {
        if buf.len() > 2 {
            out.push(format!("[{} imports folded]", buf.len()));
        } else {
            out.extend(buf.drain(..));
        }
        buf.clear();
    };

    for line in &lines {
        let trimmed = line.trim();
        let is_import = trimmed.starts_with("import ")
            || trimmed.starts_with("use ")
            || trimmed.starts_with("from ")
            || trimmed.starts_with("require(")
            || (trimmed.starts_with("const ") && trimmed.contains("require("));

        if is_import {
            import_buf.push(line.to_string());
        } else {
            flush_imports(&mut import_buf, &mut result);
            result.push(line.to_string());
        }
    }
    flush_imports(&mut import_buf, &mut result);
    result.join("\n")
}

/// Combine multiple optional system-prompt sections, joining with double newline.
#[allow(dead_code)]
pub fn combine_prompt_parts(parts: impl IntoIterator<Item = Option<String>>) -> Option<String> {
    let joined: String = parts
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join("\n\n");
    if joined.is_empty() { None } else { Some(joined) }
}

/// Build skills section with selective injection.
///
/// Instead of injecting full SKILL.md content, splits each skill by `## ` headers
/// and only includes sections whose header or content matches keywords from the prompt.
/// Unmatched sections are replaced with a compact reference: "[SkillName: N sections omitted]".
pub fn build_skills_section(skill_names: &[String], prompt: &str) -> Option<String> {
    if skill_names.is_empty() {
        return None;
    }

    // Extract keywords from prompt (≥3 chars, lowercased, unique)
    let keywords: Vec<String> = prompt
        .split_whitespace()
        .map(|w| w.to_lowercase())
        .filter(|w| w.len() >= 3)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    let mut skill_blocks = Vec::new();
    for name in skill_names {
        if let Ok(skill) = crate::commands::skills::get_skill(name.clone()) {
            let block = extract_relevant_skill_sections(&skill.name, &skill.content, &keywords);
            skill_blocks.push(block);
        }
    }
    if skill_blocks.is_empty() {
        return None;
    }
    Some(format!("## Active skills\n\n{}", skill_blocks.join("\n\n")))
}

/// Extract only relevant sections from a skill's markdown content.
///
/// Splits by `## ` headers, checks each section for keyword matches,
/// includes matching sections and summarizes omitted ones.
fn extract_relevant_skill_sections(skill_name: &str, content: &str, keywords: &[String]) -> String {
    // Split content by ## headers
    let mut sections: Vec<(&str, &str)> = Vec::new(); // (header, body)
    let mut current_header = "";
    let mut current_start = 0;

    for (i, line) in content.lines().enumerate() {
        if line.starts_with("## ") {
            if i > 0 {
                let body = &content[current_start..content.lines().take(i).map(|l| l.len() + 1).sum::<usize>().saturating_sub(1)];
                sections.push((current_header, body.trim()));
            }
            current_header = line;
            current_start = content.lines().take(i).map(|l| l.len() + 1).sum::<usize>();
        }
    }
    // Last section
    let remaining = &content[current_start..];
    sections.push((current_header, remaining.trim()));

    // If no headers found (flat content) or keywords empty → include everything
    if sections.len() <= 1 || keywords.is_empty() {
        return format!("### {}\n\n{}", skill_name, truncate_str(content, 2000));
    }

    let mut included = Vec::new();
    let mut omitted = 0;

    for (header, body) in &sections {
        let combined = format!("{} {}", header.to_lowercase(), body.to_lowercase());
        let matches = keywords.iter().any(|kw| combined.contains(kw.as_str()));
        if matches || header.is_empty() {
            // Include this section (truncate individual sections to prevent bloat)
            included.push(format!("{}\n{}", header, truncate_str(body, 800)));
        } else {
            omitted += 1;
        }
    }

    let mut result = format!("### {}\n\n", skill_name);
    if !included.is_empty() {
        result.push_str(&included.join("\n\n"));
    }
    if omitted > 0 {
        result.push_str(&format!("\n\n[{}: {} section{} omitted]", skill_name, omitted, if omitted > 1 { "s" } else { "" }));
    }
    result
}

pub fn build_cross_session_section(
    cross_session: &[(String, Vec<(String, String)>)],
) -> Option<String> {
    if cross_session.is_empty() {
        return None;
    }
    let mut blocks = Vec::new();
    for (label, rows) in cross_session {
        if rows.is_empty() {
            continue;
        }
        let mut block = format!("### {}\n", label);
        for (role, content) in rows {
            block.push_str(&format!("\n[{}] {}\n", role, truncate_str(content, 200)));
        }
        blocks.push(block);
    }
    // Fold near-duplicate cross-session blocks
    fold_similar_blocks(&mut blocks);
    if blocks.is_empty() {
        return None;
    }
    Some(format!("## Cross-session context\n\n{}", blocks.join("\n")))
}

/// Keywords that signal code-related intent — rawq is only useful for these.
const CODE_SIGNAL_KEYWORDS: &[&str] = &[
    // 한국어
    "파일", "코드", "함수", "구현", "클래스", "구조", "모듈", "타입", "인터페이스",
    "컴포넌트", "변수", "메서드", "에러", "버그", "수정", "리팩", "검색", "찾아",
    // 영어
    "file", "code", "function", "implement", "class", "struct", "module", "type",
    "interface", "component", "variable", "method", "error", "bug", "fix", "refactor",
    "search", "find", "where", "how does",
    // 경로/확장자 패턴
    "src/", "src\\", ".rs", ".ts", ".tsx", ".js", ".py", ".go", ".java",
];

/// Check if a prompt likely needs code context from rawq.
/// Relaxed: returns true for prompts longer than 10 chars (nearly all real prompts).
/// Short prompts (greetings, single words) still skip.
fn prompt_needs_rawq(prompt: &str) -> bool {
    let lower = prompt.to_lowercase();
    // Always include if explicit code signals
    if CODE_SIGNAL_KEYWORDS.iter().any(|kw| lower.contains(kw)) {
        return true;
    }
    // Include for any substantive prompt (> 10 chars, not just a greeting)
    prompt.trim().len() > 10
}

/// Maximum snippet length per rawq result (chars).
const RAWQ_SNIPPET_MAX_CHARS: usize = 300;
/// Minimum confidence threshold for rawq results (post-filter).
const RAWQ_MIN_CONFIDENCE: f64 = 0.4;
/// Lines within this range are considered overlapping and merged.
const RAWQ_DEDUP_LINE_RANGE: usize = 5;

pub fn build_rawq_section(project_path: Option<&str>, prompt: &str) -> Option<String> {
    let path = project_path?;

    if !prompt_needs_rawq(prompt) {
        eprintln!("[context_pack] rawq skipped — no code signal in prompt");
        return None;
    }

    // Skip search if no index exists (empty project, not yet indexed)
    match rawq::index_status(path) {
        Ok(Some(info)) => {
            // info has files/chunks — proceed with search
            eprintln!("[context_pack] rawq index: {} files", info.files);
            if info.files == 0 {
                eprintln!("[context_pack] rawq skipped — index empty (no code files)");
                return None;
            }
        }
        _ => {
            eprintln!("[context_pack] rawq skipped — no index for project");
            return None;
        }
    }

    // Detect if prompt is conceptual (favor semantic) or code-specific (favor BM25)
    let is_conceptual = !CODE_SIGNAL_KEYWORDS.iter().any(|kw| prompt.to_lowercase().contains(kw));
    let opts = rawq::SearchOptions {
        limit: RAWQ_MAX_RESULTS + 3,
        threshold: 0.3,
        rerank: true,
        token_budget: None,
        text_weight: Some(if is_conceptual { 0.8 } else { 0.5 }),
        rrf_weight: if is_conceptual { Some(0.7) } else { None }, // None = auto-detect
        context_lines: 2,
    };

    match rawq::search_with_options(path, prompt, opts) {
        Ok(mut results) => {
            // Post-processing: filter low confidence
            results.retain(|r| r.confidence >= RAWQ_MIN_CONFIDENCE);

            // Dedup: merge results from same file within ±DEDUP_LINE_RANGE
            results = dedup_rawq_results(results);

            // Sort by confidence descending
            results.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));

            // Take top-K after post-processing
            results.truncate(RAWQ_MAX_RESULTS);

            if results.is_empty() {
                eprintln!("[context_pack] rawq: all results filtered out (low confidence)");
                return None;
            }

            let mut out = String::from("## Code context (rawq)\n");
            for (idx, r) in results.iter().enumerate() {
                let meta = match &r.scope {
                    Some(s) => format!(" ({}, {:.0}%)", s, r.confidence * 100.0),
                    None => format!(" ({:.0}%)", r.confidence * 100.0),
                };

                // Multi-resolution: top 2 = full snippet, next 2 = skeleton, rest = one-line
                let snippet = if idx < 2 {
                    // Full snippet — fold imports, truncate to max
                    let folded = fold_import_block(&r.snippet);
                    if folded.len() > RAWQ_SNIPPET_MAX_CHARS {
                        let end = folded.char_indices()
                            .map(|(i, _)| i)
                            .take_while(|&i| i <= RAWQ_SNIPPET_MAX_CHARS)
                            .last()
                            .unwrap_or(0);
                        format!("{}…", &folded[..end])
                    } else {
                        folded
                    }
                } else if idx < 4 {
                    // Skeleton — first meaningful line only (signature/declaration)
                    r.snippet.lines()
                        .find(|l| {
                            let t = l.trim();
                            !t.is_empty() && !t.starts_with("import ") && !t.starts_with("use ")
                                && !t.starts_with("from ") && !t.starts_with("//") && !t.starts_with("#")
                        })
                        .unwrap_or("")
                        .trim()
                        .to_string()
                } else {
                    // One-line reference
                    String::new()
                };

                if snippet.is_empty() {
                    out.push_str(&format!("\n`{}` L{}{}\n", r.file, r.line, meta));
                } else {
                    out.push_str(&format!("\n`{}` L{}{}:\n{}\n", r.file, r.line, meta, snippet));
                }
            }
            Some(out)
        }
        Err(e) => {
            eprintln!("[context_pack] rawq: {}", e);
            None
        }
    }
}

/// Merge rawq results from the same file within ±N lines.
/// Keeps the entry with higher confidence and merges snippets.
fn dedup_rawq_results(results: Vec<rawq::SearchResult>) -> Vec<rawq::SearchResult> {
    let mut deduped: Vec<rawq::SearchResult> = Vec::new();
    for r in results {
        let merged = deduped.iter_mut().find(|existing| {
            existing.file == r.file
                && (existing.line as isize - r.line as isize).unsigned_abs() <= RAWQ_DEDUP_LINE_RANGE
        });
        if let Some(existing) = merged {
            // Keep higher confidence, merge snippets if distinct
            if r.confidence > existing.confidence {
                existing.confidence = r.confidence;
                existing.scope = r.scope.or(existing.scope.take());
            }
            // Extend snippet if the new one adds info
            if !existing.snippet.contains(&r.snippet) && !r.snippet.contains(&existing.snippet) {
                existing.snippet = format!("{}\n{}", existing.snippet, r.snippet);
            }
        } else {
            deduped.push(r);
        }
    }
    deduped
}

#[allow(dead_code)]
pub fn build_context_summary(
    current_rows: &[(String, String)],
    parent_rows: &[(String, String)],
    is_branch: bool,
) -> Option<String> {
    let has_current = !current_rows.is_empty();
    let has_parent = !parent_rows.is_empty();

    if !has_current && !has_parent {
        return None;
    }

    let mut parts: Vec<String> = Vec::new();

    if has_parent {
        parts.push(format_section("Parent conversation context", parent_rows, 300));
    }

    if has_current {
        let header = if is_branch {
            "Branch conversation history (you are continuing this conversation)"
        } else {
            "Conversation history (you are continuing this conversation — refer to these messages as your own prior responses)"
        };
        parts.push(format_section(header, current_rows, 400));
    }

    Some(parts.join("\n"))
}

/// Build context summary with per-message author attribution.
///
/// Each assistant message shows `[assistant:ProfileName (engine)]` so the model
/// can distinguish which agent authored each past message.
pub fn build_context_summary_with_authors(
    current_rows: &[(String, String, Option<String>, Option<String>)],
    parent_rows: &[(String, String, Option<String>, Option<String>)],
    is_branch: bool,
) -> Option<String> {
    let has_current = !current_rows.is_empty();
    let has_parent = !parent_rows.is_empty();

    if !has_current && !has_parent {
        return None;
    }

    let mut parts: Vec<String> = Vec::new();

    if has_parent {
        parts.push(format_section_with_authors("Parent conversation context", parent_rows, 300));
    }

    if has_current {
        let header = if is_branch {
            "Branch conversation history (each assistant message shows its author — do not claim other agents' messages as your own)"
        } else {
            "Conversation history (each assistant message shows its author — you are continuing this conversation, but do not claim messages authored by other agents as your own)"
        };
        parts.push(format_section_with_authors(header, current_rows, 400));
    }

    Some(parts.join("\n"))
}

/// Build `## Recent Artifacts` from the most recent approved/draft artifacts.
///
/// Takes up to 3 recent artifacts for the conversation, showing title, type,
/// status, and a short content preview. Returns None if no artifacts exist.
pub fn build_artifact_handoff_section(
    conn: &rusqlite::Connection,
    conversation_id: &str,
) -> Option<String> {
    let mut stmt = conn
        .prepare(
            "SELECT title, type, status, content FROM artifacts
             WHERE conversation_id = ?1
             ORDER BY updated_at DESC LIMIT 3",
        )
        .ok()?;
    let rows: Vec<(String, String, String, String)> = stmt
        .query_map([conversation_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .ok()?
        .filter_map(|r| r.ok())
        .collect();

    if rows.is_empty() {
        return None;
    }

    let mut out = String::from("## Recent Artifacts\n");
    for (title, art_type, status, content) in &rows {
        let preview = if content.len() > 120 {
            let end = content.char_indices().map(|(i, _)| i).take_while(|&i| i <= 117).last().unwrap_or(0);
            format!("{}...", &content[..end])
        } else {
            content.clone()
        };
        out.push_str(&format!(
            "\n**{}** ({}·{}): {}\n",
            title, art_type, status, preview
        ));
    }

    Some(out)
}

/// Build `## Recent Agent Findings` from the most recent roundtable_brief memos.
///
/// Queries briefs from:
/// 1. The conversation itself
/// 2. Any RT branch shadow conversations (`branch:{id}` where branch belongs to this conversation)
///
/// This ensures RT branch results are visible in the parent conversation's ContextPack.
pub fn build_findings_section(
    conn: &rusqlite::Connection,
    conversation_id: &str,
) -> Option<String> {
    // Briefs from this conversation + its branch shadow conversations
    let mut stmt = conn
        .prepare(
            "SELECT content FROM memos
             WHERE type = 'roundtable_brief'
               AND (conversation_id = ?1
                    OR conversation_id IN (
                      SELECT 'branch:' || id FROM branches WHERE conversation_id = ?1
                    ))
             ORDER BY created_at DESC LIMIT 3",
        )
        .ok()?;
    let briefs: Vec<String> = stmt
        .query_map([conversation_id], |row| row.get::<_, String>(0))
        .ok()?
        .filter_map(|r| r.ok())
        .collect();

    if briefs.is_empty() {
        return None;
    }

    let mut out = String::from("## Recent Agent Findings\n");
    for (i, brief) in briefs.iter().enumerate() {
        // Take first 600 chars of each brief to keep section short
        let truncated = if brief.len() > 600 {
            let end = brief.char_indices().map(|(i, _)| i).take_while(|&i| i <= 597).last().unwrap_or(0);
            format!("{}...", &brief[..end])
        } else {
            brief.clone()
        };
        if i > 0 {
            out.push_str("\n---\n");
        }
        out.push('\n');
        out.push_str(&truncated);
        out.push('\n');
    }

    Some(out)
}

pub fn build_plan_section(
    conn: &rusqlite::Connection,
    conversation_id: &str,
) -> Option<String> {
    let plan: (String, String, Option<String>) = conn
        .query_row(
            "SELECT id, title, description FROM plans
             WHERE conversation_id = ?1 AND status = 'active'
             ORDER BY updated_at DESC LIMIT 1",
            [conversation_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .ok()?;

    let (plan_id, title, description) = plan;

    let mut stmt = conn
        .prepare(
            "SELECT title, status, details FROM plan_subtasks
             WHERE plan_id = ?1 ORDER BY idx",
        )
        .ok()?;
    let subtasks: Vec<(String, String, Option<String>)> = stmt
        .query_map([&plan_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .ok()?
        .filter_map(|r| r.ok())
        .collect();

    let mut out = format!("## Active Plan\n\n### {}\n", title);
    if let Some(desc) = &description {
        if !desc.is_empty() {
            out.push_str(&format!("{}\n", desc));
        }
    }

    let in_progress: Vec<&str> = subtasks
        .iter()
        .filter(|(_, s, _)| s == "in_progress")
        .map(|(t, _, _)| t.as_str())
        .collect();
    if !in_progress.is_empty() {
        out.push_str(&format!("\n**Current:** {}\n", in_progress.join(", ")));
    }

    if let Some((next_title, _, _)) = subtasks.iter().find(|(_, s, _)| s == "todo") {
        out.push_str(&format!("**Next:** {}\n", next_title));
    }

    let done_count = subtasks.iter().filter(|(_, s, _)| s == "done").count();
    let total = subtasks.len();
    if total > 0 {
        out.push_str(&format!("**Progress:** {}/{} done\n", done_count, total));
    }

    Some(out)
}

/// Resolve branch conversation_id to its parent for plan lookup.
pub fn resolve_plan_conversation_id(conn: &rusqlite::Connection, conversation_id: &str) -> String {
    if !conversation_id.starts_with("branch:") {
        return conversation_id.to_string();
    }
    let branch_id = &conversation_id["branch:".len()..];
    conn.query_row(
        "SELECT conversation_id FROM branches WHERE id = ?1",
        [branch_id],
        |row| row.get::<_, String>(0),
    )
    .unwrap_or_else(|_| conversation_id.to_string())
}

/// Build a lightweight context prefix for non-Claude engines.
///
/// For branch conversations, also includes anchor message + recent parent turns
/// so non-Claude engines in threads still get inherited context.
pub fn build_lite_context_prompt(
    conn: &rusqlite::Connection,
    conversation_id: &str,
    user_prompt: &str,
) -> String {
    use crate::commands::context_queries::{load_anchor_message, load_recent_messages, parent_conversation_id};

    let is_branch = conversation_id.starts_with("branch:");

    // Parent context for branch conversations
    let mut parent_prefix = String::new();
    if is_branch {
        if let Some((_role, content)) = load_anchor_message(conn, conversation_id) {
            let truncated = truncate_str(&content, 400);
            parent_prefix.push_str(&format!("Thread anchor:\n{}\n\n", truncated));
        }
        if let Some(parent_id) = parent_conversation_id(conn, conversation_id) {
            let parent_rows = load_recent_messages(conn, &parent_id, 2);
            if !parent_rows.is_empty() {
                parent_prefix.push_str("Parent conversation:\n");
                for (role, content) in &parent_rows {
                    parent_prefix.push_str(&format!("[{}] {}\n", role, truncate_str(content, 200)));
                }
                parent_prefix.push_str("\n---\n\n");
            }
        }
    }

    let Ok(mut stmt) = conn.prepare(
        "SELECT role, content FROM messages
         WHERE conversation_id = ?1
         ORDER BY timestamp DESC LIMIT ?2",
    ) else {
        return format!("{}{}", parent_prefix, user_prompt);
    };

    let mut rows: Vec<(String, String)> = stmt
        .query_map(params![conversation_id, LITE_CONTEXT_MESSAGES_LIMIT], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map(|mapped| mapped.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();
    rows.reverse();

    if rows.is_empty() && parent_prefix.is_empty() {
        return user_prompt.to_string();
    }

    let mut context = parent_prefix;
    if !rows.is_empty() {
        context.push_str("Recent conversation:\n");
        let mut char_count = context.len();
        for (role, content) in &rows {
            let truncated = truncate_str(content, 600);
            let line = format!("[{}] {}\n", role, truncated);
            if char_count + line.len() > LITE_CONTEXT_MAX_CHARS {
                break;
            }
            context.push_str(&line);
            char_count += line.len();
        }
        context.push_str("\n---\n\n");
    }

    format!("{}{}", context, user_prompt)
}

// ─── Thread / RT inheritance helpers ─────────────────────────────────────────

/// Maximum characters for the anchor message included in inheritance context.
const ANCHOR_MAX_CHARS: usize = 600;
/// Maximum recent parent turns for thread inheritance.
const THREAD_PARENT_RECENT: i64 = 3;
/// Maximum recent parent turns for RT inheritance (more concise).
const RT_PARENT_RECENT: i64 = 2;

/// Build an inheritance context section for a thread (branch) conversation.
///
/// Priority: anchor message > recent parent turns.
/// Does NOT include full parent history — only checkpoint + last N turns.
pub fn build_thread_inheritance_section(
    conn: &rusqlite::Connection,
    branch_conv_id: &str,
) -> Option<String> {
    use crate::commands::context_queries::{load_anchor_message, load_recent_messages, parent_conversation_id};

    let mut parts: Vec<String> = Vec::new();

    // 1. Anchor message (the message this branch was created from)
    if let Some((_role, content)) = load_anchor_message(conn, branch_conv_id) {
        let truncated = truncate_str(&content, ANCHOR_MAX_CHARS);
        parts.push(format!("### Thread Anchor\n\nThis thread was started from the following message:\n{}", truncated));
    }

    // 2. Recent parent turns (2-3)
    if let Some(parent_id) = parent_conversation_id(conn, branch_conv_id) {
        let recent = load_recent_messages(conn, &parent_id, THREAD_PARENT_RECENT);
        if !recent.is_empty() {
            parts.push(format_section("Recent parent conversation", &recent, 300));
        }
    }

    if parts.is_empty() {
        return None;
    }
    Some(format!("## Thread Context\n\n{}", parts.join("\n\n")))
}

/// Build an inheritance context section for a roundtable conversation.
///
/// Priority: explicit source > anchor message > recent parent turns.
/// More concise than thread inheritance.
pub fn build_rt_inheritance_section(
    conn: &rusqlite::Connection,
    conversation_id: &str,
    explicit_source: Option<&str>,
) -> Option<String> {
    use crate::commands::context_queries::{load_anchor_message, load_recent_messages, parent_conversation_id};

    let mut parts: Vec<String> = Vec::new();

    // 1. Explicit source (highest priority)
    if let Some(source) = explicit_source {
        if !source.is_empty() {
            let truncated = truncate_str(source, 800);
            parts.push(format!("### Discussion Source\n\n{}", truncated));
        }
    }

    // 2. Anchor message (if no explicit source, or as supplementary)
    if parts.is_empty() {
        if let Some((_role, content)) = load_anchor_message(conn, conversation_id) {
            let truncated = truncate_str(&content, ANCHOR_MAX_CHARS);
            parts.push(format!("### Discussion Anchor\n\n{}", truncated));
        }
    }

    // 3. Recent parent turns (1-2, concise)
    if let Some(parent_id) = parent_conversation_id(conn, conversation_id) {
        let recent = load_recent_messages(conn, &parent_id, RT_PARENT_RECENT);
        if !recent.is_empty() {
            parts.push(format_section("Recent parent context", &recent, 200));
        }
    }

    if parts.is_empty() {
        return None;
    }
    Some(format!("## Roundtable Context\n\n{}", parts.join("\n\n")))
}

/// Assemble the system prompt component of ContextPack (step 1).
/// If both agent prompt and extra system_prompt are present, they are concatenated.
pub fn assemble_system_prompt(
    agent_name: Option<&str>,
    project_path: Option<&str>,
    extra: Option<&str>,
) -> Option<String> {
    let agent_prompt = agent_name
        .zip(project_path)
        .and_then(|(name, path)| {
            loader::load_agent(path, name)
                .map(|a| a.system_prompt)
                .ok()
        });

    match (agent_prompt, extra) {
        (Some(a), Some(e)) => Some(format!("{}\n\n{}", a, e)),
        (Some(a), None) => Some(a),
        (None, Some(e)) => Some(e.to_string()),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── truncate_str ────────────────────────────────────────────────────
    #[test]
    fn truncate_str_within_limit() {
        assert_eq!(truncate_str("hello", 10), "hello");
    }

    #[test]
    fn truncate_str_over_limit() {
        let result = truncate_str("hello world", 5);
        assert!(result.ends_with('…'));
        assert!(result.len() <= 10); // 5 bytes + ellipsis
    }

    #[test]
    fn truncate_str_empty() {
        assert_eq!(truncate_str("", 10), "");
    }

    // ─── combine_prompt_parts ────────────────────────────────────────────
    #[test]
    fn combine_all_none() {
        assert_eq!(combine_prompt_parts([None, None, None]), None);
    }

    #[test]
    fn combine_some_parts() {
        let result = combine_prompt_parts([
            Some("part1".into()),
            None,
            Some("part2".into()),
        ]);
        assert_eq!(result, Some("part1\n\npart2".into()));
    }

    #[test]
    fn combine_single_part() {
        let result = combine_prompt_parts([Some("only".into()), None]);
        assert_eq!(result, Some("only".into()));
    }

    // ─── build_context_summary ───────────────────────────────────────────
    #[test]
    fn context_summary_empty_inputs() {
        assert_eq!(build_context_summary(&[], &[], false), None);
    }

    #[test]
    fn context_summary_current_only() {
        let current = vec![("user".into(), "hello".into())];
        let result = build_context_summary(&current, &[], false).unwrap();
        assert!(result.contains("Conversation history"));
        assert!(result.contains("hello"));
    }

    #[test]
    fn context_summary_parent_only() {
        let parent = vec![("assistant".into(), "response".into())];
        let result = build_context_summary(&[], &parent, false).unwrap();
        assert!(result.contains("Parent conversation context"));
    }

    #[test]
    fn context_summary_branch_mode() {
        let current = vec![("user".into(), "msg".into())];
        let result = build_context_summary(&current, &[], true).unwrap();
        assert!(result.contains("Branch conversation history"));
    }

    #[test]
    fn context_summary_both() {
        let current = vec![("user".into(), "cur".into())];
        let parent = vec![("user".into(), "par".into())];
        let result = build_context_summary(&current, &parent, true).unwrap();
        assert!(result.contains("Parent conversation context"));
        assert!(result.contains("Branch conversation history"));
    }

    // ─── build_cross_session_section ─────────────────────────────────────
    #[test]
    fn cross_session_empty() {
        assert_eq!(build_cross_session_section(&[]), None);
    }

    #[test]
    fn cross_session_with_data() {
        let data = vec![
            ("Session A".into(), vec![("user".into(), "question".into())]),
        ];
        let result = build_cross_session_section(&data).unwrap();
        assert!(result.contains("Cross-session context"));
        assert!(result.contains("Session A"));
    }

    // ─── assemble_system_prompt ──────────────────────────────────────────
    #[test]
    fn assemble_no_agent_no_extra() {
        assert_eq!(assemble_system_prompt(None, None, None), None);
    }

    #[test]
    fn assemble_extra_only() {
        let result = assemble_system_prompt(None, None, Some("custom prompt"));
        assert_eq!(result, Some("custom prompt".into()));
    }

    // ─── format_section ─────────────────────────────────────────────────
    #[test]
    fn format_section_basic() {
        let rows = vec![("user".into(), "hello".into())];
        let result = format_section("Test", &rows, 100);
        assert!(result.starts_with("## Test\n"));
        assert!(result.contains("[user] hello"));
    }

    // ─── format_section_with_authors ────────────────────────────────────
    #[test]
    fn author_tag_with_persona_and_engine() {
        let rows = vec![
            ("assistant".into(), "response".into(), Some("claude-code".into()), Some("Architect Claude".into())),
        ];
        let result = format_section_with_authors("Test", &rows, 400);
        assert!(result.contains("[assistant:Architect Claude (claude-code)]"));
    }

    #[test]
    fn author_tag_engine_only() {
        let rows = vec![
            ("assistant".into(), "response".into(), Some("gemini".into()), None),
        ];
        let result = format_section_with_authors("Test", &rows, 400);
        assert!(result.contains("[assistant (gemini)]"));
    }

    #[test]
    fn author_tag_user_unchanged() {
        let rows = vec![
            ("user".into(), "question".into(), None, None),
        ];
        let result = format_section_with_authors("Test", &rows, 400);
        assert!(result.contains("[user]"));
        assert!(!result.contains("assistant"));
    }

    // ─── build_context_summary_with_authors ─────────────────────────────
    #[test]
    fn context_summary_authors_attribution_header() {
        let current = vec![
            ("user".into(), "hi".into(), None, None),
            ("assistant".into(), "hello".into(), Some("claude-code".into()), Some("Arch".into())),
        ];
        let result = build_context_summary_with_authors(&current, &[], false).unwrap();
        assert!(result.contains("do not claim messages authored by other agents"));
        assert!(result.contains("[assistant:Arch (claude-code)]"));
    }

    // ─── rawq dedup ────────────────────────────────────────────────────
    #[test]
    fn dedup_merges_same_file_nearby_lines() {
        let results = vec![
            rawq::SearchResult { file: "src/main.rs".into(), line: 10, snippet: "fn main()".into(), scope: None, confidence: 0.9 },
            rawq::SearchResult { file: "src/main.rs".into(), line: 12, snippet: "let x = 1;".into(), scope: None, confidence: 0.8 },
        ];
        let deduped = dedup_rawq_results(results);
        assert_eq!(deduped.len(), 1);
        assert!(deduped[0].confidence >= 0.9); // keeps higher
        assert!(deduped[0].snippet.contains("fn main()"));
    }

    #[test]
    fn dedup_keeps_distant_lines() {
        let results = vec![
            rawq::SearchResult { file: "src/main.rs".into(), line: 10, snippet: "a".into(), scope: None, confidence: 0.9 },
            rawq::SearchResult { file: "src/main.rs".into(), line: 100, snippet: "b".into(), scope: None, confidence: 0.8 },
        ];
        let deduped = dedup_rawq_results(results);
        assert_eq!(deduped.len(), 2);
    }

    #[test]
    fn dedup_keeps_different_files() {
        let results = vec![
            rawq::SearchResult { file: "a.rs".into(), line: 10, snippet: "a".into(), scope: None, confidence: 0.9 },
            rawq::SearchResult { file: "b.rs".into(), line: 10, snippet: "b".into(), scope: None, confidence: 0.8 },
        ];
        let deduped = dedup_rawq_results(results);
        assert_eq!(deduped.len(), 2);
    }

    // ─── Jaccard similarity ─────────────────────────────────────────────
    #[test]
    fn jaccard_identical() {
        assert!((jaccard_similarity("hello world", "hello world") - 1.0).abs() < 0.01);
    }

    #[test]
    fn jaccard_disjoint() {
        assert!(jaccard_similarity("hello world", "foo bar") < 0.01);
    }

    #[test]
    fn jaccard_partial_overlap() {
        let sim = jaccard_similarity("the quick brown fox", "the quick red fox");
        assert!(sim > 0.5 && sim < 1.0);
    }

    #[test]
    fn fold_similar_blocks_removes_duplicates() {
        let mut blocks = vec![
            "user asked about Rust code review".into(),
            "user asked about Rust code review process".into(),
            "completely different topic about Python".into(),
        ];
        fold_similar_blocks(&mut blocks);
        assert!(blocks[1].contains("folded"));
        assert!(!blocks[2].contains("folded"));
    }

    // ─── lighten_markdown ───────────────────────────────────────────────
    #[test]
    fn lighten_strips_bold_italic() {
        let result = lighten_markdown("**bold** and *italic* text");
        assert!(!result.contains("**"));
        assert!(!result.contains("*italic*"));
        assert!(result.contains("bold"));
    }

    #[test]
    fn lighten_preserves_code_blocks() {
        let input = "text\n```rust\nlet **x** = 1;\n```\nmore text";
        let result = lighten_markdown(input);
        assert!(result.contains("let **x** = 1;"));
    }

    // ─── fold_import_block ──────────────────────────────────────────────
    #[test]
    fn fold_imports_large_block() {
        let snippet = "import a\nimport b\nimport c\nimport d\nfn main() {}";
        let folded = fold_import_block(snippet);
        assert!(folded.contains("[4 imports folded]"));
        assert!(folded.contains("fn main()"));
    }

    #[test]
    fn fold_imports_keeps_small_block() {
        let snippet = "import a\nimport b\nfn main() {}";
        let folded = fold_import_block(snippet);
        // 2 imports — not folded (threshold is >2)
        assert!(!folded.contains("folded"));
    }
}
