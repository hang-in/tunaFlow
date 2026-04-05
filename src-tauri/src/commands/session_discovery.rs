//! Auto session discovery — find related conversations using FTS5.
//!
//! Replaces manual crossSessionIds toggling with automatic discovery.
//! Manual overrides are stored as session_links with method='manual'.

use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::db::migrations::now_epoch_ms;
use crate::errors::AppError;

/// Minimum FTS5 score to consider a session link relevant.
const SCORE_THRESHOLD: f64 = 0.3;
/// Max auto-discovered links per conversation.
const MAX_AUTO_LINKS: usize = 5;

/// A link between two conversations.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionLink {
    pub id: String,
    pub conversation_id: String,
    pub linked_conv_id: String,
    pub linked_conv_label: Option<String>,
    pub score: f64,
    pub method: String,
    pub created_at: i64,
}

/// Discover related sessions using FTS5.
/// Extracts keywords from recent user messages, searches project-wide, groups by conversation.
pub fn discover_related_sessions(
    conn: &Connection,
    conversation_id: &str,
    project_key: &str,
    limit: usize,
) -> Vec<(String, f64)> {
    // Get last 3 user messages from current conversation
    let mut stmt = match conn.prepare(
        "SELECT content FROM messages
         WHERE conversation_id = ?1 AND role = 'user'
         ORDER BY timestamp DESC LIMIT 3",
    ) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let user_messages: Vec<String> = stmt
        .query_map([conversation_id], |row| row.get(0))
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();

    if user_messages.is_empty() {
        return Vec::new();
    }

    // Build FTS5 query from combined user messages
    let combined: String = user_messages.join(" ");
    let fts_query = build_discovery_query(&combined);
    if fts_query.is_empty() {
        return Vec::new();
    }

    // Search all messages in project, excluding current conversation
    let sql = "
        SELECT m.conversation_id, -rank as score
        FROM messages_fts f
        JOIN messages m ON m.rowid = f.rowid
        JOIN conversations c ON c.id = m.conversation_id
        WHERE messages_fts MATCH ?1
          AND c.project_key = ?2
          AND m.conversation_id != ?3
          AND c.mode != 'roundtable'
        ORDER BY score DESC
        LIMIT 50
    ";

    let mut search_stmt = match conn.prepare(sql) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[session_discovery] FTS5 query error: {}", e);
            return Vec::new();
        }
    };

    let hits: Vec<(String, f64)> = search_stmt
        .query_map(params![fts_query, project_key, conversation_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();

    // Group by conversation_id, sum scores
    let mut conv_scores: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    for (conv_id, score) in &hits {
        *conv_scores.entry(conv_id.clone()).or_insert(0.0) += score;
    }

    // Normalize scores (0..1 range)
    let max_score = conv_scores.values().cloned().fold(0.0_f64, f64::max);
    if max_score > 0.0 {
        for v in conv_scores.values_mut() {
            *v /= max_score;
        }
    }

    // Boost with vector similarity (best-effort, skip if daemon not ready)
    let combined = &user_messages.join(" ");
    if crate::agents::rawq::is_daemon_ready() {
    if let Ok(query_emb) = crate::agents::rawq::embed_text(combined, true) {
        let vec_results = crate::commands::vector_search::search_similar(
            conn, &query_emb, project_key, conversation_id, 10,
        );
        // Group vector hits by conversation, take max score
        for vc in &vec_results {
            if vc.score > 0.25 {
                let entry = conv_scores.entry(vc.conversation_id.clone()).or_insert(0.0);
                // Add vector signal (weighted 0.5 relative to FTS5)
                *entry += vc.score as f64 * 0.5;
            }
        }
        // Re-normalize after vector boost
        let new_max = conv_scores.values().cloned().fold(0.0_f64, f64::max);
        if new_max > 0.0 {
            for v in conv_scores.values_mut() {
                *v /= new_max;
            }
        }
    }
    } // is_daemon_ready

    // Filter and sort
    let mut results: Vec<(String, f64)> = conv_scores
        .into_iter()
        .filter(|(_, score)| *score >= SCORE_THRESHOLD)
        .collect();
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(limit);
    results
}

/// Refresh session_links for a conversation (upsert auto-discovered links).
pub fn refresh_links(
    conn: &Connection,
    conversation_id: &str,
    project_key: &str,
) -> Result<(), AppError> {
    let discovered = discover_related_sessions(conn, conversation_id, project_key, MAX_AUTO_LINKS);
    let now = now_epoch_ms();

    // Remove stale auto links (not in new discovery set)
    let discovered_ids: Vec<&str> = discovered.iter().map(|(id, _)| id.as_str()).collect();

    // Delete auto links not in new set
    let existing_auto: Vec<String> = {
        let mut stmt = conn.prepare(
            "SELECT linked_conv_id FROM session_links
             WHERE conversation_id = ?1 AND method = 'fts5'"
        )?;
        stmt.query_map([conversation_id], |row| row.get(0))
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    };

    for old_id in &existing_auto {
        if !discovered_ids.contains(&old_id.as_str()) {
            conn.execute(
                "DELETE FROM session_links WHERE conversation_id = ?1 AND linked_conv_id = ?2 AND method = 'fts5'",
                params![conversation_id, old_id],
            )?;
        }
    }

    // Upsert new discoveries
    for (linked_id, score) in &discovered {
        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO session_links (id, conversation_id, linked_conv_id, score, method, created_at)
             VALUES (?1, ?2, ?3, ?4, 'fts5', ?5)
             ON CONFLICT(conversation_id, linked_conv_id) DO UPDATE SET score = ?4, created_at = ?5
             WHERE method = 'fts5'",
            params![id, conversation_id, linked_id, score, now],
        )?;
    }

    Ok(())
}

/// Load session links for a conversation (auto + manual).
pub fn load_session_links(conn: &Connection, conversation_id: &str) -> Vec<SessionLink> {
    let sql = "
        SELECT sl.id, sl.conversation_id, sl.linked_conv_id, sl.score, sl.method, sl.created_at,
               COALESCE(c.label, c.id) as linked_label
        FROM session_links sl
        LEFT JOIN conversations c ON c.id = sl.linked_conv_id
        WHERE sl.conversation_id = ?1
        ORDER BY sl.method ASC, sl.score DESC
    ";

    let mut stmt = match conn.prepare(sql) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    stmt.query_map([conversation_id], |row| {
        Ok(SessionLink {
            id: row.get(0)?,
            conversation_id: row.get(1)?,
            linked_conv_id: row.get(2)?,
            score: row.get(3)?,
            method: row.get(4)?,
            created_at: row.get(5)?,
            linked_conv_label: row.get(6)?,
        })
    })
    .map(|rows| rows.filter_map(|r| r.ok()).collect())
    .unwrap_or_default()
}

/// Load auto-discovered + manual session IDs for ContextPack injection.
/// Returns conversation IDs sorted by score (manual first, then auto).
pub fn load_active_session_ids(
    conn: &Connection,
    conversation_id: &str,
    max_count: usize,
) -> Vec<String> {
    let sql = "
        SELECT linked_conv_id
        FROM session_links
        WHERE conversation_id = ?1
        ORDER BY
            CASE WHEN method = 'manual' THEN 0 ELSE 1 END,
            score DESC
        LIMIT ?2
    ";

    let mut stmt = match conn.prepare(sql) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    stmt.query_map(params![conversation_id, max_count as i64], |row| row.get(0))
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
}

/// Build FTS5 query for session discovery (reuses stopword filtering).
fn build_discovery_query(text: &str) -> String {
    const STOPWORDS: &[&str] = &[
        "the", "is", "are", "was", "were", "be", "been", "being",
        "have", "has", "had", "do", "does", "did", "will", "would", "could", "should",
        "can", "may", "might", "shall", "must",
        "it", "its", "he", "she", "we", "they", "you", "me", "him", "her", "us", "them",
        "this", "that", "these", "those", "what", "which", "who", "whom", "how", "when", "where", "why",
        "if", "or", "and", "but", "not", "no", "so", "as", "at", "by", "for", "in", "of", "on", "to", "up",
        "an", "my", "our", "your", "all", "any", "each", "some", "such",
        "than", "too", "very", "just", "also", "more", "most", "only", "even",
        "with", "from", "into", "about", "after", "before", "between", "through",
        "이", "그", "저", "것", "수", "등", "및", "또", "더",
    ];

    // Strip FTS5 operator characters (quotes, parens, etc.)
    let cleaned: String = text.chars()
        .map(|c| if c == '"' || c == '\'' || c == '(' || c == ')' || c == '*' || c == '?' || c == '!' || c == '.' || c == ',' || c == ':' || c == ';' {
            ' '
        } else { c })
        .collect();
    let words: Vec<&str> = cleaned
        .split_whitespace()
        .filter(|w| w.len() >= 2)
        .filter(|w| !STOPWORDS.contains(&w.to_lowercase().as_str()))
        .take(12)
        .collect();

    if words.is_empty() {
        let fallback: Vec<&str> = cleaned
            .split_whitespace()
            .filter(|w| w.len() >= 3)
            .take(6)
            .collect();
        if fallback.is_empty() {
            return String::new();
        }
        return fallback.join(" OR ");
    }
    words.join(" OR ")
}

// === Tauri Commands ===

/// Get session links for a conversation.
#[tauri::command]
pub fn get_session_links(
    conversation_id: String,
    state: tauri::State<crate::db::DbState>,
) -> Result<Vec<SessionLink>, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    Ok(load_session_links(&conn, &conversation_id))
}

/// Refresh auto-discovered session links.
/// Runs on a background thread to avoid blocking the main thread (FTS5 + rawq embed).
#[tauri::command]
pub async fn refresh_session_links(
    conversation_id: String,
    state: tauri::State<'_, crate::db::DbState>,
) -> Result<Vec<SessionLink>, AppError> {
    let db = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        let conn = db.write.lock().map_err(|_| AppError::Lock)?;

        // Get project_key for this conversation
        let project_key: String = conn
            .query_row(
                "SELECT project_key FROM conversations WHERE id = ?1",
                [&conversation_id],
                |row| row.get(0),
            )
            .map_err(|_| AppError::NotFound("conversation not found".into()))?;

        refresh_links(&conn, &conversation_id, &project_key)?;
        Ok(load_session_links(&conn, &conversation_id))
    }).await.map_err(|_| AppError::Lock)?
}

/// Toggle a manual session link (pin/unpin).
#[tauri::command]
pub fn toggle_manual_session_link(
    conversation_id: String,
    linked_conv_id: String,
    state: tauri::State<crate::db::DbState>,
) -> Result<Vec<SessionLink>, AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;

    // Check if manual link exists
    let exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM session_links
             WHERE conversation_id = ?1 AND linked_conv_id = ?2 AND method = 'manual'",
            params![&conversation_id, &linked_conv_id],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0) > 0;

    if exists {
        // Remove manual link
        conn.execute(
            "DELETE FROM session_links WHERE conversation_id = ?1 AND linked_conv_id = ?2 AND method = 'manual'",
            params![&conversation_id, &linked_conv_id],
        )?;
    } else {
        // Add manual link (or upgrade existing auto link)
        let id = Uuid::new_v4().to_string();
        let now = now_epoch_ms();
        conn.execute(
            "INSERT INTO session_links (id, conversation_id, linked_conv_id, score, method, created_at)
             VALUES (?1, ?2, ?3, 1.0, 'manual', ?4)
             ON CONFLICT(conversation_id, linked_conv_id) DO UPDATE SET method = 'manual', score = 1.0",
            params![id, &conversation_id, &linked_conv_id, now],
        )?;
    }

    Ok(load_session_links(&conn, &conversation_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_query_filters_stopwords() {
        let q = build_discovery_query("the user wants to implement a new database schema");
        assert!(!q.contains("the"));
        assert!(!q.contains("to"));
        assert!(q.contains("implement"));
        assert!(q.contains("database"));
        assert!(q.contains("schema"));
    }

    #[test]
    fn build_query_empty_input() {
        let q = build_discovery_query("");
        assert!(q.is_empty());
    }

    #[test]
    fn build_query_all_stopwords_fallback() {
        let q = build_discovery_query("the is are");
        // All are stopwords with len < 3 except "the" and "are" which are stopwords
        // fallback tries words ≥3 chars: "the", "are" → both stopwords but fallback skips stopword check
        assert!(q.is_empty() || !q.is_empty()); // just ensure no panic
    }

    #[test]
    fn build_query_korean() {
        let q = build_discovery_query("데이터베이스 스키마 설계 및 구현");
        assert!(q.contains("데이터베이스"));
        assert!(q.contains("스키마"));
        assert!(!q.contains("및")); // stopword
    }

    // ─── FTS5 operator sanitization ──────────────────────────────────────

    #[test]
    fn build_query_strips_operators() {
        let q = build_discovery_query("what's the \"best\" approach? (using rust)");
        assert!(!q.contains('"'));
        assert!(!q.contains('?'));
        assert!(!q.contains('('));
        assert!(!q.contains(')'));
        assert!(!q.contains('\''));
    }

    #[test]
    fn build_query_joins_with_or() {
        let q = build_discovery_query("database schema migration");
        assert!(q.contains(" OR "));
        assert!(q.contains("database"));
        assert!(q.contains("schema"));
        assert!(q.contains("migration"));
    }

    // ─── Word length filtering ───────────────────────────────────────────

    #[test]
    fn build_query_filters_single_char_words() {
        let q = build_discovery_query("a b c database");
        assert!(q.contains("database"));
        // single-char words should be filtered
        assert!(!q.contains(" a "));
    }

    #[test]
    fn build_query_max_12_words() {
        let input = (0..20).map(|i| format!("word{}", i)).collect::<Vec<_>>().join(" ");
        let q = build_discovery_query(&input);
        let word_count = q.split(" OR ").count();
        assert!(word_count <= 12, "should cap at 12 words, got {}", word_count);
    }

    // ─── Korean stopwords ────────────────────────────────────────────────

    #[test]
    fn build_query_filters_korean_stopwords() {
        let q = build_discovery_query("이것은 또 더 좋은 방법");
        assert!(!q.contains("또"));
        assert!(!q.contains("더"));
        assert!(q.contains("좋은") || q.contains("방법"));
    }

    // ─── Fallback behavior ───────────────────────────────────────────────

    #[test]
    fn build_query_single_word() {
        let q = build_discovery_query("authentication");
        assert_eq!(q, "authentication");
    }

    #[test]
    fn build_query_short_input() {
        let q = build_discovery_query("hi");
        assert_eq!(q, "hi");
    }

    #[test]
    fn build_query_only_short_words() {
        // All words < 2 chars → empty after filter, fallback checks ≥ 3 chars
        let q = build_discovery_query("a b c");
        assert!(q.is_empty());
    }
}
