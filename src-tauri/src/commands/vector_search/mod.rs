//! Vector-based conversation search — semantic retrieval using bge-m3 embeddings.
//!
//! Indexes conversation messages as chunks (user+assistant pairs) with
//! BLOB embeddings in `conversation_chunks`. Uses bge-m3 (1024dim) for
//! document/conversation search, rawq (384dim) for code search only.
//!
//! Sub-modules:
//! - `helpers`: truncate_str, embedding_to_blob, blob_to_embedding, content classifiers
//! - `index`: sliding-window chunking + index_conversation_chunks (Tauri command)
//! - `query`: search_similar, vec0 KNN, brute-force, search_conversation_vectors, search_memory_semantic (Tauri commands)

mod backfill;
mod helpers;
mod index;
mod query;

pub use backfill::spawn_startup_backfill;

// Re-export all public and proc-macro generated symbols so callers
// use `commands::vector_search::*` unchanged.
pub use helpers::*;
pub use index::*;
pub use query::*;

use rusqlite::Connection;
use crate::errors::AppError;

// ─── Shared types ─────────────────────────────────────────────────────────────

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

// ─── Status query ─────────────────────────────────────────────────────────────

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

/// Get vector index status for a conversation.
#[tauri::command]
pub fn get_vector_index_status(
    conversation_id: String,
    state: tauri::State<crate::db::DbState>,
) -> Result<VectorIndexStatus, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    Ok(get_index_status(&conn, &conversation_id))
}
