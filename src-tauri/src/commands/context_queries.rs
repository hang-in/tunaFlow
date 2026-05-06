//! Common DB query helpers shared across context-domain commands
//! (agents, plans, memos, artifacts, roundtable).

use rusqlite::{params, Connection};

/// Load the N most recent messages from a conversation in chronological order.
/// Returns `(role, content)` pairs.
pub fn load_recent_messages(
    conn: &Connection,
    conversation_id: &str,
    limit: i64,
) -> Vec<(String, String)> {
    load_recent_messages_with_author(conn, conversation_id, limit)
        .into_iter()
        .map(|(role, content, _, _)| (role, content))
        .collect()
}

/// Load recent messages with author metadata.
/// Returns `(role, content, engine, persona)` tuples.
///
/// devbug #263 Task 03 — RT 메시지 (rt_round_index IS NOT NULL) 도 그대로 포함.
/// single agent dispatch 가 RT 라운드 안에 들어가지 않은 *순수 main conv* 메시지
/// 만 받고 싶다면 [`load_recent_messages_excluding_rt`] 사용.
pub fn load_recent_messages_with_author(
    conn: &Connection,
    conversation_id: &str,
    limit: i64,
) -> Vec<(String, String, Option<String>, Option<String>)> {
    let Ok(mut stmt) = conn.prepare(
        "SELECT role, content, engine, persona FROM messages
         WHERE conversation_id = ?1
         ORDER BY timestamp DESC LIMIT ?2",
    ) else {
        return Vec::new();
    };
    let mut rows: Vec<(String, String, Option<String>, Option<String>)> = stmt
        .query_map(params![conversation_id, limit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })
        .map(|mapped| mapped.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();
    rows.reverse();
    rows
}

/// Load recent messages excluding RT round messages.
///
/// devbug #263 Task 03 — single agent dispatch (main conv 의 *모든 에이전트
/// 선택 해제* 후 단일 질의) 시점에 ContextPack 이 RT round transcript 를
/// *주제별 컨텍스트* 로 prepend 하지 않게 사용.
///
/// `rt_round_index IS NULL` row 만 collect — 컬럼 자체가 없는 v50 이전
/// fixture 에서도 query 가 깨지지 않게 fallback (호출 측이 v51+ 가정).
///
/// INV-RTC-7/8 (RT 미사용 영향 0): RT 한번도 안 쓴 conv 는 모든 row 의
/// rt_round_index = NULL → 출력이 `load_recent_messages_with_author` 와 동일.
pub fn load_recent_messages_excluding_rt(
    conn: &Connection,
    conversation_id: &str,
    limit: i64,
) -> Vec<(String, String, Option<String>, Option<String>)> {
    let Ok(mut stmt) = conn.prepare(
        "SELECT role, content, engine, persona FROM messages
         WHERE conversation_id = ?1 AND rt_round_index IS NULL
         ORDER BY timestamp DESC LIMIT ?2",
    ) else {
        // 컬럼 부재 (구 fixture) 시 기존 동작으로 fallback
        return load_recent_messages_with_author(conn, conversation_id, limit);
    };
    let mut rows: Vec<(String, String, Option<String>, Option<String>)> = stmt
        .query_map(params![conversation_id, limit], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })
        .map(|mapped| mapped.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();
    rows.reverse();
    rows
}

/// Load the display label for a conversation (custom_label if set, otherwise label).
pub fn conversation_label(conn: &Connection, conversation_id: &str) -> Option<String> {
    conn.query_row(
        "SELECT COALESCE(custom_label, label) FROM conversations WHERE id = ?1",
        [conversation_id],
        |row| row.get(0),
    )
    .ok()
}

/// Load the anchor (checkpoint) message for a branch conversation.
///
/// Given a branch shadow conversation id (format `branch:{branch_id}`),
/// looks up the branch's checkpoint_id and returns `(role, content)` of that message.
pub fn load_anchor_message(conn: &Connection, branch_conv_id: &str) -> Option<(String, String)> {
    if !branch_conv_id.starts_with("branch:") {
        return None;
    }
    let branch_id = &branch_conv_id["branch:".len()..];
    let checkpoint_id: String = conn
        .query_row(
            "SELECT checkpoint_id FROM branches WHERE id = ?1",
            [branch_id],
            |row| row.get(0),
        )
        .ok()?;
    conn.query_row(
        "SELECT role, content FROM messages WHERE id = ?1",
        [&checkpoint_id],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    )
    .ok()
}

/// Load the parent conversation id for a branch shadow conversation.
pub fn parent_conversation_id(conn: &Connection, branch_conv_id: &str) -> Option<String> {
    conn.query_row(
        "SELECT parent_id FROM conversations WHERE id = ?1",
        [branch_conv_id],
        |row| row.get(0),
    )
    .ok()
    .flatten()
}

/// A retrieved conversation chunk — pair (user+assistant), anchor, or brief.
pub struct RetrievedChunk {
    pub kind: &'static str,  // "pair", "anchor", "brief"
    pub messages: Vec<(String, String, Option<String>, Option<String>)>, // (role, content, engine, persona)
    pub conversation_id: String,
    pub score: f64,
    pub timestamp: i64,
}

/// Retrieve past conversation chunks with scoring, dedup, and overlap suppression.
///
/// Pipeline: FTS5 broad search → chunk assembly → scoring → Jaccard dedup → top-N trim.
/// `existing_context` is concatenated text from sections already in ContextPack (for overlap penalty).
#[allow(dead_code)]
pub fn retrieve_relevant_chunks(
    conn: &Connection,
    project_key: &str,
    _current_conversation_id: &str,
    query: &str,
    recent_message_ids: &[String],
    limit: i64,
) -> Vec<RetrievedChunk> {
    retrieve_relevant_chunks_with_overlap(conn, project_key, _current_conversation_id, query, recent_message_ids, limit, None)
}

pub fn retrieve_relevant_chunks_with_overlap(
    conn: &Connection,
    project_key: &str,
    _current_conversation_id: &str,
    query: &str,
    recent_message_ids: &[String],
    limit: i64,
    existing_context: Option<&str>,
) -> Vec<RetrievedChunk> {
    let fts_query = build_fts_query(query);
    if fts_query.is_empty() {
        return Vec::new();
    }

    // Resolve the current conversation's type ('main' / 'scratchpad' / …).
    // Scratchpad and main chat are semantically different workspaces — a
    // main-chat user asking "what did I discuss" should NOT get scratchpad
    // snippets back (pollution) and vice versa. Branch shadow convs inherit
    // their parent's type. NULL defaults to 'main' for legacy rows.
    // Explicit cross-type retrieval is still available through the
    // `tool-request:sessions` marker, which bypasses this filter.
    let current_type: String = conn.query_row(
        "SELECT COALESCE(type, 'main') FROM conversations WHERE id = ?1",
        [_current_conversation_id],
        |row| row.get(0),
    ).unwrap_or_else(|_| "main".into());

    // Step 1: Broad FTS5 search (fetch more than needed for ranking headroom).
    // Exclude current conversation — its recent context is already loaded via
    // load_recent_messages_with_author(). Including it here re-surfaces old messages
    // from the same thread and pollutes ContextPack with stale topic (e.g. earlier
    // experiments leaking into current task). Branch shadow convs (branch:UUID) are
    // likewise excluded — the parent thread is handled separately.
    let fetch_count = (limit * 4).max(20);
    let Ok(mut stmt) = conn.prepare(
        "SELECT m.id, m.role, m.conversation_id, m.timestamp, rank
         FROM messages_fts fts
         JOIN messages m ON m.rowid = fts.rowid
         JOIN conversations c ON c.id = m.conversation_id
         WHERE messages_fts MATCH ?1
           AND c.project_key = ?2
           AND m.conversation_id != ?3
           AND COALESCE(c.type, 'main') = ?4
         ORDER BY rank
         LIMIT ?5",
    ) else {
        return Vec::new();
    };

    let hits: Vec<(String, String, String, i64, f64)> = stmt
        .query_map(params![fts_query, project_key, _current_conversation_id, current_type, fetch_count], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?))
        })
        .map(|mapped| mapped.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();

    // Step 2: Assemble chunks with initial scoring
    let mut seen_chunks: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut chunks: Vec<RetrievedChunk> = Vec::new();
    let now_ts = crate::db::migrations::now_epoch_ms();

    for (hit_id, hit_role, conv_id, hit_ts, fts_rank) in &hits {
        if recent_message_ids.contains(hit_id) {
            continue;
        }

        let chunk_key = format!("{}:{}", conv_id, hit_ts);
        if seen_chunks.contains(&chunk_key) {
            continue;
        }

        let chunk = if conv_id.starts_with("branch:") {
            let is_brief: bool = conn.query_row(
                "SELECT COUNT(*) > 0 FROM memos WHERE message_id = ?1 AND type = 'roundtable_brief'",
                [hit_id], |row| row.get(0),
            ).unwrap_or(false);
            if is_brief {
                build_single_chunk(conn, hit_id, "brief")
            } else {
                build_pair_chunk(conn, conv_id, hit_id, hit_role, *hit_ts)
            }
        } else {
            build_pair_chunk(conn, conv_id, hit_id, hit_role, *hit_ts)
        };

        if let Some(mut c) = chunk {
            c.conversation_id = conv_id.clone();
            c.timestamp = *hit_ts;

            // Scoring: combine FTS rank + recency + kind bonus + query coverage.
            let fts_score = (-fts_rank).max(0.0).min(10.0) / 10.0; // normalize 0-1
            let age_hours = ((now_ts - hit_ts) as f64 / 3_600_000.0).max(0.0);
            // Decay over 2 days (not weeks) — users working actively on a project
            // see "last week" as old context, and 168h decay made 3-day-old chunks
            // still score ~0.69, effectively no recency signal.
            let recency_score = 1.0 / (1.0 + age_hours / 48.0);
            let kind_bonus = match c.kind {
                "pair" => 0.25,  // full Q&A is most useful
                "brief" => 0.2,  // RT briefs are curated
                "anchor" => 0.15, // anchors provide context
                _ => 0.0,
            };

            // Query coverage bonus (D): FTS5 with OR'd terms hits any chunk containing
            // ANY query word. A chunk that contains MULTIPLE query words is usually far
            // more relevant than one that repeats a single common word. Rerank by how
            // many meaningful query terms are actually present in the chunk text.
            let coverage_bonus = {
                let chunk_text_lower: String = c.messages.iter()
                    .map(|(_, content, _, _)| content.to_lowercase())
                    .collect::<Vec<_>>().join(" ");
                let query_terms: Vec<String> = fts_query
                    .split(" OR ")
                    .map(|w| w.trim().to_lowercase())
                    .filter(|w| !w.is_empty())
                    .collect();
                if query_terms.is_empty() {
                    0.0
                } else {
                    let hit_count = query_terms.iter()
                        .filter(|t| chunk_text_lower.contains(t.as_str()))
                        .count();
                    let ratio = hit_count as f64 / query_terms.len() as f64;
                    // 0 terms: 0.0, all terms: +0.3, smooth scaling
                    ratio * 0.3
                }
            };

            // Overlap penalty: if chunk content strongly overlaps existing context
            // Uses stopword-filtered Jaccard to reduce false positives from common vocabulary
            let overlap_penalty = if let Some(existing) = existing_context {
                let chunk_text: String = c.messages.iter().map(|(_, content, _, _)| content.as_str()).collect::<Vec<_>>().join(" ");
                let sim = jaccard_word_similarity_filtered(&chunk_text, existing);
                if sim > 0.75 { 0.4 } else if sim > 0.5 { 0.15 } else { 0.0 }
            } else {
                0.0
            };

            // Weight shift (A): recency 0.2 → 0.4. FTS alone dominated and old but
            // keyword-heavy messages beat recent-but-quiet ones.
            c.score = fts_score * 0.5 + recency_score * 0.4 + kind_bonus + coverage_bonus - overlap_penalty;

            seen_chunks.insert(chunk_key);
            chunks.push(c);
        }
    }

    // Step 3: Jaccard dedup — remove near-duplicate chunks
    let mut deduped: Vec<RetrievedChunk> = Vec::new();
    for chunk in chunks {
        let chunk_text: String = chunk.messages.iter().map(|(_, c, _, _)| c.as_str()).collect::<Vec<_>>().join(" ");
        let is_duplicate = deduped.iter().any(|existing| {
            let existing_text: String = existing.messages.iter().map(|(_, c, _, _)| c.as_str()).collect::<Vec<_>>().join(" ");
            jaccard_word_similarity(&chunk_text, &existing_text) > 0.7
        });
        if !is_duplicate {
            deduped.push(chunk);
        }
    }

    // Step 4: Sort by score descending, trim to limit
    deduped.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    deduped.truncate(limit as usize);

    deduped
}

/// Word-level Jaccard similarity (raw, for dedup).
fn jaccard_word_similarity(a: &str, b: &str) -> f64 {
    let words_a: std::collections::HashSet<&str> = a.split_whitespace().collect();
    let words_b: std::collections::HashSet<&str> = b.split_whitespace().collect();
    if words_a.is_empty() && words_b.is_empty() { return 1.0; }
    let intersection = words_a.intersection(&words_b).count();
    let union = words_a.union(&words_b).count();
    if union == 0 { return 0.0; }
    intersection as f64 / union as f64
}

/// Stopword-filtered Jaccard — reduces false positives from common vocabulary.
fn jaccard_word_similarity_filtered(a: &str, b: &str) -> f64 {
    let words_a: std::collections::HashSet<String> = a.split_whitespace()
        .map(|w| w.to_lowercase())
        .filter(|w| w.len() >= 3 && !STOPWORDS.contains(&w.as_str()))
        .collect();
    let words_b: std::collections::HashSet<String> = b.split_whitespace()
        .map(|w| w.to_lowercase())
        .filter(|w| w.len() >= 3 && !STOPWORDS.contains(&w.as_str()))
        .collect();
    if words_a.is_empty() && words_b.is_empty() { return 0.0; }
    let intersection = words_a.intersection(&words_b).count();
    let union = words_a.union(&words_b).count();
    if union == 0 { return 0.0; }
    intersection as f64 / union as f64
}

/// Build a user+assistant pair chunk around a hit message.
fn build_pair_chunk(
    conn: &Connection,
    conv_id: &str,
    hit_id: &str,
    hit_role: &str,
    hit_ts: i64,
) -> Option<RetrievedChunk> {
    // If hit is user, get the next assistant; if assistant, get the previous user
    let (user_msg, asst_msg) = if hit_role == "user" {
        let user = load_message_by_id(conn, hit_id)?;
        let asst = conn.query_row(
            "SELECT id, role, content, engine, persona FROM messages
             WHERE conversation_id = ?1 AND role = 'assistant' AND timestamp > ?2
             ORDER BY timestamp ASC LIMIT 1",
            params![conv_id, hit_ts],
            |row| Ok((row.get::<_,String>(0)?, row.get::<_,String>(1)?, row.get::<_,String>(2)?, row.get::<_,Option<String>>(3)?, row.get::<_,Option<String>>(4)?)),
        ).ok();
        (Some(user), asst)
    } else {
        let asst = load_message_by_id(conn, hit_id)?;
        let user = conn.query_row(
            "SELECT id, role, content, engine, persona FROM messages
             WHERE conversation_id = ?1 AND role = 'user' AND timestamp <= ?2
             ORDER BY timestamp DESC LIMIT 1",
            params![conv_id, hit_ts],
            |row| Ok((row.get::<_,String>(0)?, row.get::<_,String>(1)?, row.get::<_,String>(2)?, row.get::<_,Option<String>>(3)?, row.get::<_,Option<String>>(4)?)),
        ).ok();
        (user, Some(asst))
    };

    let mut messages = Vec::new();
    if let Some((_, role, content, engine, persona)) = user_msg {
        messages.push((role, content, engine, persona));
    }
    if let Some((_, role, content, engine, persona)) = asst_msg {
        messages.push((role, content, engine, persona));
    }

    if messages.is_empty() { return None; }

    Some(RetrievedChunk {
        kind: "pair",
        messages,
        conversation_id: String::new(),
        score: 0.0,
        timestamp: 0,
    })
}

/// Build a single-message chunk (anchor or brief).
fn build_single_chunk(conn: &Connection, msg_id: &str, kind: &'static str) -> Option<RetrievedChunk> {
    let msg = load_message_by_id(conn, msg_id)?;
    let (_, role, content, engine, persona) = msg;
    Some(RetrievedChunk {
        kind,
        messages: vec![(role, content, engine, persona)],
        conversation_id: String::new(),
        score: 0.0,
        timestamp: 0,
    })
}

fn load_message_by_id(conn: &Connection, id: &str) -> Option<(String, String, String, Option<String>, Option<String>)> {
    conn.query_row(
        "SELECT id, role, content, engine, persona FROM messages WHERE id = ?1",
        [id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
    ).ok()
}

/// Common stopwords that inflate FTS5 results without adding relevance.
const STOPWORDS: &[&str] = &[
    // English
    "the", "is", "are", "was", "were", "be", "been", "being",
    "have", "has", "had", "do", "does", "did", "will", "would", "could", "should",
    "can", "may", "might", "shall", "must",
    "it", "its", "he", "she", "we", "they", "you", "me", "him", "her", "us", "them",
    "this", "that", "these", "those", "what", "which", "who", "whom", "how", "when", "where", "why",
    "if", "or", "and", "but", "not", "no", "so", "as", "at", "by", "for", "in", "of", "on", "to", "up",
    "an", "my", "our", "your", "all", "any", "each", "some", "such",
    "than", "too", "very", "just", "also", "more", "most", "only", "even",
    "with", "from", "into", "about", "after", "before", "between", "through",
    // Korean particles/endings (common short words that match everything)
    "이", "그", "저", "것", "수", "등", "및", "또", "더",
];

/// Build FTS5 query from natural language.
/// Strips punctuation/quotes, filters stopwords, extracts meaningful words, joins with OR.
fn build_fts_query(query: &str) -> String {
    // Strip characters that FTS5 interprets as operators or syntax
    let cleaned: String = query.chars()
        .map(|c| if c == '"' || c == '\'' || c == '(' || c == ')' || c == '*' || c == '?' || c == '!' || c == '.' || c == ',' || c == ':' || c == ';' {
            ' '
        } else { c })
        .collect();
    let words: Vec<&str> = cleaned.split_whitespace()
        .filter(|w| w.len() >= 2)
        .filter(|w| !STOPWORDS.contains(&w.to_lowercase().as_str()))
        .take(8)
        .collect();
    if words.is_empty() {
        let fallback: Vec<&str> = cleaned.split_whitespace()
            .filter(|w| w.len() >= 3)
            .take(4)
            .collect();
        if fallback.is_empty() { return String::new(); }
        return fallback.join(" OR ");
    }
    words.join(" OR ")
}

/// Load the project_key for a conversation, then resolve the project path.
pub fn project_path_for_conversation(conn: &Connection, conversation_id: &str) -> Option<String> {
    let project_key: String = conn
        .query_row(
            "SELECT project_key FROM conversations WHERE id = ?1",
            [conversation_id],
            |row| row.get(0),
        )
        .ok()?;
    conn.query_row(
        "SELECT path FROM projects WHERE key = ?1",
        [&project_key],
        |row| row.get(0),
    )
    .ok()
    .flatten()
}

// ─── RT marker filter tests (devbug #263 Task 03) ──────────────────────────
#[cfg(test)]
mod rt_filter_tests {
    use super::*;

    fn build_db_with_messages() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE messages (
                id              TEXT    PRIMARY KEY,
                conversation_id TEXT    NOT NULL,
                role            TEXT    NOT NULL,
                content         TEXT    NOT NULL,
                timestamp       INTEGER NOT NULL,
                status          TEXT    NOT NULL DEFAULT 'done',
                progress_content TEXT,
                engine          TEXT,
                model           TEXT,
                persona         TEXT,
                rt_round_index  INTEGER
             );
             -- Main conv 일반 user/assistant 메시지
             INSERT INTO messages (id, conversation_id, role, content, timestamp, status, engine, persona, rt_round_index)
             VALUES
                ('m1', 'conv-1', 'user',      'main user msg 1',           100, 'done', NULL,         NULL,         NULL),
                ('m2', 'conv-1', 'assistant', 'main assistant reply',      200, 'done', 'claude',     NULL,         NULL),
                -- RT 라운드 1 헤더 + 참여자 응답
                ('m3', 'conv-1', 'assistant', '--- Round 1 ... ---',       300, 'done', 'system',     NULL,         1),
                ('m4', 'conv-1', 'assistant', 'rt round 1 claude reply',   400, 'done', 'claude-code','claude',     1),
                ('m5', 'conv-1', 'assistant', 'rt round 1 codex reply',    500, 'done', 'codex',      'codex',      1),
                -- 라운드 사이 단일 follow-up (RT 메시지 아님)
                ('m6', 'conv-1', 'user',      'follow-up: clarify X',      600, 'done', NULL,         NULL,         NULL),
                ('m7', 'conv-1', 'assistant', 'single agent answer',       700, 'done', 'claude',     NULL,         NULL);",
        ).unwrap();
        conn
    }

    /// Single agent dispatch 시 RT 라운드 메시지가 transcript 에서 제외 — Plan
    /// §3 Task 03 의 시나리오 A 회복 핵심 가드.
    #[test]
    fn single_agent_dispatch_skips_rt_transcript() {
        let conn = build_db_with_messages();
        let rows = load_recent_messages_excluding_rt(&conn, "conv-1", 20);

        // RT 라운드 1 메시지 (m3, m4, m5) 모두 제외
        let contents: Vec<&str> = rows.iter().map(|(_, c, _, _)| c.as_str()).collect();
        assert!(!contents.iter().any(|c| c.contains("Round 1")));
        assert!(!contents.iter().any(|c| c.contains("rt round 1 claude")));
        assert!(!contents.iter().any(|c| c.contains("rt round 1 codex")));

        // 일반 main conv 메시지는 그대로 — m1, m2, m6, m7 4건
        assert_eq!(rows.len(), 4);
        assert!(contents.contains(&"main user msg 1"));
        assert!(contents.contains(&"main assistant reply"));
        assert!(contents.contains(&"follow-up: clarify X"));
        assert!(contents.contains(&"single agent answer"));
    }

    /// 기존 helper 는 *모든* 메시지 그대로 — RT 본 흐름 (transcript 조립) 영역
    /// 의 동작 보존.
    #[test]
    fn legacy_loader_includes_rt_messages() {
        let conn = build_db_with_messages();
        let rows = load_recent_messages_with_author(&conn, "conv-1", 20);
        assert_eq!(rows.len(), 7, "기존 helper 는 모든 메시지 포함");
    }

    /// RT 미사용 conv (rt_round_index 모두 NULL) 의 두 helper 결과 동일 —
    /// INV-RTC-7/8 fast path 검증.
    #[test]
    fn no_rt_usage_yields_identical_results() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE messages (
                id              TEXT    PRIMARY KEY,
                conversation_id TEXT    NOT NULL,
                role            TEXT    NOT NULL,
                content         TEXT    NOT NULL,
                timestamp       INTEGER NOT NULL,
                status          TEXT    NOT NULL DEFAULT 'done',
                progress_content TEXT,
                engine          TEXT,
                model           TEXT,
                persona         TEXT,
                rt_round_index  INTEGER
             );
             INSERT INTO messages (id, conversation_id, role, content, timestamp, status, engine, persona, rt_round_index)
             VALUES
                ('m1', 'conv-x', 'user',      'msg 1', 100, 'done', NULL,     NULL, NULL),
                ('m2', 'conv-x', 'assistant', 'msg 2', 200, 'done', 'claude', NULL, NULL);",
        )
        .unwrap();

        let with_rt    = load_recent_messages_with_author(&conn, "conv-x", 20);
        let without_rt = load_recent_messages_excluding_rt(&conn, "conv-x", 20);
        assert_eq!(with_rt, without_rt);
    }

    /// 컬럼 부재 fixture 에서도 panic 없이 fallback (구 schema 호환).
    #[test]
    fn falls_back_when_rt_round_index_column_missing() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE messages (
                id              TEXT    PRIMARY KEY,
                conversation_id TEXT    NOT NULL,
                role            TEXT    NOT NULL,
                content         TEXT    NOT NULL,
                timestamp       INTEGER NOT NULL,
                status          TEXT    NOT NULL DEFAULT 'done',
                progress_content TEXT,
                engine          TEXT,
                model           TEXT,
                persona         TEXT
             );
             INSERT INTO messages (id, conversation_id, role, content, timestamp, status)
             VALUES ('m1', 'conv-y', 'user', 'msg', 100, 'done');",
        )
        .unwrap();
        // 컬럼 부재 → fallback 으로 일반 helper 결과
        let rows = load_recent_messages_excluding_rt(&conn, "conv-y", 20);
        assert_eq!(rows.len(), 1);
    }
}
