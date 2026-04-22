//! Project Document RAG — index docs/*.md into conversation_chunks for vector search.
//!
//! Extends the existing vector search infrastructure to cover project documentation
//! (plans, ideas, references, CLAUDE.md) in addition to conversation messages.
//!
//! Features:
//! - Markdown ## section-based chunking
//! - SHA-256 change detection (skip unchanged files)
//! - Inter-document link extraction → document_edges graph
//! - rawq embed for vector similarity search

use rusqlite::{params, Connection};
use sha2::{Sha256, Digest};
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::db::migrations::{now_epoch, now_epoch_ms};
use crate::errors::AppError;

// ═══════════════════════════════════════════════════════════════════════════
// Public types
// ═══════════════════════════════════════════════════════════════════════════

/// A section extracted from a markdown file.
#[derive(Debug, Clone, PartialEq)]
pub struct MarkdownSection {
    pub title: String,
    pub content: String,
}

/// A link extracted from markdown content.
#[derive(Debug, Clone, PartialEq)]
pub struct MarkdownLink {
    pub label: String,
    pub target: String,
}

/// An edge in the document graph.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentEdge {
    pub source_path: String,
    pub target_path: String,
    pub relation: String,
    pub context: Option<String>,
}

/// Result of a document indexing operation.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexResult {
    pub files_scanned: usize,
    pub files_indexed: usize,
    pub files_skipped: usize,
    pub chunks_created: usize,
    pub edges_created: usize,
    pub errors: Vec<String>,
}

/// A document search result.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentSearchResult {
    pub id: String,
    pub file_path: String,
    pub section_title: Option<String>,
    pub text_preview: String,
    pub score: f32,
}

// ═══════════════════════════════════════════════════════════════════════════
// Markdown parser (pure functions)
// ═══════════════════════════════════════════════════════════════════════════

/// Split markdown content by ## headings into sections.
/// The first section (before any heading) gets title "(intro)".
/// Sections longer than 500 chars are further split at paragraph boundaries.
pub fn split_by_headings(content: &str) -> Vec<MarkdownSection> {
    let mut sections = Vec::new();
    let mut current_title = "(intro)".to_string();
    let mut current_lines: Vec<&str> = Vec::new();

    for line in content.lines() {
        if line.starts_with("## ") || line.starts_with("### ") {
            // Flush previous section
            let text = current_lines.join("\n").trim().to_string();
            if !text.is_empty() {
                for sub in split_long_section(&current_title, &text) {
                    sections.push(sub);
                }
            }
            current_title = line.trim().to_string();
            current_lines.clear();
        } else {
            current_lines.push(line);
        }
    }
    // Flush last section
    let text = current_lines.join("\n").trim().to_string();
    if !text.is_empty() {
        for sub in split_long_section(&current_title, &text) {
            sections.push(sub);
        }
    }

    sections
}

/// Split a section that exceeds 500 chars at paragraph boundaries (blank lines).
fn split_long_section(title: &str, text: &str) -> Vec<MarkdownSection> {
    const MAX_CHARS: usize = 500;
    if text.chars().count() <= MAX_CHARS {
        return vec![MarkdownSection { title: title.to_string(), content: text.to_string() }];
    }

    let paragraphs: Vec<&str> = text.split("\n\n").collect();
    let mut sections = Vec::new();
    let mut buffer = String::new();
    let mut part = 1;

    for para in paragraphs {
        if !buffer.is_empty() && buffer.chars().count() + para.chars().count() > MAX_CHARS {
            sections.push(MarkdownSection {
                title: format!("{} (part {})", title, part),
                content: buffer.trim().to_string(),
            });
            buffer.clear();
            part += 1;
        }
        if !buffer.is_empty() {
            buffer.push_str("\n\n");
        }
        buffer.push_str(para);
    }
    if !buffer.is_empty() {
        let final_title = if part > 1 { format!("{} (part {})", title, part) } else { title.to_string() };
        sections.push(MarkdownSection {
            title: final_title,
            content: buffer.trim().to_string(),
        });
    }

    sections
}

/// Extract markdown links `[label](target.md)` from content.
/// Only includes links to .md files (relative paths).
pub fn extract_markdown_links(content: &str) -> Vec<MarkdownLink> {
    let re = regex::Regex::new(r"\[([^\]]*)\]\(([^)]+\.md)\)").unwrap();
    re.captures_iter(content)
        .map(|cap| MarkdownLink {
            label: cap[1].to_string(),
            target: cap[2].to_string(),
        })
        .collect()
}

/// Extract filename mentions (e.g. `somePlan.md`) from plain text.
/// Excludes filenames already captured as markdown link targets.
pub fn extract_filename_mentions(content: &str, exclude_link_targets: &[String]) -> Vec<String> {
    let re = regex::Regex::new(r"\b([a-zA-Z0-9_\-]+\.md)\b").unwrap();
    let exclude: std::collections::HashSet<&str> = exclude_link_targets.iter()
        .map(|t| t.rsplit('/').next().unwrap_or(t))
        .collect();
    let mut seen = std::collections::HashSet::new();
    re.find_iter(content)
        .map(|m| m.as_str().to_string())
        .filter(|name| {
            !exclude.contains(name.as_str()) && seen.insert(name.clone())
        })
        .collect()
}

/// Compute SHA-256 hash of content, return hex string.
pub fn sha256_hex(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

// ═══════════════════════════════════════════════════════════════════════════
// Document indexing
// ═══════════════════════════════════════════════════════════════════════════

/// Index all project documents (docs/*.md, CLAUDE.md, root *.md).
/// Uses SHA-256 change detection to skip unchanged files.
/// Extracts inter-document links into document_edges.
pub fn index_project_documents(
    db: &crate::db::DbState,
    project_key: &str,
    project_path: &str,
) -> Result<IndexResult, AppError> {
    index_project_documents_with_options(db, project_key, project_path, false)
}

/// Same as `index_project_documents` but with `force` option. When force=true,
/// SHA-256 change detection is bypassed — every file is re-indexed. Used by
/// the reindex CLI / HTTP endpoint for resyncing stale DB state after bulk
/// document reorganization.
pub fn index_project_documents_with_options(
    db: &crate::db::DbState,
    project_key: &str,
    project_path: &str,
    force: bool,
) -> Result<IndexResult, AppError> {
    let base = Path::new(project_path);
    let mut result = IndexResult {
        files_scanned: 0,
        files_indexed: 0,
        files_skipped: 0,
        chunks_created: 0,
        edges_created: 0,
        errors: Vec::new(),
    };

    // Collect all markdown files to index
    let mut md_files: Vec<PathBuf> = Vec::new();

    // docs/ directory (recursive)
    let docs_dir = base.join("docs");
    if docs_dir.is_dir() {
        collect_md_files(&docs_dir, &mut md_files);
    }

    // CLAUDE.md at project root
    let claude_md = base.join("CLAUDE.md");
    if claude_md.is_file() {
        md_files.push(claude_md);
    }

    // Root *.md files (README.md etc.)
    if let Ok(entries) = std::fs::read_dir(base) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |e| e == "md") && path != base.join("CLAUDE.md") {
                md_files.push(path);
            }
        }
    }

    result.files_scanned = md_files.len();

    eprintln!("[doc-index] scanning {} for project={}", project_path, project_key);

    // Check rawq daemon before starting (all-or-nothing)
    if !crate::agents::rawq::is_daemon_ready() {
        eprintln!("[doc-index] ERROR: rawq daemon not ready");
        return Err(AppError::Agent("rawq daemon not ready — cannot embed documents".into()));
    }

    // Phase 1: Read files, check hashes, collect sections + edges
    let mut to_index: Vec<(String, String, Vec<MarkdownSection>, Vec<MarkdownLink>, Vec<String>)> = Vec::new(); // (relative_path, content_hash, sections, links, mentions)

    for file_path in &md_files {
        let relative = match file_path.strip_prefix(base) {
            Ok(r) => r.to_string_lossy().to_string(),
            Err(_) => file_path.to_string_lossy().to_string(),
        };

        let content = match std::fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(e) => {
                result.errors.push(format!("{}: read error: {}", relative, e));
                continue;
            }
        };

        let hash = sha256_hex(&content);

        // Change detection: skip if hash unchanged (unless force=true).
        let needs_index = force || {
            let conn = db.read.lock().map_err(|_| AppError::Lock)?;
            let stored_hash: Option<String> = conn.query_row(
                "SELECT content_hash FROM document_index_status WHERE project_key = ?1 AND file_path = ?2",
                params![project_key, relative],
                |r| r.get(0),
            ).ok();
            stored_hash.as_deref() != Some(&hash)
        };

        if !needs_index {
            result.files_skipped += 1;
            continue;
        }

        let sections = split_by_headings(&content);
        let links = extract_markdown_links(&content);
        let link_targets: Vec<String> = links.iter().map(|l| l.target.clone()).collect();
        let mentions = extract_filename_mentions(&content, &link_targets);
        to_index.push((relative, hash, sections, links, mentions));
    }

    eprintln!("[doc-index] scanned={} files, skipped={}, to_index={}", result.files_scanned, result.files_skipped, to_index.len());

    if to_index.is_empty() {
        return Ok(result);
    }

    // Phase 2: Embed sections (outside DB lock — rawq calls are slow)
    let embed_start = std::time::Instant::now();
    struct EmbeddedFile {
        relative_path: String,
        content_hash: String,
        chunks: Vec<(String, String, Vec<u8>)>, // (section_title, text_preview, embedding_blob)
        links: Vec<MarkdownLink>,
        mentions: Vec<String>, // filename mentions (e.g. "somePlan.md")
    }

    let mut embedded_files: Vec<EmbeddedFile> = Vec::new();

    let total_files = to_index.len();
    for (file_idx, (relative, hash, sections, links, mentions)) in to_index.into_iter().enumerate() {
        let mut chunks = Vec::new();
        let _section_count = sections.len();
        for section in &sections {
            if section.content.chars().count() < 20 {
                continue;
            }
            let embed_input = format!("{}\n{}", section.title, section.content);
            match crate::agents::embedder::embed_text(&embed_input, false) {
                Ok(v) => {
                    let blob = super::vector_search::embedding_to_blob(&v);
                    let preview = super::vector_search::truncate_str(&section.content, 300).to_string();
                    chunks.push((section.title.clone(), preview, blob));
                }
                Err(e) => {
                    result.errors.push(format!("{} > {}: embed error: {:?}", relative, section.title, e));
                }
            }
        }
        if (file_idx + 1) % 20 == 0 || file_idx + 1 == total_files {
            eprintln!("[doc-index] embed progress: {}/{} files ({} chunks so far)",
                file_idx + 1, total_files, embedded_files.iter().map(|f| f.chunks.len()).sum::<usize>() + chunks.len());
        }
        embedded_files.push(EmbeddedFile {
            relative_path: relative,
            content_hash: hash,
            chunks,
            links,
            mentions,
        });
    }

    let total_chunks: usize = embedded_files.iter().map(|f| f.chunks.len()).sum();
    let total_errors = result.errors.len();
    eprintln!("[doc-index] embed done in {:.1}s: {} files, {} chunks, {} errors",
        embed_start.elapsed().as_secs_f64(), embedded_files.len(), total_chunks, total_errors);

    // Phase 3: Write to DB — per-file lock/unlock to avoid blocking other writers
    // Document chunks need a conversation_id for FK. Use a sentinel conversation per project.
    let sentinel_conv = format!("__doc__:{}", project_key);
    {
        let conn = db.write.lock().map_err(|_| AppError::Lock)?;
        let now_s = now_epoch();
        conn.execute(
            "INSERT OR IGNORE INTO conversations (id, project_key, label, mode, type, source, created_at, updated_at)
             VALUES (?1, ?2, '[Document Index]', 'system', 'system', 'system', ?3, ?3)",
            params![sentinel_conv, project_key, now_s],
        )?;
    }

    for ef in &embedded_files {
        let conn = db.write.lock().map_err(|_| AppError::Lock)?;

        // Delete old chunks for this file
        let old_rowids: Vec<i64> = conn.prepare(
            "SELECT rowid FROM conversation_chunks WHERE project_key = ?1 AND file_path = ?2 AND source_type = 'document'"
        ).and_then(|mut s| {
            s.query_map(params![project_key, ef.relative_path], |r| r.get(0))
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
        }).unwrap_or_default();

        for rid in &old_rowids {
            conn.execute("DELETE FROM vec_chunks WHERE rowid = ?1", [rid]).ok();
        }
        conn.execute(
            "DELETE FROM conversation_chunks WHERE project_key = ?1 AND file_path = ?2 AND source_type = 'document'",
            params![project_key, ef.relative_path],
        )?;

        // Delete old edges from this file
        conn.execute(
            "DELETE FROM document_edges WHERE project_key = ?1 AND source_path = ?2",
            params![project_key, ef.relative_path],
        )?;

        // Insert new chunks
        let now = now_epoch_ms();
        for (section_title, text_preview, blob) in &ef.chunks {
            let id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO conversation_chunks (id, project_key, conversation_id, kind, root_message_id, text_preview, embedding, created_at, source_type, file_path, section_title)
                 VALUES (?1, ?2, ?3, 'document', '', ?4, ?5, ?6, 'document', ?7, ?8)",
                params![id, project_key, sentinel_conv, text_preview, blob, now, ef.relative_path, section_title],
            )?;

            // Insert into vec_chunks for KNN search
            let chunk_rowid: i64 = conn.query_row(
                "SELECT rowid FROM conversation_chunks WHERE id = ?1", [&id], |r| r.get(0)
            ).unwrap_or(0);
            if chunk_rowid > 0 {
                if let Err(e) = conn.execute(
                    "INSERT INTO vec_chunks(rowid, embedding) VALUES (?1, ?2)",
                    params![chunk_rowid, blob],
                ) {
                    result.errors.push(format!("{}: vec_chunks insert error: {}", ef.relative_path, e));
                }
            }
            result.chunks_created += 1;
        }

        // Insert edges
        let edge_now = now_epoch();
        for link in &ef.links {
            // Normalize target path relative to source directory
            let target_normalized = normalize_link_target(&ef.relative_path, &link.target);
            if let Err(e) = conn.execute(
                "INSERT OR REPLACE INTO document_edges (project_key, source_path, target_path, relation, context, created_at)
                 VALUES (?1, ?2, ?3, 'link', ?4, ?5)",
                params![project_key, ef.relative_path, target_normalized, link.label, edge_now],
            ) {
                result.errors.push(format!("edge {} -> {}: {}", ef.relative_path, target_normalized, e));
            } else {
                result.edges_created += 1;
            }
        }

        // Insert mention edges (filename references in text, not explicit links)
        for mention in &ef.mentions {
            // mention is just a filename like "somePlan.md" — need to find full path
            // Search indexed files for matching filename
            let target_path: Option<String> = conn.query_row(
                "SELECT file_path FROM document_index_status WHERE project_key = ?1 AND file_path LIKE '%/' || ?2",
                params![project_key, mention],
                |r| r.get(0),
            ).ok().or_else(|| {
                // Also try exact match (root files like README.md)
                conn.query_row(
                    "SELECT file_path FROM document_index_status WHERE project_key = ?1 AND file_path = ?2",
                    params![project_key, mention],
                    |r| r.get(0),
                ).ok()
            });
            if let Some(target) = target_path {
                if target != ef.relative_path { // no self-edges
                    let _ = conn.execute(
                        "INSERT OR IGNORE INTO document_edges (project_key, source_path, target_path, relation, context, created_at)
                         VALUES (?1, ?2, ?3, 'mention', ?4, ?5)",
                        params![project_key, ef.relative_path, target, mention, edge_now],
                    );
                    result.edges_created += 1;
                }
            }
        }

        // Update index status
        conn.execute(
            "INSERT OR REPLACE INTO document_index_status (project_key, file_path, content_hash, chunk_count, indexed_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![project_key, ef.relative_path, ef.content_hash, ef.chunks.len() as i64, edge_now],
        )?;

        result.files_indexed += 1;
        drop(conn); // Explicitly release write lock between files
    }

    eprintln!(
        "[doc-index] project={}: scanned={}, indexed={}, skipped={}, chunks={}, edges={}, errors={}",
        project_key, result.files_scanned, result.files_indexed, result.files_skipped,
        result.chunks_created, result.edges_created, result.errors.len()
    );

    Ok(result)
}

/// Normalize a relative link target based on the source file's directory.
/// e.g. source="docs/plans/foo.md", target="./bar.md" → "docs/plans/bar.md"
fn normalize_link_target(source_path: &str, target: &str) -> String {
    let target_clean = target.strip_prefix("./").unwrap_or(target);
    if let Some(dir) = Path::new(source_path).parent() {
        let resolved = dir.join(target_clean);
        // Normalize: remove ".." segments
        let mut parts: Vec<&std::ffi::OsStr> = Vec::new();
        for comp in resolved.components() {
            match comp {
                std::path::Component::ParentDir => { parts.pop(); }
                std::path::Component::Normal(s) => { parts.push(s); }
                _ => {}
            }
        }
        parts.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>().join("/")
    } else {
        target_clean.to_string()
    }
}

/// Recursively collect .md files from a directory.
fn collect_md_files(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                collect_md_files(&path, out);
            } else if path.is_file() && path.extension().map_or(false, |e| e == "md") {
                out.push(path);
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Document search
// ═══════════════════════════════════════════════════════════════════════════

/// Search project documents by vector similarity.
pub fn search_documents(
    db: &crate::db::DbState,
    project_key: &str,
    query: &str,
    limit: usize,
) -> Result<Vec<DocumentSearchResult>, AppError> {
    let query_embedding = crate::agents::embedder::embed_text(query, true)?;

    let conn = db.read.lock().map_err(|_| AppError::Lock)?;
    let query_blob = super::vector_search::embedding_to_blob(&query_embedding);

    // Use vec0 KNN with source_type filter
    let sql = "
        SELECT c.id, c.file_path, c.section_title, c.text_preview, v.distance
        FROM vec_chunks v
        JOIN conversation_chunks c ON c.rowid = v.rowid
        WHERE v.embedding MATCH ?1
          AND k = ?2
          AND c.project_key = ?3
          AND c.source_type = 'document'
        ORDER BY v.distance ASC
    ";

    let mut stmt = conn.prepare(sql).map_err(|e| AppError::Agent(e.to_string()))?;
    let results: Vec<DocumentSearchResult> = stmt
        .query_map(params![query_blob, limit as i64 * 3, project_key], |row| {
            Ok(DocumentSearchResult {
                id: row.get(0)?,
                file_path: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                section_title: row.get(2)?,
                text_preview: row.get(3)?,
                score: 1.0 - row.get::<_, f32>(4)?, // distance → similarity
            })
        })
        .map_err(|e| AppError::Agent(e.to_string()))?
        .filter_map(|r| r.ok())
        .filter(|r| r.score > 0.5) // minimum relevance threshold
        .take(limit)
        .collect();

    Ok(results)
}

// ═══════════════════════════════════════════════════════════════════════════
// Document graph
// ═══════════════════════════════════════════════════════════════════════════

/// Get the document edge graph for a project.
pub fn get_document_graph(
    conn: &Connection,
    project_key: &str,
) -> Vec<DocumentEdge> {
    let sql = "SELECT source_path, target_path, relation, context FROM document_edges WHERE project_key = ?1 ORDER BY source_path";
    conn.prepare(sql)
        .and_then(|mut s| {
            s.query_map([project_key], |row| {
                Ok(DocumentEdge {
                    source_path: row.get(0)?,
                    target_path: row.get(1)?,
                    relation: row.get(2)?,
                    context: row.get(3)?,
                })
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default()
}

/// Get edges connected to a specific document (incoming + outgoing).
pub fn get_document_edges(
    conn: &Connection,
    project_key: &str,
    file_path: &str,
) -> Vec<DocumentEdge> {
    let sql = "
        SELECT source_path, target_path, relation, context FROM document_edges
        WHERE project_key = ?1 AND (source_path = ?2 OR target_path = ?2)
        ORDER BY source_path
    ";
    conn.prepare(sql)
        .and_then(|mut s| {
            s.query_map(params![project_key, file_path], |row| {
                Ok(DocumentEdge {
                    source_path: row.get(0)?,
                    target_path: row.get(1)?,
                    relation: row.get(2)?,
                    context: row.get(3)?,
                })
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default()
}

/// Find orphan documents (no incoming edges from other documents).
pub fn find_orphan_documents(
    conn: &Connection,
    project_key: &str,
) -> Vec<String> {
    let sql = "
        SELECT DISTINCT s.file_path
        FROM document_index_status s
        WHERE s.project_key = ?1
          AND s.file_path NOT IN (
              SELECT target_path FROM document_edges WHERE project_key = ?1
          )
        ORDER BY s.file_path
    ";
    conn.prepare(sql)
        .and_then(|mut s| {
            s.query_map([project_key], |row| row.get(0))
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default()
}

/// Get document index status for a project.
pub fn get_index_status(
    conn: &Connection,
    project_key: &str,
) -> Vec<serde_json::Value> {
    let sql = "SELECT file_path, content_hash, chunk_count, indexed_at FROM document_index_status WHERE project_key = ?1 ORDER BY file_path";
    conn.prepare(sql)
        .and_then(|mut s| {
            s.query_map([project_key], |row| {
                Ok(serde_json::json!({
                    "filePath": row.get::<_, String>(0)?,
                    "contentHash": row.get::<_, String>(1)?,
                    "chunkCount": row.get::<_, i64>(2)?,
                    "indexedAt": row.get::<_, i64>(3)?,
                }))
            })
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default()
}

// ═══════════════════════════════════════════════════════════════════════════
// Tauri commands
// ═══════════════════════════════════════════════════════════════════════════

/// Index project documents (docs/*.md + CLAUDE.md).
///
/// `force=true` 면 SHA change detection 우회 — 모든 파일 재인덱싱. bulk 문서
/// 재조직 후 DB 재동기화 용도.
#[tauri::command]
pub async fn index_project_docs(
    project_key: String,
    force: Option<bool>,
    state: tauri::State<'_, crate::db::DbState>,
) -> Result<IndexResult, AppError> {
    let db = state.inner().clone();
    let project_path = {
        let conn = db.read.lock().map_err(|_| AppError::Lock)?;
        conn.query_row(
            "SELECT path FROM projects WHERE key = ?1",
            [&project_key], |r| r.get::<_, Option<String>>(0),
        ).map_err(|_| AppError::NotFound("project not found".into()))?
            .ok_or_else(|| AppError::NotFound("project has no path".into()))?
    };
    let force = force.unwrap_or(false);

    tokio::task::spawn_blocking(move || {
        index_project_documents_with_options(&db, &project_key, &project_path, force)
    }).await.map_err(|e| AppError::Agent(format!("task join error: {}", e)))?
}

/// Result of a stale cleanup pass.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupResult {
    pub files_checked: usize,
    pub files_removed: usize,
    pub chunks_removed: usize,
    pub edges_removed: usize,
}

/// Remove DB rows (chunks / edges / index_status) for files that no longer
/// exist on disk. Called after bulk document reorganization (e.g. mv plans
/// to archive) to keep the DB in sync with the file system.
pub fn cleanup_stale_documents(
    db: &crate::db::DbState,
    project_key: &str,
    project_path: &str,
) -> Result<CleanupResult, AppError> {
    let base = Path::new(project_path);
    let mut result = CleanupResult {
        files_checked: 0,
        files_removed: 0,
        chunks_removed: 0,
        edges_removed: 0,
    };

    // 1. 모든 indexed file_path 수집
    let file_paths: Vec<String> = {
        let conn = db.read.lock().map_err(|_| AppError::Lock)?;
        let mut stmt = conn.prepare(
            "SELECT DISTINCT file_path FROM conversation_chunks
             WHERE project_key = ?1 AND source_type = 'document' AND file_path IS NOT NULL",
        )?;
        let rows: Vec<String> = stmt
            .query_map([project_key], |r| r.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .collect();
        rows
    };
    result.files_checked = file_paths.len();

    // 2. fs 존재 확인 → missing 목록
    let missing: Vec<String> = file_paths
        .into_iter()
        .filter(|rel| !base.join(rel).is_file())
        .collect();
    result.files_removed = missing.len();

    if missing.is_empty() {
        return Ok(result);
    }

    // 3. chunks + vec_chunks + edges + status 삭제
    let conn = db.write.lock().map_err(|_| AppError::Lock)?;
    for path in &missing {
        // vec_chunks 는 rowid 로 conversation_chunks 와 연결
        let rowids: Vec<i64> = conn.prepare(
            "SELECT rowid FROM conversation_chunks
             WHERE project_key = ?1 AND source_type = 'document' AND file_path = ?2",
        )
        .and_then(|mut s| {
            s.query_map(params![project_key, path], |r| r.get(0))
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();
        for rid in &rowids {
            conn.execute("DELETE FROM vec_chunks WHERE rowid = ?1", [rid]).ok();
        }
        let chunks = conn.execute(
            "DELETE FROM conversation_chunks
             WHERE project_key = ?1 AND source_type = 'document' AND file_path = ?2",
            params![project_key, path],
        ).unwrap_or(0);
        result.chunks_removed += chunks;

        // edges: source_path 또는 target_path 이 missing 이면 삭제
        let edges = conn.execute(
            "DELETE FROM document_edges
             WHERE project_key = ?1 AND (source_path = ?2 OR target_path = ?2)",
            params![project_key, path],
        ).unwrap_or(0);
        result.edges_removed += edges;

        conn.execute(
            "DELETE FROM document_index_status WHERE project_key = ?1 AND file_path = ?2",
            params![project_key, path],
        ).ok();
    }
    eprintln!(
        "[doc-index] cleanup project={}: removed {} stale files, {} chunks, {} edges",
        project_key, result.files_removed, result.chunks_removed, result.edges_removed
    );
    Ok(result)
}

/// Clean up stale document entries for a project (files removed from disk).
/// 보통 `reindex_project_docs` 직전에 호출하는 걸 권장.
#[tauri::command]
pub async fn cleanup_project_stale_docs(
    project_key: String,
    state: tauri::State<'_, crate::db::DbState>,
) -> Result<CleanupResult, AppError> {
    let db = state.inner().clone();
    let project_path = {
        let conn = db.read.lock().map_err(|_| AppError::Lock)?;
        conn.query_row(
            "SELECT path FROM projects WHERE key = ?1",
            [&project_key], |r| r.get::<_, Option<String>>(0),
        ).map_err(|_| AppError::NotFound("project not found".into()))?
            .ok_or_else(|| AppError::NotFound("project has no path".into()))?
    };
    tokio::task::spawn_blocking(move || {
        cleanup_stale_documents(&db, &project_key, &project_path)
    })
    .await
    .map_err(|e| AppError::Agent(format!("task join error: {}", e)))?
}

/// Search project documents by query.
#[tauri::command]
pub fn search_project_docs(
    project_key: String,
    query: String,
    limit: Option<usize>,
    state: tauri::State<crate::db::DbState>,
) -> Result<Vec<DocumentSearchResult>, AppError> {
    let db = state.inner().clone();
    search_documents(&db, &project_key, &query, limit.unwrap_or(10))
}

/// Get document relationship graph for a project.
#[tauri::command]
pub fn get_project_document_graph(
    project_key: String,
    state: tauri::State<crate::db::DbState>,
) -> Result<Vec<DocumentEdge>, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    Ok(get_document_graph(&conn, &project_key))
}

/// Get orphan documents (not referenced by any other document).
#[tauri::command]
pub fn get_orphan_documents(
    project_key: String,
    state: tauri::State<crate::db::DbState>,
) -> Result<Vec<String>, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    Ok(find_orphan_documents(&conn, &project_key))
}

/// Get document index status for a project.
#[tauri::command]
pub fn get_document_index_status(
    project_key: String,
    state: tauri::State<crate::db::DbState>,
) -> Result<Vec<serde_json::Value>, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    Ok(get_index_status(&conn, &project_key))
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ─── split_by_headings ──────────────────────────────────────────────

    #[test]
    fn split_basic_headings() {
        let content = "intro text\n\n## Section One\n\nContent one.\n\n## Section Two\n\nContent two.";
        let sections = split_by_headings(content);
        assert_eq!(sections.len(), 3);
        assert_eq!(sections[0].title, "(intro)");
        assert!(sections[0].content.contains("intro text"));
        assert_eq!(sections[1].title, "## Section One");
        assert!(sections[1].content.contains("Content one"));
        assert_eq!(sections[2].title, "## Section Two");
        assert!(sections[2].content.contains("Content two"));
    }

    #[test]
    fn split_no_headings() {
        let content = "Just plain text\nwith multiple lines\nno headings.";
        let sections = split_by_headings(content);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].title, "(intro)");
    }

    #[test]
    fn split_empty_content() {
        let sections = split_by_headings("");
        assert!(sections.is_empty());
    }

    #[test]
    fn split_h3_headings() {
        let content = "### Sub Section\n\nSub content.";
        let sections = split_by_headings(content);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].title, "### Sub Section");
    }

    #[test]
    fn split_long_section_at_paragraphs() {
        let long_para = "x".repeat(300);
        let content = format!("## Long\n\n{}\n\n{}\n\n{}", long_para, long_para, long_para);
        let sections = split_by_headings(&content);
        // 900 chars total > 500 limit → should be split
        assert!(sections.len() >= 2, "long section should be split, got {}", sections.len());
        assert!(sections[0].title.contains("Long"));
    }

    #[test]
    fn split_preserves_content_integrity() {
        let content = "## Plan\n\n| Col1 | Col2 |\n|---|---|\n| a | b |\n\nMore text.";
        let sections = split_by_headings(content);
        assert_eq!(sections.len(), 1);
        assert!(sections[0].content.contains("| Col1 | Col2 |"));
        assert!(sections[0].content.contains("More text"));
    }

    // ─── extract_markdown_links ────────────────────────────────────────

    #[test]
    fn extract_links_basic() {
        let content = "See [plan doc](./planA.md) and [other](../ideas/foo.md).";
        let links = extract_markdown_links(content);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].label, "plan doc");
        assert_eq!(links[0].target, "./planA.md");
        assert_eq!(links[1].target, "../ideas/foo.md");
    }

    #[test]
    fn extract_links_no_md() {
        let content = "See [image](./photo.png) and [site](https://example.com).";
        let links = extract_markdown_links(content);
        assert!(links.is_empty());
    }

    #[test]
    fn extract_links_empty() {
        assert!(extract_markdown_links("").is_empty());
    }

    #[test]
    fn extract_links_multiple_per_line() {
        let content = "[a](a.md) text [b](b.md) more [c](c.md)";
        let links = extract_markdown_links(content);
        assert_eq!(links.len(), 3);
    }

    // ─── sha256_hex ────────────────────────────────────────────────────

    #[test]
    fn sha256_deterministic() {
        let h1 = sha256_hex("hello world");
        let h2 = sha256_hex("hello world");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64); // SHA-256 = 32 bytes = 64 hex chars
    }

    #[test]
    fn sha256_different_content() {
        assert_ne!(sha256_hex("hello"), sha256_hex("world"));
    }

    // ─── normalize_link_target ─────────────────────────────────────────

    // ─── extract_filename_mentions ────────────────────────────────────

    #[test]
    fn mentions_basic() {
        let content = "See contextPackP0Phase1Plan_2026-03-30.md for details. Also check README.md.";
        let mentions = extract_filename_mentions(content, &[]);
        assert_eq!(mentions.len(), 2);
        assert!(mentions.contains(&"contextPackP0Phase1Plan_2026-03-30.md".to_string()));
        assert!(mentions.contains(&"README.md".to_string()));
    }

    #[test]
    fn mentions_excludes_link_targets() {
        let content = "See [plan](./planA.md) and also planB.md and planA.md again.";
        let links = extract_markdown_links(content);
        let targets: Vec<String> = links.iter().map(|l| l.target.clone()).collect();
        let mentions = extract_filename_mentions(content, &targets);
        assert_eq!(mentions.len(), 1);
        assert_eq!(mentions[0], "planB.md");
    }

    #[test]
    fn mentions_dedup() {
        let content = "foo.md and foo.md and foo.md";
        let mentions = extract_filename_mentions(content, &[]);
        assert_eq!(mentions.len(), 1);
    }

    // ─── normalize_link_target ─────────────────────────────────────────

    #[test]
    fn normalize_same_dir() {
        assert_eq!(
            normalize_link_target("docs/plans/foo.md", "./bar.md"),
            "docs/plans/bar.md"
        );
    }

    #[test]
    fn normalize_parent_dir() {
        assert_eq!(
            normalize_link_target("docs/plans/foo.md", "../ideas/baz.md"),
            "docs/ideas/baz.md"
        );
    }

    #[test]
    fn normalize_no_prefix() {
        assert_eq!(
            normalize_link_target("docs/plans/foo.md", "bar.md"),
            "docs/plans/bar.md"
        );
    }

    #[test]
    fn normalize_root_file() {
        assert_eq!(
            normalize_link_target("CLAUDE.md", "./docs/plans/foo.md"),
            "docs/plans/foo.md"
        );
    }
}
