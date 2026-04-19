//! Vector similarity search — KNN via sqlite-vec and brute-force cosine fallback.

use rusqlite::Connection;

use crate::agents::{embedder, rawq};
use crate::errors::AppError;

use super::{VectorChunk, helpers::{embedding_to_blob, blob_to_embedding, truncate_str}};

/// Resolve the conversation's type for retrieval scoping ('main' / 'scratchpad').
/// Main chat and scratchpad are separate workspaces — automatic retrieval
/// should not cross the boundary (see context_queries.rs for rationale).
fn resolve_conv_type(conn: &Connection, conv_id: &str) -> String {
    conn.query_row(
        "SELECT COALESCE(type, 'main') FROM conversations WHERE id = ?1",
        [conv_id],
        |row| row.get(0),
    )
    .unwrap_or_else(|_| "main".into())
}

/// Search for similar chunks across a project using sqlite-vec KNN.
/// Falls back to brute-force cosine if vec0 table is unavailable.
///
/// Scoping: results are filtered to chunks whose conversation.type matches
/// the querying conversation's type. Main-chat queries never match scratchpad
/// chunks and vice versa. Cross-type retrieval remains available via explicit
/// `tool-request:sessions` markers.
pub fn search_similar(
    conn: &Connection,
    query_embedding: &[f32],
    project_key: &str,
    exclude_conv_id: &str,
    limit: usize,
) -> Vec<VectorChunk> {
    let query_blob = embedding_to_blob(query_embedding);
    let current_type = resolve_conv_type(conn, exclude_conv_id);

    // Try sqlite-vec KNN first (O(log n) with HNSW index)
    let results = search_via_vec0(conn, &query_blob, project_key, exclude_conv_id, &current_type, limit);
    if !results.is_empty() {
        return results;
    }

    // Fallback: brute-force cosine (for pre-migration databases)
    search_brute_force(conn, query_embedding, project_key, exclude_conv_id, &current_type, limit)
}

/// KNN search via sqlite-vec vec0 virtual table.
fn search_via_vec0(
    conn: &Connection,
    query_blob: &[u8],
    project_key: &str,
    exclude_conv_id: &str,
    conv_type: &str,
    limit: usize,
) -> Vec<VectorChunk> {
    let sql = "
        SELECT cc.id, cc.conversation_id, cc.kind, cc.text_preview, cc.root_message_id, vc.distance
        FROM vec_chunks vc
        JOIN conversation_chunks cc ON cc.rowid = vc.rowid
        JOIN conversations c ON c.id = cc.conversation_id
        WHERE vc.embedding MATCH ?1
          AND cc.project_key = ?2
          AND cc.conversation_id != ?3
          AND COALESCE(c.type, 'main') = ?4
          AND vc.k = ?5
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
        .query_map(rusqlite::params![query_blob, project_key, exclude_conv_id, conv_type, fetch_limit as i64], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?))
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();

    // Convert distance to similarity score (cosine distance: 0=identical, 2=opposite)
    let mut chunks: Vec<VectorChunk> = Vec::with_capacity(limit);
    for (id, conv_id, kind, text, root_msg_id, distance) in rows.into_iter().take(limit) {
        let score = 1.0 - (distance as f32 / 2.0);

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
    conv_type: &str,
    limit: usize,
) -> Vec<VectorChunk> {
    let mut stmt = match conn.prepare(
        "SELECT cc.id, cc.conversation_id, cc.kind, cc.text_preview, cc.embedding, cc.root_message_id
         FROM conversation_chunks cc
         JOIN conversations c ON c.id = cc.conversation_id
         WHERE cc.project_key = ?1
           AND cc.conversation_id != ?2
           AND COALESCE(c.type, 'main') = ?3
           AND cc.embedding IS NOT NULL",
    ) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let mut results: Vec<(VectorChunk, String)> = stmt
        .query_map(rusqlite::params![project_key, exclude_conv_id, conv_type], |row| {
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

// ─── Memory semantic search (within a single conversation) ────────────────────

/// A hit returned by `search_memory_semantic` — the semantic replacement for
/// substring-matching `list_memory_topics`. Unlike `VectorChunk`, this is
/// conversation-scoped and always kind='window' (v39 cleaned up legacy kinds).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySearchHit {
    pub chunk_id: String,
    pub text: String,
    pub score: f32,
    /// Timestamp of the root message that anchors this chunk (epoch ms).
    pub timestamp: Option<i64>,
}

/// Search for semantically similar chunks **within a single conversation**.
/// Restricted to kind='window' (sliding-window summaries of 3-turn context).
/// Tries vec0 KNN first; falls back to brute-force cosine on the conv's chunks.
pub fn search_within_conversation(
    conn: &Connection,
    query_embedding: &[f32],
    conversation_id: &str,
    limit: usize,
) -> Vec<MemorySearchHit> {
    let query_blob = embedding_to_blob(query_embedding);

    // Try sqlite-vec KNN first
    let vec0_sql = "
        SELECT cc.id, cc.text_preview, cc.root_message_id, vc.distance
        FROM vec_chunks vc
        JOIN conversation_chunks cc ON cc.rowid = vc.rowid
        WHERE vc.embedding MATCH ?1
          AND cc.conversation_id = ?2
          AND cc.kind = 'window'
          AND vc.k = ?3
        ORDER BY vc.distance
    ";
    let fetch_limit = limit.saturating_mul(2).max(limit);
    if let Ok(mut stmt) = conn.prepare(vec0_sql) {
        let rows: Vec<(String, String, String, f64)> = stmt
            .query_map(
                rusqlite::params![query_blob, conversation_id, fetch_limit as i64],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default();

        if !rows.is_empty() {
            let mut hits: Vec<MemorySearchHit> = Vec::with_capacity(limit);
            for (chunk_id, text, root_msg_id, distance) in rows.into_iter().take(limit) {
                let score = 1.0 - (distance as f32 / 2.0);
                let timestamp = conn
                    .query_row(
                        "SELECT timestamp FROM messages WHERE id = ?1",
                        [&root_msg_id],
                        |r| r.get::<_, i64>(0),
                    )
                    .ok();
                hits.push(MemorySearchHit { chunk_id, text, score, timestamp });
            }
            return hits;
        }
    }

    // Fallback: brute-force cosine on chunks of this conversation only
    let Ok(mut stmt) = conn.prepare(
        "SELECT cc.id, cc.text_preview, cc.embedding, cc.root_message_id
         FROM conversation_chunks cc
         WHERE cc.conversation_id = ?1
           AND cc.kind = 'window'
           AND cc.embedding IS NOT NULL",
    ) else { return Vec::new(); };

    let mut scored: Vec<(MemorySearchHit, String)> = stmt
        .query_map([conversation_id], |row| {
            let id: String = row.get(0)?;
            let text: String = row.get(1)?;
            let blob: Vec<u8> = row.get(2)?;
            let root_msg_id: String = row.get(3)?;
            Ok((id, text, blob, root_msg_id))
        })
        .map(|rows| {
            rows.filter_map(|r| r.ok())
                .filter_map(|(id, text, blob, root_msg_id)| {
                    let embedding = blob_to_embedding(&blob)?;
                    let score = crate::agents::rawq::cosine_similarity(query_embedding, &embedding);
                    Some((MemorySearchHit { chunk_id: id, text, score, timestamp: None }, root_msg_id))
                })
                .collect()
        })
        .unwrap_or_default();

    scored.sort_by(|a, b| b.0.score.partial_cmp(&a.0.score).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(limit);

    // Resolve timestamps for truncated results
    scored
        .into_iter()
        .map(|(mut hit, root_msg_id)| {
            hit.timestamp = conn
                .query_row(
                    "SELECT timestamp FROM messages WHERE id = ?1",
                    [&root_msg_id],
                    |r| r.get::<_, i64>(0),
                )
                .ok();
            hit
        })
        .collect()
}

/// Semantic memory search **within the current conversation**.
///
/// Supersedes the substring-matching path in `toolRequestHandler.memory:` which
/// filtered on `conversation_memory.topic.toLowerCase().includes(query)`. That
/// missed topically-related hits with different wording. This searches
/// `conversation_chunks` (kind='window') by embedding similarity via vec0 KNN.
///
/// Returns up to `limit` (default 3, clamp 1..=10) hits ordered by relevance.
#[tauri::command]
pub fn search_memory_semantic(
    conversation_id: String,
    query: String,
    limit: Option<i64>,
    state: tauri::State<crate::db::DbState>,
) -> Result<Vec<MemorySearchHit>, AppError> {
    let lim = limit.unwrap_or(3).clamp(1, 10) as usize;
    let query_embedding = embedder::embed_text(&query, true)?;
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    Ok(search_within_conversation(&conn, &query_embedding, &conversation_id, lim))
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::helpers::embedding_to_blob;

    const TEST_DIM: usize = 1024;

    fn fake_embedding(seed: usize) -> Vec<f32> {
        (0..TEST_DIM).map(|i| {
            let x = ((seed * 7919 + i * 104729) % 100000) as f32 / 100000.0;
            x * 2.0 - 1.0
        }).collect()
    }

    #[test]
    fn benchmark_brute_force_vs_vec0() {
        use std::time::Instant;
        use rusqlite::Connection;

        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
                sqlite_vec::sqlite3_vec_init as *const (),
            )));
        }

        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA journal_mode = WAL;").unwrap();

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
            CREATE TABLE conversations (
                id TEXT PRIMARY KEY,
                type TEXT DEFAULT 'main'
            );
            CREATE VIRTUAL TABLE vec_chunks USING vec0(
                embedding float[1024] distance_metric=cosine
            );
        ").unwrap();

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

        for i in 0..n {
            conn.execute(
                "INSERT INTO messages (id, conversation_id, role, content, timestamp) VALUES (?1, ?2, 'assistant', ?3, 0)",
                rusqlite::params![format!("msg-{}", i), format!("conv-{}", i % 100), format!("Full message content for chunk {}", i)],
            ).unwrap();
        }

        // Populate conversations with type='main' so the type-filter JOIN matches.
        for i in 0..100i32 {
            conn.execute(
                "INSERT OR IGNORE INTO conversations (id, type) VALUES (?1, 'main')",
                [format!("conv-{}", i)],
            ).unwrap();
        }

        let query = fake_embedding(42);
        let query_blob = embedding_to_blob(&query);

        let t_brute = Instant::now();
        let brute_results = search_brute_force(&conn, &query, "proj1", "conv-999", "main", 10);
        let brute_ms = t_brute.elapsed().as_micros();

        let t_vec0 = Instant::now();
        let vec0_results = search_via_vec0(&conn, &query_blob, "proj1", "conv-999", "main", 10);
        let vec0_ms = t_vec0.elapsed().as_micros();

        eprintln!("[bench] {} chunks:", n);
        eprintln!("  brute-force: {}μs ({} results)", brute_ms, brute_results.len());
        eprintln!("  vec0 KNN:    {}μs ({} results)", vec0_ms, vec0_results.len());
        eprintln!("  speedup:     {:.1}x", brute_ms as f64 / vec0_ms.max(1) as f64);

        assert!(!brute_results.is_empty());
        assert!(!vec0_results.is_empty());
        assert!(brute_results[0].full_text.is_some());
        assert!(vec0_results[0].full_text.is_some());
    }

    /// Vector retrieval must scope by conversation.type — main-chat queries
    /// should not pull scratchpad chunks back and vice versa. Session 2026-04-18 s37.
    #[test]
    fn type_filter_separates_main_and_scratchpad_in_brute_force() {
        use rusqlite::Connection;
        let conn = Connection::open_in_memory().unwrap();
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
            CREATE TABLE conversations (
                id TEXT PRIMARY KEY,
                type TEXT DEFAULT 'main'
            );
        ").unwrap();

        // Two conversations: main-A and scratchpad-B, both in project P.
        conn.execute("INSERT INTO conversations (id, type) VALUES ('main-A', 'main')", []).unwrap();
        conn.execute("INSERT INTO conversations (id, type) VALUES ('scratch-B', 'scratchpad')", []).unwrap();

        let emb = fake_embedding(100);
        let blob = embedding_to_blob(&emb);

        // Chunk in main-A (matches query seed)
        conn.execute("INSERT INTO messages (id, conversation_id, role, content, timestamp) VALUES ('m1','main-A','assistant','main text',0)", []).unwrap();
        conn.execute(
            "INSERT INTO conversation_chunks (id, project_key, conversation_id, kind, root_message_id, text_preview, embedding, created_at)
             VALUES ('c1','P','main-A','pair','m1','main preview',?1,0)",
            [&blob],
        ).unwrap();

        // Chunk in scratchpad-B (same embedding, so similarity score would be high if retrieved)
        conn.execute("INSERT INTO messages (id, conversation_id, role, content, timestamp) VALUES ('m2','scratch-B','assistant','scratch text',0)", []).unwrap();
        conn.execute(
            "INSERT INTO conversation_chunks (id, project_key, conversation_id, kind, root_message_id, text_preview, embedding, created_at)
             VALUES ('c2','P','scratch-B','pair','m2','scratch preview',?1,0)",
            [&blob],
        ).unwrap();

        // Query from a different main conv — should ONLY return main-A, not scratchpad-B.
        conn.execute("INSERT INTO conversations (id, type) VALUES ('main-C', 'main')", []).unwrap();
        let main_results = search_brute_force(&conn, &emb, "P", "main-C", "main", 10);
        assert_eq!(main_results.len(), 1);
        assert_eq!(main_results[0].conversation_id, "main-A");

        // Query from a different scratchpad conv — should ONLY return scratch-B.
        conn.execute("INSERT INTO conversations (id, type) VALUES ('scratch-D', 'scratchpad')", []).unwrap();
        let scratch_results = search_brute_force(&conn, &emb, "P", "scratch-D", "scratchpad", 10);
        assert_eq!(scratch_results.len(), 1);
        assert_eq!(scratch_results[0].conversation_id, "scratch-B");
    }

    #[test]
    fn search_within_conversation_scopes_to_target_conv_only() {
        use rusqlite::Connection;
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("
            CREATE TABLE conversation_chunks (
                id TEXT PRIMARY KEY,
                project_key TEXT NOT NULL,
                conversation_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                root_message_id TEXT NOT NULL,
                text_preview TEXT NOT NULL,
                embedding BLOB,
                created_at INTEGER NOT NULL
            );
            CREATE TABLE messages (
                id TEXT PRIMARY KEY, conversation_id TEXT, role TEXT, content TEXT, timestamp INTEGER
            );
        ").unwrap();

        let emb_a = fake_embedding(10);
        let emb_b = fake_embedding(20);
        let emb_noise = fake_embedding(999);
        let blob_a = embedding_to_blob(&emb_a);
        let blob_b = embedding_to_blob(&emb_b);
        let blob_n = embedding_to_blob(&emb_noise);

        // target conv (A): 1 window chunk matching query, 1 noise window chunk
        conn.execute("INSERT INTO messages(id,conversation_id,role,content,timestamp) VALUES('mA1','A','assistant','a1',100)", []).unwrap();
        conn.execute("INSERT INTO messages(id,conversation_id,role,content,timestamp) VALUES('mA2','A','assistant','a2',200)", []).unwrap();
        conn.execute(
            "INSERT INTO conversation_chunks(id,project_key,conversation_id,kind,root_message_id,text_preview,embedding,created_at)
             VALUES('cA1','P','A','window','mA1','A1 matching text',?1,0)",
            [&blob_a],
        ).unwrap();
        conn.execute(
            "INSERT INTO conversation_chunks(id,project_key,conversation_id,kind,root_message_id,text_preview,embedding,created_at)
             VALUES('cA2','P','A','window','mA2','A2 noise text',?1,0)",
            [&blob_n],
        ).unwrap();

        // stale anchor/pair in target conv — must be filtered out by kind='window' clause
        conn.execute(
            "INSERT INTO conversation_chunks(id,project_key,conversation_id,kind,root_message_id,text_preview,embedding,created_at)
             VALUES('cA3','P','A','pair','mA1','A3 legacy pair',?1,0)",
            [&blob_a],
        ).unwrap();

        // other conv (B): matching embedding, must NOT be returned
        conn.execute("INSERT INTO messages(id,conversation_id,role,content,timestamp) VALUES('mB1','B','assistant','b1',300)", []).unwrap();
        conn.execute(
            "INSERT INTO conversation_chunks(id,project_key,conversation_id,kind,root_message_id,text_preview,embedding,created_at)
             VALUES('cB1','P','B','window','mB1','B1 matching text',?1,0)",
            [&blob_b],
        ).unwrap();

        // Query close to emb_a (in target conv A). Expect A1 first, A2 second, no B, no pair.
        let hits = search_within_conversation(&conn, &emb_a, "A", 5);
        assert!(!hits.is_empty(), "should find at least one hit in conv A");
        assert_eq!(hits[0].chunk_id, "cA1", "highest similarity chunk is cA1");
        assert!(!hits.iter().any(|h| h.chunk_id == "cB1"), "must not cross conversation boundary");
        assert!(!hits.iter().any(|h| h.chunk_id == "cA3"), "must not return kind='pair' legacy rows");
        // Timestamp resolved from root message
        assert_eq!(hits[0].timestamp, Some(100));
    }

    #[test]
    fn resolve_conv_type_defaults_to_main_when_missing() {
        use rusqlite::Connection;
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE conversations (id TEXT PRIMARY KEY, type TEXT);").unwrap();
        // Missing row → default 'main'
        assert_eq!(resolve_conv_type(&conn, "nonexistent"), "main");
        // NULL type → default 'main' via COALESCE
        conn.execute("INSERT INTO conversations (id, type) VALUES ('legacy', NULL)", []).unwrap();
        assert_eq!(resolve_conv_type(&conn, "legacy"), "main");
        // Explicit type
        conn.execute("INSERT INTO conversations (id, type) VALUES ('sc', 'scratchpad')", []).unwrap();
        assert_eq!(resolve_conv_type(&conn, "sc"), "scratchpad");
    }
}
