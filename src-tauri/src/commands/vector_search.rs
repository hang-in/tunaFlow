//! Vector-based conversation search — semantic retrieval using rawq embeddings.
//!
//! Indexes conversation messages as chunks (user+assistant pairs) with
//! BLOB embeddings in `conversation_chunks`. Provides brute-force cosine
//! similarity search for semantic retrieval beyond FTS5 keyword matching.

use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::agents::rawq::{self, EMBED_DIM};
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
        let embedding = match rawq::embed_text(text, false) {
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

/// Search for similar chunks across a project using brute-force cosine similarity.
pub fn search_similar(
    conn: &Connection,
    query_embedding: &[f32],
    project_key: &str,
    exclude_conv_id: &str,
    limit: usize,
) -> Vec<VectorChunk> {
    // Load all chunk embeddings for the project (excluding the current conversation)
    let mut stmt = match conn.prepare(
        "SELECT id, conversation_id, kind, text_preview, embedding
         FROM conversation_chunks
         WHERE project_key = ?1 AND conversation_id != ?2 AND embedding IS NOT NULL",
    ) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let mut results: Vec<VectorChunk> = stmt
        .query_map(params![project_key, exclude_conv_id], |row| {
            let id: String = row.get(0)?;
            let conv_id: String = row.get(1)?;
            let kind: String = row.get(2)?;
            let text: String = row.get(3)?;
            let blob: Vec<u8> = row.get(4)?;
            Ok((id, conv_id, kind, text, blob))
        })
        .map(|rows| {
            rows.filter_map(|r| r.ok())
                .filter_map(|(id, conv_id, kind, text, blob)| {
                    let embedding = blob_to_embedding(&blob)?;
                    let score = rawq::cosine_similarity(query_embedding, &embedding);
                    Some(VectorChunk {
                        id,
                        conversation_id: conv_id,
                        kind,
                        text_preview: text,
                        score,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    // Sort by score descending, take top N
    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(limit);
    results
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

fn truncate_str(s: &str, max_chars: usize) -> String {
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

fn embedding_to_blob(embedding: &[f32]) -> Vec<u8> {
    let mut blob = Vec::with_capacity(embedding.len() * 4);
    for &val in embedding {
        blob.extend_from_slice(&val.to_le_bytes());
    }
    blob
}

fn blob_to_embedding(blob: &[u8]) -> Option<Vec<f32>> {
    if blob.len() != EMBED_DIM * 4 {
        return None;
    }
    let mut vec = Vec::with_capacity(EMBED_DIM);
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
                "SELECT id, role, content FROM messages WHERE conversation_id = ?1 ORDER BY timestamp ASC",
            )?;
            let messages: Vec<(String, String, String)> = stmt
                .query_map([&conversation_id], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                })?
                .filter_map(|r| r.ok())
                .collect();

            let mut chunk_texts: Vec<(String, String, String)> = Vec::new();
            let mut i = 0;
            while i < messages.len() {
                let (ref id, ref role, ref content) = messages[i];
                if is_workflow_prompt(content) { i += 1; continue; }
                if role == "user" && i + 1 < messages.len() && messages[i + 1].1 == "assistant" {
                    let text = format!("Q: {}\nA: {}", truncate_str(content, 200), truncate_str(&messages[i + 1].2, 200));
                    chunk_texts.push((id.clone(), "pair".to_string(), text));
                    i += 2;
                } else {
                    let text = truncate_str(content, 300);
                    if text.len() >= 20 { chunk_texts.push((id.clone(), "anchor".to_string(), text)); }
                    i += 1;
                }
            }
            (pk, chunk_texts)
            // Read lock released here
        };

        if chunks.is_empty() { return Ok(0); }

        // Phase 2: embed (NO lock held — rawq is external process, can be slow)
        let mut embedded: Vec<(String, String, String, Vec<u8>)> = Vec::new();
        for (root_id, kind, text) in &chunks {
            match rawq::embed_text(text, false) {
                Ok(v) => {
                    embedded.push((root_id.clone(), kind.clone(), text.clone(), embedding_to_blob(&v)));
                }
                Err(e) => {
                    eprintln!("[vector] embed failed for chunk {}: {:?}", root_id, e);
                }
            }
        }

        if embedded.is_empty() { return Ok(0); }

        // Phase 3: write results (short lock)
        let conn = db.write.lock().map_err(|_| AppError::Lock)?;
        conn.execute("DELETE FROM conversation_chunks WHERE conversation_id = ?1", [&conversation_id])?;
        let now = now_epoch_ms();
        let mut indexed = 0;
        for (root_id, kind, text, blob) in &embedded {
            let id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO conversation_chunks (id, project_key, conversation_id, kind, root_message_id, text_preview, embedding, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![id, project_key, conversation_id, kind, root_id, text, blob, now],
            )?;
            indexed += 1;
        }
        eprintln!("[vector] indexed {} chunks for {} (from {} texts)", indexed, conversation_id, chunks.len());
        Ok(indexed)
    }).await.map_err(|_| AppError::Lock)?
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
    let query_embedding = rawq::embed_text(&query, true)
        .map_err(|e| AppError::Agent(format!("embed failed: {:?}", e)))?;
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

    #[test]
    fn embedding_blob_roundtrip() {
        let original: Vec<f32> = (0..EMBED_DIM).map(|i| i as f32 * 0.01).collect();
        let blob = embedding_to_blob(&original);
        assert_eq!(blob.len(), EMBED_DIM * 4);
        let recovered = blob_to_embedding(&blob).unwrap();
        assert_eq!(recovered.len(), EMBED_DIM);
        for i in 0..EMBED_DIM {
            assert!((original[i] - recovered[i]).abs() < 1e-6);
        }
    }

    #[test]
    fn blob_wrong_size_returns_none() {
        let blob = vec![0u8; 100]; // wrong size
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
        let zeros: Vec<f32> = vec![0.0; EMBED_DIM];
        let blob = embedding_to_blob(&zeros);
        let recovered = blob_to_embedding(&blob).unwrap();
        assert!(recovered.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn embedding_blob_negative_values() {
        let negatives: Vec<f32> = (0..EMBED_DIM).map(|i| -(i as f32) * 0.1).collect();
        let blob = embedding_to_blob(&negatives);
        let recovered = blob_to_embedding(&blob).unwrap();
        for i in 0..EMBED_DIM {
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
}
