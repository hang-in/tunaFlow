//! Insight pre-extraction pipeline.
//!
//! Gathers data from rawq, CRG, failure_lessons, test_runner, and conversation
//! memory — organized by analysis category — so the agent receives a focused
//! context instead of scanning the entire project.

use serde::Serialize;
use tauri::State;

use crate::agents::{crg, rawq};
use crate::commands::test_runner;
use crate::db::DbState;
use crate::errors::AppError;

// ─── Output types ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedSnippet {
    pub file: String,
    pub line: usize,
    pub snippet: String,
    pub scope: Option<String>,
    pub confidence: f64,
    pub query: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryExtraction {
    pub category: String,
    pub snippets: Vec<ExtractedSnippet>,
    pub extra_context: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractionResult {
    pub categories: Vec<CategoryExtraction>,
    pub test_output: Option<test_runner::TestRunResult>,
    pub crg_summary: Option<String>,
    pub failure_lessons: Vec<String>,
    pub memory_topics: Vec<String>,
}

// ─── Category → rawq search patterns ────────────────────────────────────────

struct PatternDef {
    category: &'static str,
    queries: &'static [&'static str],
}

const CATEGORY_PATTERNS: &[PatternDef] = &[
    PatternDef {
        category: "stability",
        queries: &[
            "catch {} empty error handling",
            "unwrap() expect() panic",
            "todo! unimplemented!",
            "silent error swallow",
        ],
    },
    PatternDef {
        category: "security",
        queries: &[
            "innerHTML dangerouslySetInnerHTML",
            "SQL string concatenation query",
            ".env secret credential token",
            "eval() Function() unsafe",
        ],
    },
    PatternDef {
        category: "performance",
        queries: &[
            ".clone() unnecessary copy",
            "lock() mutex contention",
            "loop query N+1 database",
            "blocking IO sync operation",
        ],
    },
    PatternDef {
        category: "debt",
        queries: &[
            "TODO FIXME HACK WORKAROUND",
            "deprecated legacy obsolete",
            "dead code unused function",
        ],
    },
    PatternDef {
        category: "architecture",
        queries: &[
            "circular dependency import cycle",
            "god object large class",
        ],
    },
];

// ─── Extraction logic ────────────────────────────────────────────────────────

fn extract_rawq_for_category(
    project_path: &str,
    pattern: &PatternDef,
) -> CategoryExtraction {
    let mut snippets = Vec::new();
    let opts = rawq::SearchOptions {
        limit: 5,
        threshold: 0.35,
        rerank: true,
        token_budget: Some(2000),
        text_weight: Some(0.5),
        rrf_weight: None,
        context_lines: 1,
    };

    for &query in pattern.queries {
        match rawq::search_with_options(project_path, query, rawq::SearchOptions {
            limit: opts.limit,
            threshold: opts.threshold,
            rerank: opts.rerank,
            token_budget: opts.token_budget,
            text_weight: opts.text_weight,
            rrf_weight: opts.rrf_weight,
            context_lines: opts.context_lines,
        }) {
            Ok(results) => {
                for r in results {
                    // Dedup: skip if same file+line already captured
                    let dominated = snippets.iter().any(|s: &ExtractedSnippet| {
                        s.file == r.file && (s.line as i64 - r.line as i64).unsigned_abs() < 5
                    });
                    if !dominated {
                        snippets.push(ExtractedSnippet {
                            file: r.file,
                            line: r.line,
                            snippet: truncate_snippet(&r.snippet, 300),
                            scope: r.scope,
                            confidence: r.confidence,
                            query: query.to_string(),
                        });
                    }
                }
            }
            Err(e) => {
                eprintln!("[insight] rawq search '{}' for {}: {}", query, pattern.category, e);
            }
        }
    }

    // Sort by confidence desc, keep top 15 per category
    snippets.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));
    snippets.truncate(15);

    CategoryExtraction {
        category: pattern.category.to_string(),
        snippets,
        extra_context: Vec::new(),
    }
}

fn extract_crg_summary(project_path: &str) -> Option<String> {
    if !crg::is_available() {
        return None;
    }
    match crg::detect_changes(project_path, "HEAD~5") {
        Ok(val) => {
            let summary = val.get("summary").and_then(|v| v.as_str()).unwrap_or("");
            if summary.is_empty() || summary.contains("No changes") {
                return None;
            }
            // Extract risk-scored files
            let mut parts = vec![format!("## Code Change Summary\n{}", summary)];
            if let Some(files) = val.get("files").and_then(|v| v.as_array()) {
                let top: Vec<String> = files
                    .iter()
                    .take(10)
                    .filter_map(|f| {
                        let path = f.get("path")?.as_str()?;
                        let risk = f.get("risk_score").and_then(|v| v.as_f64()).unwrap_or(0.0);
                        Some(format!("- {} (risk: {:.1})", path, risk))
                    })
                    .collect();
                if !top.is_empty() {
                    parts.push(format!("Risk-scored files:\n{}", top.join("\n")));
                }
            }
            Some(parts.join("\n\n"))
        }
        Err(_) => None,
    }
}

fn extract_failure_lessons(
    conn: &rusqlite::Connection,
    project_key: &str,
) -> Vec<String> {
    let sql = "SELECT finding, file_path, pattern FROM failure_lessons
               WHERE project_key = ?1 AND resolution IS NULL
               ORDER BY created_at DESC LIMIT 20";
    let mut stmt = match conn.prepare(sql) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let rows = stmt.query_map(rusqlite::params![project_key], |row| {
        let finding: String = row.get(0)?;
        let file_path: Option<String> = row.get(1)?;
        let pattern: Option<String> = row.get(2)?;
        let mut entry = finding;
        if let Some(fp) = file_path {
            entry = format!("[{}] {}", fp, entry);
        }
        if let Some(p) = pattern {
            entry = format!("{} (pattern: {})", entry, p);
        }
        Ok(entry)
    }).ok();
    rows.map(|r| r.filter_map(|v| v.ok()).collect()).unwrap_or_default()
}

fn extract_memory_topics(
    conn: &rusqlite::Connection,
    project_key: &str,
) -> Vec<String> {
    // Get recent conversation memory topics across all conversations in the project
    let sql = "SELECT cm.topic, cm.summary FROM conversation_memory cm
               JOIN conversations c ON cm.conversation_id = c.id
               WHERE c.project_key = ?1
               ORDER BY cm.updated_at DESC LIMIT 10";
    let mut stmt = match conn.prepare(sql) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let rows = stmt.query_map(rusqlite::params![project_key], |row| {
        let topic: String = row.get(0)?;
        let summary: String = row.get(1)?;
        Ok(format!("**{}**: {}", topic, truncate_snippet(&summary, 200)))
    }).ok();
    rows.map(|r| r.filter_map(|v| v.ok()).collect()).unwrap_or_default()
}

fn truncate_snippet(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let truncated: String = s.chars().take(max).collect();
    format!("{}...", truncated)
}

// ─── Main extraction command ─────────────────────────────────────────────────

#[tauri::command]
pub async fn run_insight_extraction(
    project_key: String,
    project_path: String,
    categories: Option<Vec<String>>,
    state: State<'_, DbState>,
) -> Result<ExtractionResult, AppError> {
    let pp = project_path.clone();
    let pk = project_key.clone();

    // Determine which categories to extract
    let selected: Vec<&str> = match &categories {
        Some(cats) => cats.iter().map(|s| s.as_str()).collect(),
        None => vec!["stability", "security", "performance", "debt", "architecture", "test"],
    };

    // 1. rawq extractions (sequential — rawq CLI is single-threaded)
    let mut category_extractions: Vec<CategoryExtraction> = Vec::new();
    let rawq_indexed = rawq::is_indexed(&pp).unwrap_or(false);
    eprintln!("[insight] extraction start: project={}, path={}, rawq_indexed={}, categories={:?}",
        pk, pp, rawq_indexed, selected);

    if rawq_indexed {
        for pattern in CATEGORY_PATTERNS {
            if selected.contains(&pattern.category) {
                let extraction = extract_rawq_for_category(&pp, pattern);
                eprintln!("[insight] rawq {} → {} snippets", pattern.category, extraction.snippets.len());
                category_extractions.push(extraction);
            }
        }
    } else {
        eprintln!("[insight] rawq NOT indexed for {} — skipping rawq extraction", pp);
    }

    // Add "test" category placeholder (filled by test runner below)
    if selected.contains(&"test") && !category_extractions.iter().any(|c| c.category == "test") {
        category_extractions.push(CategoryExtraction {
            category: "test".to_string(),
            snippets: Vec::new(),
            extra_context: Vec::new(),
        });
    }

    // 2. Test runner
    let test_output = if selected.contains(&"test") {
        test_runner::run_project_tests(pp.clone(), None).ok()
    } else {
        None
    };

    // Add test results to the test category (always — even if all pass)
    if let Some(ref result) = test_output {
        if let Some(cat) = category_extractions.iter_mut().find(|c| c.category == "test") {
            cat.extra_context.push(format!(
                "Test results: {} passed, {} failed, {} skipped ({})",
                result.passed, result.failed, result.skipped,
                if result.success { "SUCCESS" } else { "FAILURE" }
            ));
            if result.failed > 0 || !result.success {
                let output_preview = truncate_snippet(&result.output, 2000);
                cat.extra_context.push(format!("Test output:\n```\n{}\n```", output_preview));
            }
        }
    }

    // 3. CRG summary
    let crg_summary = extract_crg_summary(&pp);

    // Add CRG data to architecture category
    if let Some(ref summary) = crg_summary {
        if let Some(cat) = category_extractions.iter_mut().find(|c| c.category == "architecture") {
            cat.extra_context.push(summary.clone());
        }
    }

    // 4. Failure lessons + memory (DB reads)
    let (failure_lessons, memory_topics) = {
        let conn = state.read.lock().map_err(|_| AppError::Lock)?;
        let lessons = extract_failure_lessons(&conn, &pk);
        let topics = extract_memory_topics(&conn, &pk);
        (lessons, topics)
    };

    // Add lessons to stability category
    if !failure_lessons.is_empty() {
        if let Some(cat) = category_extractions.iter_mut().find(|c| c.category == "stability") {
            cat.extra_context.push(format!(
                "## Previous Failure Patterns (unresolved)\n{}",
                failure_lessons.iter().take(10).map(|l| format!("- {}", l)).collect::<Vec<_>>().join("\n")
            ));
        }
    }

    // Add memory to architecture category
    if !memory_topics.is_empty() {
        if let Some(cat) = category_extractions.iter_mut().find(|c| c.category == "architecture") {
            cat.extra_context.push(format!(
                "## Design Context (from conversation memory)\n{}",
                memory_topics.iter().take(5).map(|t| format!("- {}", t)).collect::<Vec<_>>().join("\n")
            ));
        }
    }

    Ok(ExtractionResult {
        categories: category_extractions,
        test_output,
        crg_summary,
        failure_lessons,
        memory_topics,
    })
}

// ─── Single-turn agent analysis ──────────────────────────────────────────────

/// Run a single-turn analysis via CLI agent.
/// Supports claude, gemini, codex engines. The prompt already contains all
/// pre-extracted data — the agent only needs to analyze what is provided.
#[tauri::command]
pub async fn run_insight_analysis(
    project_key: String,
    prompt: String,
    engine: Option<String>,
    model: Option<String>,
    system_prompt: Option<String>,
    state: State<'_, DbState>,
) -> Result<String, AppError> {
    use crate::agents::claude::{RunInput, RunOutput};

    // Resolve project path for cwd
    let project_path = {
        let conn = state.read.lock().map_err(|_| AppError::Lock)?;
        conn.query_row(
            "SELECT path FROM projects WHERE key = ?1",
            [&project_key],
            |row| row.get::<_, Option<String>>(0),
        ).ok().flatten()
    };

    let fallback_system = "You are a code quality analyst. Analyze the provided code snippets and context. \
        Report findings in the exact JSON format requested. Be precise with file paths and line numbers. \
        Only report issues you can verify from the provided data. \
        Respond in Korean for descriptions but keep technical terms in English.".to_string();

    let input = RunInput {
        prompt,
        model,
        system_prompt: Some(system_prompt.unwrap_or(fallback_system)),
        resume_token: None,
        project_path, image_paths: Vec::new(),
    };

    let engine_name = engine.unwrap_or_else(|| "claude".to_string());
    eprintln!("[insight] run_insight_analysis: engine={}, model={:?}, prompt_len={}, project_path={:?}",
        engine_name, input.model, input.prompt.len(), input.project_path);

    // Run synchronously in a blocking thread — dispatch by engine
    let result: RunOutput = match engine_name.as_str() {
        "gemini" => {
            tokio::task::spawn_blocking(move || {
                crate::agents::gemini::stream_run(
                    input,
                    |_| {}, |_| {}, || false,
                )
            })
            .await
            .map_err(|e| AppError::Agent(format!("spawn_blocking failed: {}", e)))?
            .map_err(|e| AppError::Agent(format!("gemini analysis failed: {}", e)))?
        }
        "codex" => {
            tokio::task::spawn_blocking(move || {
                crate::agents::codex::stream_run(
                    input,
                    |_| {}, |_| {},
                )
            })
            .await
            .map_err(|e| AppError::Agent(format!("spawn_blocking failed: {}", e)))?
            .map_err(|e| AppError::Agent(format!("codex analysis failed: {}", e)))?
        }
        _ => {
            // Default: claude
            tokio::task::spawn_blocking(move || {
                crate::agents::claude::stream_run(
                    input,
                    |_| {}, |_| {}, || false,
                )
            })
            .await
            .map_err(|e| AppError::Agent(format!("spawn_blocking failed: {}", e)))?
            .map_err(|e| AppError::Agent(format!("claude analysis failed: {}", e)))?
        }
    };

    eprintln!("[insight] analysis done: engine={}, input_tokens={}, output_tokens={}, cost=${:.4}",
        engine_name, result.input_tokens, result.output_tokens, result.cost_usd);

    // Return content + usage as JSON
    let response = serde_json::json!({
        "content": result.content,
        "inputTokens": result.input_tokens,
        "outputTokens": result.output_tokens,
        "costUsd": result.cost_usd,
    });
    Ok(response.to_string())
}
