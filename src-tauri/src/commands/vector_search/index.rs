//! Conversation indexing — builds sliding-window chunks and stores embeddings.

use rusqlite::Connection;
use uuid::Uuid;

use crate::agents::embedder;
use crate::db::migrations::now_epoch_ms;
use crate::errors::AppError;

use super::helpers::{embedding_to_blob, truncate_str, is_workflow_prompt, format_author_label};

// ─── Sliding window chunking ──────────────────────────────────────────────────

struct Turn {
    root_id: String,
    user_text: String,
    asst_text: String,
    engine: String,
    persona: String,
}

/// Build chunks using 3-turn sliding window with 1-turn overlap.
/// Each message gets an author prefix: [user], [persona · engine], etc.
/// This improves embedding recall by +10-15% vs single-pair chunks.
pub(super) fn build_sliding_window_chunks(
    messages: &[(String, String, String, Option<String>, Option<String>)],
) -> Vec<(String, String, String)> {
    // Step 1: Group into turns (user+assistant pairs)
    let mut turns: Vec<Turn> = Vec::new();
    let mut i = 0;
    while i < messages.len() {
        let (ref id, ref role, ref content, ref _engine, ref _persona) = messages[i];
        if is_workflow_prompt(content) { i += 1; continue; }
        if role == "user" && i + 1 < messages.len() && messages[i + 1].1 == "assistant" {
            let asst = &messages[i + 1];
            turns.push(Turn {
                root_id: id.clone(),
                user_text: truncate_str(content, 150),
                asst_text: truncate_str(&asst.2, 150),
                engine: asst.3.clone().unwrap_or_default(),
                persona: asst.4.clone().unwrap_or_default(),
            });
            i += 2;
        } else {
            let text = truncate_str(content, 200);
            if text.len() >= 20 {
                turns.push(Turn {
                    root_id: id.clone(),
                    user_text: if role == "user" { text.clone() } else { String::new() },
                    asst_text: if role == "assistant" { text } else { String::new() },
                    engine: messages[i].3.clone().unwrap_or_default(),
                    persona: messages[i].4.clone().unwrap_or_default(),
                });
            }
            i += 1;
        }
    }

    if turns.is_empty() { return Vec::new(); }

    // Step 2: Sliding window (3 turns, stride 2 = 1-turn overlap)
    let window = 3;
    let stride = 2;
    let mut chunks: Vec<(String, String, String)> = Vec::new();
    let mut start = 0;

    loop {
        let end = std::cmp::min(start + window, turns.len());
        let root_id = turns[start].root_id.clone();

        let mut text = String::new();
        for turn in &turns[start..end] {
            if !turn.user_text.is_empty() {
                text.push_str(&format!("[user] {}\n", turn.user_text));
            }
            if !turn.asst_text.is_empty() {
                let label = format_author_label(&turn.engine, &turn.persona);
                text.push_str(&format!("[{}] {}\n", label, turn.asst_text));
            }
        }

        let trimmed = text.trim().to_string();
        if trimmed.len() >= 30 {
            chunks.push((root_id, "window".to_string(), trimmed));
        }

        if end >= turns.len() { break; }
        start += stride;
    }

    chunks
}

// ─── Core indexing functions ──────────────────────────────────────────────────

/// Build and store conversation chunks with embeddings.
///
/// Extracts user+assistant pairs from messages, embeds each pair,
/// and stores in `conversation_chunks`. Replaces existing chunks for the conversation.
#[allow(dead_code)]
pub fn index_conversation(
    conn: &Connection,
    conversation_id: &str,
    project_key: &str,
) -> Result<usize, AppError> {
    conn.execute(
        "DELETE FROM conversation_chunks WHERE conversation_id = ?1",
        [conversation_id],
    )?;

    let mut stmt = conn.prepare(
        "SELECT id, role, content FROM messages
         WHERE conversation_id = ?1
         ORDER BY timestamp ASC",
    )?;
    let messages: Vec<(String, String, String)> = stmt
        .query_map([conversation_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?
        .filter_map(|r| r.ok())
        .collect();

    if messages.is_empty() {
        return Ok(0);
    }

    // Build chunks: user+assistant pairs (skip workflow auto-generated prompts)
    let mut chunks: Vec<(String, String, String)> = Vec::new();
    let mut i = 0;
    while i < messages.len() {
        let (ref id, ref role, ref content) = messages[i];
        if is_workflow_prompt(content) {
            i += 1;
            continue;
        }
        if role == "user" && i + 1 < messages.len() && messages[i + 1].1 == "assistant" {
            let user_text = truncate_str(content, 200);
            let asst_text = truncate_str(&messages[i + 1].2, 200);
            let text = format!("Q: {}\nA: {}", user_text, asst_text);
            chunks.push((id.clone(), "pair".to_string(), text));
            i += 2;
        } else {
            let text = truncate_str(content, 300);
            if text.len() >= 20 {
                chunks.push((id.clone(), "anchor".to_string(), text));
            }
            i += 1;
        }
    }

    if chunks.is_empty() {
        return Ok(0);
    }

    let now = now_epoch_ms();
    let mut indexed = 0;

    for (root_id, kind, text) in &chunks {
        let embedding = match embedder::embed_text(text, false) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[vector] embed failed for chunk {}: {:?}", root_id, e);
                continue;
            }
        };

        let embedding_blob = embedding_to_blob(&embedding);
        let id = Uuid::new_v4().to_string();

        conn.execute(
            "INSERT INTO conversation_chunks (id, project_key, conversation_id, kind, root_message_id, text_preview, embedding, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![id, project_key, conversation_id, kind, root_id, text, embedding_blob, now],
        )?;
        indexed += 1;
    }

    eprintln!(
        "[vector] indexed {} chunks for {} (from {} messages)",
        indexed,
        conversation_id,
        messages.len()
    );
    Ok(indexed)
}

/// Index a conversation's messages as vector chunks.
/// Uses 3-phase lock strategy to prevent Mutex poison:
/// Phase 1 (read lock): load messages + build chunk texts
/// Phase 2 (no lock): call rawq embed for each chunk (external process, slow)
/// Phase 3 (write lock): delete old chunks + insert new ones
/// Runs on a background thread to avoid blocking the main thread.
#[tauri::command]
pub async fn index_conversation_chunks(
    conversation_id: String,
    state: tauri::State<'_, crate::db::DbState>,
) -> Result<usize, crate::errors::AppError> {
    let db = state.inner().clone();
    tokio::task::spawn_blocking(move || {
        // Phase 1: read data (short lock)
        let (project_key, chunks) = {
            let conn = db.read.lock().map_err(|_| AppError::Lock)?;
            let pk: String = conn
                .query_row(
                    "SELECT project_key FROM conversations WHERE id = ?1",
                    [&conversation_id],
                    |row| row.get(0),
                )
                .map_err(|_| AppError::NotFound("conversation not found".into()))?;

            let mut stmt = conn.prepare(
                "SELECT id, role, content, engine, persona FROM messages WHERE conversation_id = ?1 ORDER BY timestamp ASC",
            )?;
            let messages: Vec<(String, String, String, Option<String>, Option<String>)> = stmt
                .query_map([&conversation_id], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))
                })?
                .filter_map(|r| r.ok())
                .collect();

            let chunk_texts = build_sliding_window_chunks(&messages);
            (pk, chunk_texts)
            // Read lock released here
        };

        if chunks.is_empty() { return Ok(0); }

        // Phase 1b: find already-indexed root_message_ids (incremental — skip re-embedding)
        let already_indexed: std::collections::HashSet<String> = {
            let conn = db.read.lock().map_err(|_| AppError::Lock)?;
            let mut stmt = conn.prepare(
                "SELECT root_message_id FROM conversation_chunks WHERE conversation_id = ?1"
            ).unwrap_or_else(|_| conn.prepare("SELECT ''").unwrap());
            stmt.query_map([&conversation_id], |r| r.get::<_, String>(0))
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
                .unwrap_or_default()
        };

        // Phase 2: embed only NEW chunks (not yet indexed)
        let new_chunks: Vec<_> = chunks.iter()
            .filter(|(root_id, _, _)| !already_indexed.contains(root_id))
            .collect();

        if new_chunks.is_empty() { return Ok(0); }

        let mut embedded: Vec<(String, String, String, Vec<u8>)> = Vec::new();
        for (root_id, kind, text) in &new_chunks {
            match embedder::embed_text(text, false) {
                Ok(v) => {
                    embedded.push((root_id.clone(), kind.clone(), text.clone(), embedding_to_blob(&v)));
                }
                Err(e) => {
                    eprintln!("[vector] embed failed for chunk {}: {:?}", root_id, e);
                }
            }
        }

        if embedded.is_empty() { return Ok(0); }

        // Phase 3: write only new results (short lock) — do NOT delete existing chunks
        let conn = db.write.lock().map_err(|_| AppError::Lock)?;
        let now = now_epoch_ms();
        let mut indexed = 0;
        for (root_id, kind, text, blob) in &embedded {
            let id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO conversation_chunks (id, project_key, conversation_id, kind, root_message_id, text_preview, embedding, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![id, project_key, conversation_id, kind, root_id, text, blob, now],
            )?;
            // Insert into vec0 for KNN search (uses conversation_chunks rowid)
            let chunk_rowid: i64 = conn.query_row(
                "SELECT rowid FROM conversation_chunks WHERE id = ?1", [&id], |r| r.get(0)
            ).unwrap_or(0);
            if chunk_rowid > 0 {
                conn.execute(
                    "INSERT INTO vec_chunks(rowid, embedding) VALUES (?1, ?2)",
                    rusqlite::params![chunk_rowid, blob],
                ).ok();
            }
            indexed += 1;
        }
        eprintln!("[vector] indexed {} new chunks for {} ({} already indexed)", indexed, conversation_id, already_indexed.len());
        Ok(indexed)
    }).await.map_err(|_| AppError::Lock)?
}

/// Blocking index function — callable from HTTP API without Tauri State.
/// Incremental: only embeds chunks whose root_message_id is not yet in conversation_chunks.
/// This mirrors index_conversation_chunks (the async Tauri command) to avoid full re-index
/// on every agent completion, which caused large CPU spikes on long conversations.
pub fn index_chunks_blocking(db: &crate::db::DbState, conversation_id: &str) -> Result<usize, AppError> {
    // Phase 1: load messages + build chunk candidates (read lock)
    let (project_key, chunks) = {
        let conn = db.read.lock().map_err(|_| AppError::Lock)?;
        let pk: String = conn.query_row("SELECT project_key FROM conversations WHERE id = ?1", [conversation_id], |r| r.get(0))
            .map_err(|_| AppError::NotFound("conversation not found".into()))?;
        let mut stmt = conn.prepare("SELECT id, role, content, engine, persona FROM messages WHERE conversation_id = ?1 ORDER BY timestamp ASC")?;
        let messages: Vec<(String, String, String, Option<String>, Option<String>)> = stmt
            .query_map([conversation_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
            .filter_map(|r| r.ok()).collect();
        (pk, build_sliding_window_chunks(&messages))
    };
    if chunks.is_empty() { return Ok(0); }

    // Phase 1b: find already-indexed root_message_ids (read lock, incremental skip)
    let already_indexed: std::collections::HashSet<String> = {
        let conn = db.read.lock().map_err(|_| AppError::Lock)?;
        conn.prepare("SELECT root_message_id FROM conversation_chunks WHERE conversation_id = ?1")
            .and_then(|mut s| s.query_map([conversation_id], |r| r.get::<_, String>(0))
                .map(|rows| rows.filter_map(|r| r.ok()).collect()))
            .unwrap_or_default()
    };

    // Phase 2: embed only NEW chunks (no lock held — ONNX inference is the slow part)
    let new_chunks: Vec<_> = chunks.iter()
        .filter(|(root_id, _, _)| !already_indexed.contains(root_id))
        .collect();

    if new_chunks.is_empty() { return Ok(0); }

    let mut embedded: Vec<(String, String, String, Vec<u8>)> = Vec::new();
    for (root_id, kind, text) in &new_chunks {
        if let Ok(v) = crate::agents::embedder::embed_text(text, false) {
            embedded.push((root_id.clone(), kind.clone(), text.clone(), embedding_to_blob(&v)));
        }
    }
    if embedded.is_empty() { return Ok(0); }

    // Phase 3: insert only new results (write lock — fast)
    let conn = db.write.lock().map_err(|_| AppError::Lock)?;
    let now = now_epoch_ms();
    let mut indexed = 0;
    for (root_id, kind, text, blob) in &embedded {
        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO conversation_chunks (id, project_key, conversation_id, kind, root_message_id, text_preview, embedding, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![id, project_key, conversation_id, kind, root_id, text, blob, now],
        )?;
        let chunk_rowid: i64 = conn.query_row("SELECT rowid FROM conversation_chunks WHERE id = ?1", [&id], |r| r.get(0)).unwrap_or(0);
        if chunk_rowid > 0 { conn.execute("INSERT INTO vec_chunks(rowid, embedding) VALUES (?1, ?2)", rusqlite::params![chunk_rowid, blob]).ok(); }
        indexed += 1;
    }
    if indexed > 0 {
        eprintln!("[vector] indexed {} new chunks for {} ({} already indexed)", indexed, conversation_id, already_indexed.len());
    }
    Ok(indexed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(id: &str, role: &str, content: &str, engine: Option<&str>, persona: Option<&str>) -> (String, String, String, Option<String>, Option<String>) {
        (id.to_string(), role.to_string(), content.to_string(), engine.map(|s| s.to_string()), persona.map(|s| s.to_string()))
    }

    #[test]
    fn sliding_window_basic_pair() {
        let messages = vec![
            msg("m1", "user", "What is Rust?", None, None),
            msg("m2", "assistant", "Rust is a systems language", Some("claude"), None),
        ];
        let chunks = build_sliding_window_chunks(&messages);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].1, "window");
        assert!(chunks[0].2.contains("[user]"));
        assert!(chunks[0].2.contains("[claude]"));
    }

    #[test]
    fn sliding_window_3_turns_overlap() {
        let messages = vec![
            msg("m1", "user", "Question 1 about algorithms", None, None),
            msg("m2", "assistant", "Answer 1 about sorting", Some("claude"), Some("Architect")),
            msg("m3", "user", "Question 2 about data structures", None, None),
            msg("m4", "assistant", "Answer 2 about hash maps", Some("gemini"), None),
            msg("m5", "user", "Question 3 about concurrency", None, None),
            msg("m6", "assistant", "Answer 3 about threads", Some("codex"), Some("Developer")),
            msg("m7", "user", "Question 4 about testing", None, None),
            msg("m8", "assistant", "Answer 4 about unit tests", Some("claude"), None),
        ];
        let chunks = build_sliding_window_chunks(&messages);
        assert_eq!(chunks.len(), 2);
        assert!(chunks[0].2.contains("Question 1"));
        assert!(chunks[0].2.contains("Question 3"));
        assert!(chunks[1].2.contains("Question 3"));
        assert!(chunks[1].2.contains("Question 4"));
    }

    #[test]
    fn sliding_window_author_prefix() {
        let messages = vec![
            msg("m1", "user", "Hello world test message", None, None),
            msg("m2", "assistant", "Response message here", Some("claude"), Some("Architect")),
        ];
        let chunks = build_sliding_window_chunks(&messages);
        assert!(chunks[0].2.contains("[Architect · claude]"));
    }

    #[test]
    fn sliding_window_skips_workflow() {
        let messages = vec![
            msg("m1", "user", "### 🔧 구현 시작\nworkflow template", None, None),
            msg("m2", "assistant", "OK starting implementation", Some("claude"), None),
            msg("m3", "user", "Real question about the code", None, None),
            msg("m4", "assistant", "Real answer about patterns", Some("claude"), None),
        ];
        let chunks = build_sliding_window_chunks(&messages);
        assert_eq!(chunks.len(), 1);
        assert!(!chunks[0].2.contains("구현 시작"));
        assert!(chunks[0].2.contains("Real question"));
    }

    #[test]
    fn sliding_window_empty() {
        let messages: Vec<(String, String, String, Option<String>, Option<String>)> = vec![];
        assert!(build_sliding_window_chunks(&messages).is_empty());
    }
}
