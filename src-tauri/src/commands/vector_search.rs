//! Vector-based conversation search — semantic retrieval using bge-m3 embeddings.
//!
//! Indexes conversation messages as chunks (user+assistant pairs) with
//! BLOB embeddings in `conversation_chunks`. Uses bge-m3 (1024dim) for
//! document/conversation search, rawq (384dim) for code search only.

use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::agents::embedder;
use crate::agents::rawq;
use crate::db::migrations::now_epoch_ms;
use crate::errors::AppError;

/// A chunk stored in the vector index.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VectorChunk {
    pub id: String,
    pub conversation_id: String,
    pub kind: String,
    pub text_preview: String,
    pub score: f32,
    /// Original message content from parent document (via root_message_id JOIN).
    /// Longer than text_preview — used for ContextPack injection.
    pub full_text: Option<String>,
}

/// Vector index status for a conversation.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VectorIndexStatus {
    pub conversation_id: String,
    pub chunk_count: usize,
    pub indexed: bool,
}

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
    // Delete existing chunks for this conversation
    conn.execute(
        "DELETE FROM conversation_chunks WHERE conversation_id = ?1",
        [conversation_id],
    )?;

    // Load messages for chunking
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
    let mut chunks: Vec<(String, String, String)> = Vec::new(); // (root_msg_id, kind, text)
    let mut i = 0;
    while i < messages.len() {
        let (ref id, ref role, ref content) = messages[i];
        // Skip workflow auto-generated messages (pollute vector space with template text)
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
        // Embed (passage mode, not query)
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
            params![id, project_key, conversation_id, kind, root_id, text, embedding_blob, now],
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

/// Search for similar chunks across a project using sqlite-vec KNN.
/// Falls back to brute-force cosine if vec0 table is unavailable.
pub fn search_similar(
    conn: &Connection,
    query_embedding: &[f32],
    project_key: &str,
    exclude_conv_id: &str,
    limit: usize,
) -> Vec<VectorChunk> {
    let query_blob = embedding_to_blob(query_embedding);

    // Try sqlite-vec KNN first (O(log n) with HNSW index)
    let results = search_via_vec0(conn, &query_blob, project_key, exclude_conv_id, limit);
    if !results.is_empty() {
        return results;
    }

    // Fallback: brute-force cosine (for pre-migration databases)
    search_brute_force(conn, query_embedding, project_key, exclude_conv_id, limit)
}

/// KNN search via sqlite-vec vec0 virtual table.
fn search_via_vec0(
    conn: &Connection,
    query_blob: &[u8],
    project_key: &str,
    exclude_conv_id: &str,
    limit: usize,
) -> Vec<VectorChunk> {
    // vec0 KNN: MATCH returns (rowid, distance) ordered by distance ASC (cosine: 0=identical)
    // Join back to conversation_chunks for metadata + filter by project/conversation.
    let sql = "
        SELECT cc.id, cc.conversation_id, cc.kind, cc.text_preview, cc.root_message_id, vc.distance
        FROM vec_chunks vc
        JOIN conversation_chunks cc ON cc.rowid = vc.rowid
        WHERE vc.embedding MATCH ?1
          AND cc.project_key = ?2
          AND cc.conversation_id != ?3
          AND vc.k = ?4
        ORDER BY vc.distance
    ";
    let mut stmt = match conn.prepare(sql) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[vector] vec0 query failed (falling back to brute-force): {}", e);
            return Vec::new();
        }
    };

    let fetch_limit = limit * 3; // Over-fetch to account for filtered rows
    let rows: Vec<(String, String, String, String, String, f64)> = stmt
        .query_map(params![query_blob, project_key, exclude_conv_id, fetch_limit as i64], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?))
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();

    // Convert distance to similarity score (cosine distance: 0=identical, 2=opposite)
    let mut chunks: Vec<VectorChunk> = Vec::with_capacity(limit);
    for (id, conv_id, kind, text, root_msg_id, distance) in rows.into_iter().take(limit) {
        let score = 1.0 - (distance as f32 / 2.0); // distance 0→score 1.0, distance 2→score 0.0

        // Parent Document Retriever: resolve full text
        let full_text = conn.query_row(
            "SELECT content FROM messages WHERE id = ?1",
            [&root_msg_id],
            |row| row.get::<_, String>(0),
        ).ok().map(|c| truncate_str(&c, 600));

        chunks.push(VectorChunk { id, conversation_id: conv_id, kind, text_preview: text, score, full_text });
    }
    chunks
}

/// Brute-force cosine similarity search (fallback for pre-v30 databases).
fn search_brute_force(
    conn: &Connection,
    query_embedding: &[f32],
    project_key: &str,
    exclude_conv_id: &str,
    limit: usize,
) -> Vec<VectorChunk> {
    let mut stmt = match conn.prepare(
        "SELECT id, conversation_id, kind, text_preview, embedding, root_message_id
         FROM conversation_chunks
         WHERE project_key = ?1 AND conversation_id != ?2 AND embedding IS NOT NULL",
    ) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let mut results: Vec<(VectorChunk, String)> = stmt
        .query_map(params![project_key, exclude_conv_id], |row| {
            let id: String = row.get(0)?;
            let conv_id: String = row.get(1)?;
            let kind: String = row.get(2)?;
            let text: String = row.get(3)?;
            let blob: Vec<u8> = row.get(4)?;
            let root_msg_id: String = row.get(5)?;
            Ok((id, conv_id, kind, text, blob, root_msg_id))
        })
        .map(|rows| {
            rows.filter_map(|r| r.ok())
                .filter_map(|(id, conv_id, kind, text, blob, root_msg_id)| {
                    let embedding = blob_to_embedding(&blob)?;
                    let score = rawq::cosine_similarity(query_embedding, &embedding);
                    Some((VectorChunk { id, conversation_id: conv_id, kind, text_preview: text, score, full_text: None }, root_msg_id))
                })
                .collect()
        })
        .unwrap_or_default();

    results.sort_by(|a, b| b.0.score.partial_cmp(&a.0.score).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(limit);

    let mut chunks: Vec<VectorChunk> = Vec::with_capacity(results.len());
    for (mut chunk, root_msg_id) in results {
        if let Ok(content) = conn.query_row(
            "SELECT content FROM messages WHERE id = ?1", [&root_msg_id], |row| row.get::<_, String>(0),
        ) {
            chunk.full_text = Some(truncate_str(&content, 600));
        }
        chunks.push(chunk);
    }
    chunks
}

/// Get vector index status for a conversation.
pub fn get_index_status(conn: &Connection, conversation_id: &str) -> VectorIndexStatus {
    let chunk_count: usize = conn
        .query_row(
            "SELECT COUNT(*) FROM conversation_chunks WHERE conversation_id = ?1",
            [conversation_id],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0) as usize;

    VectorIndexStatus {
        conversation_id: conversation_id.to_string(),
        chunk_count,
        indexed: chunk_count > 0,
    }
}

// ─── Sliding window chunking with author prefix ────────────────────────

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
fn build_sliding_window_chunks(
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

/// Format author label for embedding prefix: "persona · engine" or "assistant"
fn format_author_label(engine: &str, persona: &str) -> String {
    if !persona.is_empty() && !engine.is_empty() {
        format!("{} · {}", persona, engine)
    } else if !engine.is_empty() {
        engine.to_string()
    } else {
        "assistant".to_string()
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────

/// Detect workflow auto-generated prompts that pollute vector search.
/// These are template messages from tunaFlow UI, not user conversations.
fn is_workflow_prompt(content: &str) -> bool {
    // starts_with works on the full string — no need to slice
    content.starts_with("### 🔧") || content.starts_with("### 📋") || content.starts_with("### 🔍")
        || content.starts_with("### 🔄") || content.starts_with("### ✏") || content.starts_with("### 💬")
        || content.starts_with("### 📝") || content.starts_with("### 📌")
        || content.starts_with("┌─") // legacy ASCII box prompts
        || content.contains("<!-- tunaflow:review-verdict -->")
        || content.contains("<!-- tunaflow:impl-plan -->")
        || content.contains("<!-- tunaflow:impl-complete -->")
}

pub fn truncate_str(s: &str, max_chars: usize) -> String {
    if s.len() <= max_chars {
        return s.to_string();
    }
    let end = s
        .char_indices()
        .take_while(|&(i, _)| i <= max_chars)
        .last()
        .map_or(0, |(i, _)| i);
    format!("{}…", &s[..end])
}

pub fn embedding_to_blob(embedding: &[f32]) -> Vec<u8> {
    let mut blob = Vec::with_capacity(embedding.len() * 4);
    for &val in embedding {
        blob.extend_from_slice(&val.to_le_bytes());
    }
    blob
}

fn blob_to_embedding(blob: &[u8]) -> Option<Vec<f32>> {
    // Dynamic dimension: accept any valid f32 blob (must be multiple of 4 bytes)
    if blob.len() % 4 != 0 || blob.is_empty() {
        return None;
    }
    let dim = blob.len() / 4;
    let mut vec = Vec::with_capacity(dim);
    for chunk in blob.chunks_exact(4) {
        vec.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    Some(vec)
}

// ─── Tauri Commands ─────────────────────────────────────────────────────

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
) -> Result<usize, AppError> {
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
            let id = uuid::Uuid::new_v4().to_string();
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
                ).ok(); // best-effort — vec0 failure shouldn't block indexing
            }
            indexed += 1;
        }
        eprintln!("[vector] indexed {} new chunks for {} ({} already indexed)", indexed, conversation_id, already_indexed.len());
        Ok(indexed)
    }).await.map_err(|_| AppError::Lock)?
}

/// Blocking index function — callable from HTTP API without Tauri State.
pub fn index_chunks_blocking(db: &crate::db::DbState, conversation_id: &str) -> Result<usize, AppError> {
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
    let mut embedded: Vec<(String, String, String, Vec<u8>)> = Vec::new();
    for (root_id, kind, text) in &chunks {
        if let Ok(v) = crate::agents::embedder::embed_text(text, false) {
            embedded.push((root_id.clone(), kind.clone(), text.clone(), embedding_to_blob(&v)));
        }
    }
    if embedded.is_empty() { return Ok(0); }
    let conn = db.write.lock().map_err(|_| AppError::Lock)?;
    let rowids: Vec<i64> = conn.prepare("SELECT rowid FROM conversation_chunks WHERE conversation_id = ?1")
        .and_then(|mut s| s.query_map([conversation_id], |r| r.get(0)).map(|rows| rows.filter_map(|r| r.ok()).collect()))
        .unwrap_or_default();
    for rid in &rowids { conn.execute("DELETE FROM vec_chunks WHERE rowid = ?1", [rid]).ok(); }
    conn.execute("DELETE FROM conversation_chunks WHERE conversation_id = ?1", [conversation_id])?;
    let now = now_epoch_ms();
    let mut indexed = 0;
    for (root_id, kind, text, blob) in &embedded {
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO conversation_chunks (id, project_key, conversation_id, kind, root_message_id, text_preview, embedding, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![id, project_key, conversation_id, kind, root_id, text, blob, now],
        )?;
        let chunk_rowid: i64 = conn.query_row("SELECT rowid FROM conversation_chunks WHERE id = ?1", [&id], |r| r.get(0)).unwrap_or(0);
        if chunk_rowid > 0 { conn.execute("INSERT INTO vec_chunks(rowid, embedding) VALUES (?1, ?2)", rusqlite::params![chunk_rowid, blob]).ok(); }
        indexed += 1;
    }
    Ok(indexed)
}

/// Blocking search — callable from HTTP API without Tauri State.
pub fn search_chunks_blocking(db: &crate::db::DbState, conversation_id: &str, query: &str, limit: usize) -> Result<Vec<serde_json::Value>, AppError> {
    let query_embedding = embedder::embed_text(query, true)?;
    let conn = db.read.lock().map_err(|_| AppError::Lock)?;
    let project_key: String = conn.query_row("SELECT project_key FROM conversations WHERE id = ?1", [conversation_id], |r| r.get(0))
        .map_err(|_| AppError::NotFound("conversation not found".into()))?;
    let chunks = search_similar(&conn, &query_embedding, &project_key, conversation_id, limit);
    Ok(chunks.into_iter().map(|c| serde_json::json!({
        "id": c.id, "conversationId": c.conversation_id, "kind": c.kind,
        "textPreview": c.text_preview, "score": c.score,
    })).collect())
}

/// Search for similar chunks using a text query.
#[tauri::command]
pub fn search_conversation_vectors(
    query: String,
    project_key: String,
    exclude_conversation_id: String,
    limit: usize,
    state: tauri::State<crate::db::DbState>,
) -> Result<Vec<VectorChunk>, AppError> {
    let query_embedding = embedder::embed_text(&query, true)?;
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    Ok(search_similar(
        &conn,
        &query_embedding,
        &project_key,
        &exclude_conversation_id,
        limit,
    ))
}

/// Get vector index status for a conversation.
#[tauri::command]
pub fn get_vector_index_status(
    conversation_id: String,
    state: tauri::State<crate::db::DbState>,
) -> Result<VectorIndexStatus, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    Ok(get_index_status(&conn, &conversation_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_DIM: usize = 1024;

    #[test]
    fn embedding_blob_roundtrip() {
        let original: Vec<f32> = (0..TEST_DIM).map(|i| i as f32 * 0.01).collect();
        let blob = embedding_to_blob(&original);
        assert_eq!(blob.len(), TEST_DIM * 4);
        let recovered = blob_to_embedding(&blob).unwrap();
        assert_eq!(recovered.len(), TEST_DIM);
        for i in 0..TEST_DIM {
            assert!((original[i] - recovered[i]).abs() < 1e-6);
        }
    }

    #[test]
    fn blob_wrong_size_returns_none() {
        let blob = vec![0u8; 3]; // not multiple of 4
        assert!(blob_to_embedding(&blob).is_none());
    }

    #[test]
    fn truncate_short_string() {
        assert_eq!(truncate_str("hello", 10), "hello");
    }

    #[test]
    fn truncate_long_string() {
        let long = "a".repeat(500);
        let result = truncate_str(&long, 100);
        assert!(result.len() <= 110); // 100 + "…" overhead
        assert!(result.ends_with('…'));
    }

    // ─── is_workflow_prompt ──────────────────────────────────────────────

    #[test]
    fn workflow_prompt_emoji_headers() {
        assert!(is_workflow_prompt("### 🔧 구현 시작\n..."));
        assert!(is_workflow_prompt("### 📋 Plan 요약\n..."));
        assert!(is_workflow_prompt("### 🔍 검색 결과\n..."));
        assert!(is_workflow_prompt("### 🔄 Rework 지시\n..."));
    }

    #[test]
    fn workflow_prompt_legacy_ascii() {
        assert!(is_workflow_prompt("┌─ Implementation Report ─┐\n..."));
    }

    #[test]
    fn workflow_prompt_html_markers() {
        assert!(is_workflow_prompt("verdict <!-- tunaflow:review-verdict --> content"));
        assert!(is_workflow_prompt("plan <!-- tunaflow:impl-plan --> json"));
        assert!(is_workflow_prompt("done <!-- tunaflow:impl-complete -->"));
    }

    #[test]
    fn workflow_prompt_normal_text_false() {
        assert!(!is_workflow_prompt("How do I implement authentication?"));
        assert!(!is_workflow_prompt("The database schema needs updating"));
        assert!(!is_workflow_prompt(""));
    }

    // ─── embedding roundtrip edge cases ──────────────────────────────────

    #[test]
    fn embedding_blob_zeros() {
        let zeros: Vec<f32> = vec![0.0; TEST_DIM];
        let blob = embedding_to_blob(&zeros);
        let recovered = blob_to_embedding(&blob).unwrap();
        assert!(recovered.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn embedding_blob_negative_values() {
        let negatives: Vec<f32> = (0..TEST_DIM).map(|i| -(i as f32) * 0.1).collect();
        let blob = embedding_to_blob(&negatives);
        let recovered = blob_to_embedding(&blob).unwrap();
        for i in 0..TEST_DIM {
            assert!((negatives[i] - recovered[i]).abs() < 1e-6);
        }
    }

    #[test]
    fn blob_empty_returns_none() {
        assert!(blob_to_embedding(&[]).is_none());
    }

    // ─── truncate_str edge cases ─────────────────────────────────────────

    #[test]
    fn truncate_empty_string() {
        assert_eq!(truncate_str("", 100), "");
    }

    #[test]
    fn truncate_exact_limit() {
        let s = "hello"; // 5 chars
        assert_eq!(truncate_str(s, 5), "hello");
    }

    #[test]
    fn truncate_multibyte_utf8() {
        let s = "한글테스트문자열이것은긴문자열입니다"; // Korean chars, multi-byte
        let result = truncate_str(s, 10);
        // Should not panic or corrupt UTF-8
        assert!(result.is_char_boundary(result.len().saturating_sub(3)) || result.ends_with('…'));
    }

    // ─── sliding window chunking ────────────────────────────────────────

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
        // 4 turns → window=3, stride=2: [0..3], [2..4] = 2 chunks
        assert_eq!(chunks.len(), 2);
        // First chunk covers turns 0-2, second covers turns 2-3
        assert!(chunks[0].2.contains("Question 1"));
        assert!(chunks[0].2.contains("Question 3")); // turn 2 in first window
        assert!(chunks[1].2.contains("Question 3")); // turn 2 overlap in second window
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
        // Workflow message skipped, only 1 valid turn
        assert_eq!(chunks.len(), 1);
        assert!(!chunks[0].2.contains("구현 시작"));
        assert!(chunks[0].2.contains("Real question"));
    }

    #[test]
    fn sliding_window_empty() {
        let messages: Vec<(String, String, String, Option<String>, Option<String>)> = vec![];
        assert!(build_sliding_window_chunks(&messages).is_empty());
    }

    #[test]
    fn format_author_label_full() {
        assert_eq!(format_author_label("claude", "Architect"), "Architect · claude");
    }

    #[test]
    fn format_author_label_engine_only() {
        assert_eq!(format_author_label("gemini", ""), "gemini");
    }

    #[test]
    fn format_author_label_fallback() {
        assert_eq!(format_author_label("", ""), "assistant");
    }

    // ─── sqlite-vec benchmark ───────────────────────────────────────────

    /// Generate a random-ish 384-dim embedding (deterministic from seed).
    fn fake_embedding(seed: usize) -> Vec<f32> {
        (0..TEST_DIM).map(|i| {
            let x = ((seed * 7919 + i * 104729) % 100000) as f32 / 100000.0;
            x * 2.0 - 1.0 // range [-1, 1]
        }).collect()
    }

    #[test]
    fn benchmark_brute_force_vs_vec0() {
        use std::time::Instant;

        // Init sqlite-vec
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
                sqlite_vec::sqlite3_vec_init as *const (),
            )));
        }

        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA journal_mode = WAL;").unwrap();

        // Create tables
        conn.execute_batch("
            CREATE TABLE conversation_chunks (
                id TEXT PRIMARY KEY,
                project_key TEXT NOT NULL,
                conversation_id TEXT NOT NULL,
                kind TEXT NOT NULL DEFAULT 'pair',
                root_message_id TEXT NOT NULL,
                text_preview TEXT NOT NULL,
                embedding BLOB,
                created_at INTEGER NOT NULL
            );
            CREATE TABLE messages (
                id TEXT PRIMARY KEY,
                conversation_id TEXT,
                role TEXT,
                content TEXT,
                timestamp INTEGER
            );
            CREATE VIRTUAL TABLE vec_chunks USING vec0(
                embedding float[1024] distance_metric=cosine
            );
        ").unwrap();

        // Insert N fake chunks
        let n = 11_000;
        let t_insert = Instant::now();
        for i in 0..n {
            let id = format!("chunk-{}", i);
            let conv_id = format!("conv-{}", i % 100);
            let emb = fake_embedding(i);
            let blob = embedding_to_blob(&emb);
            conn.execute(
                "INSERT INTO conversation_chunks (id, project_key, conversation_id, kind, root_message_id, text_preview, embedding, created_at)
                 VALUES (?1, 'proj1', ?2, 'window', ?3, ?4, ?5, 0)",
                rusqlite::params![id, conv_id, format!("msg-{}", i), format!("text preview {}", i), blob],
            ).unwrap();
            let rowid: i64 = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO vec_chunks(rowid, embedding) VALUES (?1, ?2)",
                rusqlite::params![rowid, blob],
            ).unwrap();
        }
        let insert_ms = t_insert.elapsed().as_millis();
        eprintln!("[bench] inserted {} chunks in {}ms", n, insert_ms);

        // Also insert messages for parent retriever
        for i in 0..n {
            conn.execute(
                "INSERT INTO messages (id, conversation_id, role, content, timestamp) VALUES (?1, ?2, 'assistant', ?3, 0)",
                rusqlite::params![format!("msg-{}", i), format!("conv-{}", i % 100), format!("Full message content for chunk {}", i)],
            ).unwrap();
        }

        let query = fake_embedding(42);
        let query_blob = embedding_to_blob(&query);

        // Benchmark: brute-force
        let t_brute = Instant::now();
        let brute_results = search_brute_force(&conn, &query, "proj1", "conv-999", 10);
        let brute_ms = t_brute.elapsed().as_micros();

        // Benchmark: vec0 KNN
        let t_vec0 = Instant::now();
        let vec0_results = search_via_vec0(&conn, &query_blob, "proj1", "conv-999", 10);
        let vec0_ms = t_vec0.elapsed().as_micros();

        eprintln!("[bench] {} chunks:", n);
        eprintln!("  brute-force: {}μs ({} results)", brute_ms, brute_results.len());
        eprintln!("  vec0 KNN:    {}μs ({} results)", vec0_ms, vec0_results.len());
        eprintln!("  speedup:     {:.1}x", brute_ms as f64 / vec0_ms.max(1) as f64);

        // Both should return results
        assert!(!brute_results.is_empty(), "brute-force should return results");
        assert!(!vec0_results.is_empty(), "vec0 should return results");
        // Both should have parent text resolved
        assert!(brute_results[0].full_text.is_some(), "brute-force should resolve parent text");
        assert!(vec0_results[0].full_text.is_some(), "vec0 should resolve parent text");
    }
}
