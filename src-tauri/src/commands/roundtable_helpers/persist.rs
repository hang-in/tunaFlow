use rusqlite::params;
use uuid::Uuid;

use crate::commands::agents_helpers::trace_log::{insert_trace_log, SpanInfo, new_span_id};
use crate::db::migrations::now_epoch_ms;
use crate::db::models::Message;
use crate::errors::AppError;

use super::executor::ParticipantResult;

/// Persist a round header (system message) and return it.
///
/// 기존 시그니처 보존 — `rt_round_index` NULL. RT 라운드 진행 안에서 호출
/// 되는 callsite 는 `persist_header_with_round` 사용 (devbug #263 Task 03).
pub fn persist_header(
    conn: &rusqlite::Connection,
    conversation_id: &str,
    text: &str,
) -> Result<Message, AppError> {
    persist_header_with_round(conn, conversation_id, text, None)
}

/// Round-index aware variant — Task 03 path. RT round 헤더 / synthesizer
/// 안내 메시지는 round_index 기록 → ContextPack 의 single agent dispatch 가
/// 그 메시지를 *raw transcript* 로 prepend 하지 않게 helper 가 필터.
pub fn persist_header_with_round(
    conn: &rusqlite::Connection,
    conversation_id: &str,
    text: &str,
    round_index: Option<u32>,
) -> Result<Message, AppError> {
    let id = Uuid::new_v4().to_string();
    let now = now_epoch_ms();
    conn.execute(
        "INSERT INTO messages (id, conversation_id, role, content, timestamp, status, engine, rt_round_index)
         VALUES (?1, ?2, 'assistant', ?3, ?4, 'done', 'system', ?5)",
        params![id, conversation_id, text, now, round_index.map(|r| r as i64)],
    )?;
    Ok(Message {
        id,
        conversation_id: conversation_id.to_string(),
        role: "assistant".into(),
        content: text.to_string(),
        timestamp: now,
        status: "done".into(),
        progress_content: None,
        engine: Some("system".into()),
        model: None,
        persona: None,
        duration_ms: None, input_tokens: None, output_tokens: None, cost_usd: None,
    })
}

/// Persist a streaming-start placeholder — empty message with status='streaming'.
///
/// 기존 시그니처 보존 — `rt_round_index` NULL. RT 라운드 안에서 참여자
/// 메시지 저장 시 `persist_streaming_start_with_round` 사용 (Task 03).
pub fn persist_streaming_start(
    conn: &rusqlite::Connection,
    conversation_id: &str,
    name: &str,
    engine_label: &str,
    model: Option<&str>,
    sources_json: &str,
) -> Result<Message, AppError> {
    persist_streaming_start_with_round(
        conn,
        conversation_id,
        name,
        engine_label,
        model,
        sources_json,
        None,
    )
}

/// Round-index aware variant — Task 03 path. 참여자 메시지에 rt_round_index
/// 기록 → main conv 의 단일 dispatch 가 ContextPack 에서 RT 메시지 분리.
pub fn persist_streaming_start_with_round(
    conn: &rusqlite::Connection,
    conversation_id: &str,
    name: &str,
    engine_label: &str,
    model: Option<&str>,
    sources_json: &str,
    round_index: Option<u32>,
) -> Result<Message, AppError> {
    let msg_id = Uuid::new_v4().to_string();
    let now = now_epoch_ms();
    let progress = if sources_json.is_empty() { None } else { Some(sources_json) };

    conn.execute(
        "INSERT INTO messages
         (id, conversation_id, role, content, timestamp, status, progress_content, engine, model, persona, rt_round_index)
         VALUES (?1, ?2, 'assistant', '', ?3, 'streaming', ?4, ?5, ?6, ?7, ?8)",
        params![msg_id, conversation_id, now, progress, engine_label, model, name, round_index.map(|r| r as i64)],
    )?;

    Ok(Message {
        id: msg_id,
        conversation_id: conversation_id.to_string(),
        role: "assistant".into(),
        content: String::new(),
        timestamp: now,
        status: "streaming".into(),
        progress_content: if sources_json.is_empty() { None } else { Some(sources_json.to_string()) },
        engine: Some(engine_label.to_string()),
        model: model.map(|s| s.to_string()),
        persona: Some(name.to_string()),
        duration_ms: None, input_tokens: None, output_tokens: None, cost_usd: None,
    })
}

/// Finalize a streaming message — UPDATE content, status, and write usage + trace log.
pub fn persist_streaming_done(
    conn: &rusqlite::Connection,
    conversation_id: &str,
    msg_id: &str,
    r: &ParticipantResult,
    trace_id: &str,
    root_span_id: &str,
) -> Result<Message, AppError> {
    let now = now_epoch_ms();

    conn.execute(
        "UPDATE messages SET content = ?1, status = ?2, timestamp = ?3 WHERE id = ?4",
        params![r.content, r.status, now, msg_id],
    )?;

    conn.execute(
        "UPDATE conversations SET
             total_input_tokens  = total_input_tokens  + ?1,
             total_output_tokens = total_output_tokens + ?2,
             total_cost_usd      = total_cost_usd      + ?3,
             updated_at          = ?4
         WHERE id = ?5",
        params![r.in_tokens, r.out_tokens, r.cost_usd, now / 1000, conversation_id],
    )?;

    insert_trace_log(conn, conversation_id, r.in_tokens, r.out_tokens, r.cost_usd, now, &SpanInfo {
        trace_id,
        span_id: new_span_id(),
        parent_span_id: Some(root_span_id),
        operation: "roundtable.participant",
        engine: &r.engine,
        duration_ms: 0,
        status: if r.status == "done" { "ok" } else { "error" },
    });

    Ok(Message {
        id: msg_id.to_string(),
        conversation_id: conversation_id.to_string(),
        role: "assistant".into(),
        content: r.content.clone(),
        timestamp: now,
        status: r.status.clone(),
        progress_content: if r.prompt_sources.is_empty() { None } else { Some(r.prompt_sources.clone()) },
        engine: Some(r.engine.clone()),
        model: r.model.clone(),
        persona: Some(r.name.clone()),
        duration_ms: None, input_tokens: None, output_tokens: None, cost_usd: None,
    })
}

/// Persist a single participant result, update conversation usage, and write trace log.
/// `trace_id` / `root_span_id` are passed from the roundtable command for parent linkage.
/// Retained for non-streaming fallback paths.
#[allow(dead_code)]
pub fn persist_single(
    conn: &rusqlite::Connection,
    conversation_id: &str,
    r: &ParticipantResult,
    trace_id: &str,
    root_span_id: &str,
) -> Result<Message, AppError> {
    let msg_id = Uuid::new_v4().to_string();
    let now = now_epoch_ms();
    let progress = if r.prompt_sources.is_empty() {
        None
    } else {
        Some(r.prompt_sources.as_str())
    };

    conn.execute(
        "INSERT INTO messages
         (id, conversation_id, role, content, timestamp, status, progress_content, engine, model, persona)
         VALUES (?1, ?2, 'assistant', ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            msg_id,
            conversation_id,
            r.content,
            now,
            r.status,
            progress,
            r.engine,
            r.model,
            r.name,
        ],
    )?;

    conn.execute(
        "UPDATE conversations SET
             total_input_tokens  = total_input_tokens  + ?1,
             total_output_tokens = total_output_tokens + ?2,
             total_cost_usd      = total_cost_usd      + ?3,
             updated_at          = ?4
         WHERE id = ?5",
        params![r.in_tokens, r.out_tokens, r.cost_usd, now / 1000, conversation_id],
    )?;

    insert_trace_log(conn, conversation_id, r.in_tokens, r.out_tokens, r.cost_usd, now, &SpanInfo {
        trace_id,
        span_id: new_span_id(),
        parent_span_id: Some(root_span_id),
        operation: "roundtable.participant",
        engine: &r.engine,
        duration_ms: 0, // per-participant timing not tracked at persist level
        status: if r.status == "done" { "ok" } else { "error" },
    });

    Ok(Message {
        id: msg_id,
        conversation_id: conversation_id.to_string(),
        role: "assistant".into(),
        content: r.content.clone(),
        timestamp: now,
        status: r.status.clone(),
        progress_content: if r.prompt_sources.is_empty() {
            None
        } else {
            Some(r.prompt_sources.clone())
        },
        engine: Some(r.engine.clone()),
        model: r.model.clone(),
        persona: Some(r.name.clone()),
        duration_ms: None, input_tokens: None, output_tokens: None, cost_usd: None,
    })
}

/// Archive the RT transcript into the memos table.
pub fn archive_transcript(
    conn: &rusqlite::Connection,
    conversation_id: &str,
    topic: &str,
    transcript: &[(String, String)],
    rounds: u32,
    rt_mode: &str,
) -> Result<(), AppError> {
    if transcript.is_empty() {
        return Ok(());
    }

    let project_key: String = conn
        .query_row(
            "SELECT project_key FROM conversations WHERE id = ?1",
            [conversation_id],
            |row| row.get(0),
        )
        .map_err(|_| AppError::NotFound("conversation not found for archive".into()))?;

    let transcript_text: String = transcript
        .iter()
        .map(|(name, content)| format!("**[{}]**:\n{}", name, content))
        .collect::<Vec<_>>()
        .join("\n\n");

    let mut seen = std::collections::HashSet::new();
    let unique_names: Vec<&str> = transcript
        .iter()
        .map(|(n, _)| n.as_str())
        .filter(|n| seen.insert(*n))
        .collect();

    let content = format!(
        "# Roundtable Archive\n\n\
         **Topic:** {}\n\
         **Mode:** {}\n\
         **Rounds:** {}\n\
         **Participants:** {}\n\n\
         ---\n\n\
         {}",
        topic,
        rt_mode,
        rounds,
        unique_names.join(", "),
        transcript_text,
    );

    let memo_id = Uuid::new_v4().to_string();
    let now = now_epoch_ms();
    let message_id: String = conn
        .query_row(
            "SELECT id FROM messages
             WHERE conversation_id = ?1 AND role = 'user'
             ORDER BY timestamp DESC LIMIT 1",
            [conversation_id],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| "unknown".to_string());

    conn.execute(
        "INSERT INTO memos (id, message_id, conversation_id, project_key, content, type, tags, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 'roundtable_archive', '[\"roundtable\"]', ?6)",
        params![memo_id, message_id, conversation_id, project_key, content, now],
    )?;

    Ok(())
}

/// Generate and save a short shared brief after a roundtable round.
///
/// The brief is a rule-based summary (no LLM call) stored as a memo with
/// type = `roundtable_brief`. It captures each participant's key position
/// from their latest response in the transcript.
///
/// Failure is silently swallowed — brief generation must never break the main flow.
pub fn save_shared_brief(
    conn: &rusqlite::Connection,
    conversation_id: &str,
    topic: &str,
    transcript: &[(String, String)],
    rt_mode: &str,
) {
    if transcript.is_empty() {
        return;
    }

    // Resolve project_key
    let project_key: String = match conn.query_row(
        "SELECT project_key FROM conversations WHERE id = ?1",
        [conversation_id],
        |row| row.get(0),
    ) {
        Ok(k) => k,
        Err(_) => return,
    };

    // Deduplicate participant names (preserve order)
    let mut seen = std::collections::HashSet::new();
    let unique_names: Vec<&str> = transcript
        .iter()
        .map(|(n, _)| n.as_str())
        .filter(|n| seen.insert(*n))
        .collect();

    // Extract each participant's LAST response, take first 2 sentences as summary
    let mut position_lines: Vec<String> = Vec::new();
    for name in &unique_names {
        // Find the last entry for this participant
        if let Some((_, content)) = transcript.iter().rev().find(|(n, _)| n == name) {
            let summary = first_sentences(content, 2);
            position_lines.push(format!("- **{}**: {}", name, summary));
        }
    }

    let brief_content = format!(
        "# Roundtable Brief\n\n\
         **Topic:** {}\n\
         **Mode:** {}\n\
         **Participants:** {}\n\n\
         ## Key Positions\n\n\
         {}\n",
        topic,
        rt_mode,
        unique_names.join(", "),
        position_lines.join("\n"),
    );

    let memo_id = Uuid::new_v4().to_string();
    let now = now_epoch_ms();
    let message_id: String = conn
        .query_row(
            "SELECT id FROM messages
             WHERE conversation_id = ?1 AND role = 'user'
             ORDER BY timestamp DESC LIMIT 1",
            [conversation_id],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| "unknown".to_string());

    let _ = conn.execute(
        "INSERT INTO memos (id, message_id, conversation_id, project_key, content, type, tags, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 'roundtable_brief', '[\"roundtable\",\"brief\"]', ?6)",
        params![memo_id, message_id, conversation_id, project_key, brief_content, now],
    );
}

/// Persist roundtable consensus rows extracted from a synthesizer response.
///
/// 한 라운드의 synthesizer 가 합의 항목 목록을 도출하면, 본 helper 가 axis 별
/// row 로 `roundtable_consensus` 테이블에 누적 적재한다. 이후 라운드 N+1 의
/// `build_round_prompt_with_identity()` 가 `prior_consensus` 를 입력받아 *"이미
/// 합의된 axis"* 정보를 synthesizer prompt 에 명시적으로 주입 → 같은 합의 재시도
/// 환각 차단 (devbug #263 시나리오 B 회복).
///
/// 정책:
/// - 빈 list 입력 시 no-op (미합의 라운드)
/// - 한 트랜잭션이 아니라 row 단위 best-effort insert — 한 row 실패해도 다음
///   row 시도 (silent log)
/// - INV-RTC-8: RT 미진행 conv 호출 영향 0 (caller 측에서 진입 자체 skip)
///
/// Failure 는 silently swallow — consensus 저장이 RT 본 흐름 깨면 안 됨
/// (synthesizer 결과는 메시지로도 별도 persist 됨).
pub fn save_consensus(
    conn: &rusqlite::Connection,
    conversation_id: &str,
    round_index: u32,
    items: &[ConsensusItem],
) {
    if items.is_empty() {
        return;
    }
    let now = now_epoch_ms();
    for item in items {
        let id = Uuid::new_v4().to_string();
        let participants_json =
            serde_json::to_string(&item.participants).unwrap_or_else(|_| "[]".into());
        let result = conn.execute(
            "INSERT INTO roundtable_consensus
                (id, conversation_id, round_index, axis, decision,
                 participants, confidence, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                id,
                conversation_id,
                round_index as i64,
                item.axis,
                item.decision,
                participants_json,
                item.confidence,
                now,
            ],
        );
        if let Err(e) = result {
            eprintln!("[rt-consensus] save row failed: {e}");
        }
    }
}

/// A single consensus item — one *axis* (subject) and the *decision* reached.
///
/// Synthesizer prompt 의 marker 응답 (Task 02) 에서 1:N 으로 추출됨. axis 는
/// 짧은 주제 키워드, decision 은 1~3 문장 요약, participants 는 합의에 동의한
/// 참여자 이름 list, confidence 는 synthesizer 의 0~1 판단치.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConsensusItem {
    pub axis: String,
    pub decision: String,
    pub participants: Vec<String>,
    pub confidence: f64,
}

/// Load prior consensus items for a conversation, ordered by round_index.
///
/// caller (synthesizer prompt assembly + Architect ContextPack section) 가
/// 누적 합의를 input 으로 활용. 빈 list 반환 시 caller 측 fast path skip.
pub fn load_consensus(
    conn: &rusqlite::Connection,
    conversation_id: &str,
) -> Vec<(u32, ConsensusItem)> {
    let mut stmt = match conn.prepare(
        "SELECT round_index, axis, decision, participants, confidence
           FROM roundtable_consensus
          WHERE conversation_id = ?1
          ORDER BY round_index ASC, created_at ASC",
    ) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let rows = stmt.query_map([conversation_id], |row| {
        let round_index: i64 = row.get(0)?;
        let axis: String = row.get(1)?;
        let decision: String = row.get(2)?;
        let participants_json: String = row.get(3)?;
        let confidence: f64 = row.get(4)?;
        let participants: Vec<String> =
            serde_json::from_str(&participants_json).unwrap_or_default();
        Ok((
            round_index as u32,
            ConsensusItem { axis, decision, participants, confidence },
        ))
    });
    match rows {
        Ok(it) => it.filter_map(|r| r.ok()).collect(),
        Err(_) => Vec::new(),
    }
}

/// Extract `ConsensusItem` rows from a synthesizer response body.
///
/// Synthesizer prompt (Task 02) 가 *"## Agreed axes"* 섹션을 *machine-readable*
/// marker 형식으로 출력하도록 지시 (Plan §3 Task 02 — JSON 또는 marker 기반).
/// 본 함수는 두 형식 모두 시도 → 실패 시 빈 list 반환 (silent — 합의가 *없는*
/// 라운드 또는 synthesizer 가 다른 형식으로 응답한 케이스).
///
/// 우선순위:
/// 1. `<!-- tunaflow:consensus -->` JSON fence (가장 결정적)
/// 2. Markdown bullet list under `## Agreed axes` heading (자연어 fallback)
///
/// INV-RTC-2 (Synthesizer 본체 알고리즘 보존): 본 함수는 *추출* 만 — synthesizer
/// 의 voting / dissent 판단 자체는 손대지 않음.
pub fn extract_consensus_items(synthesizer_response: &str) -> Vec<ConsensusItem> {
    // 1) JSON fence — primary path
    if let Some(items) = parse_consensus_json_fence(synthesizer_response) {
        if !items.is_empty() {
            return items;
        }
    }
    // 2) Markdown fallback — "## Agreed axes" 또는 "## Consensus" 섹션 아래의
    //    `- **<axis>**: <decision>` 패턴
    parse_consensus_markdown(synthesizer_response)
}

fn parse_consensus_json_fence(text: &str) -> Option<Vec<ConsensusItem>> {
    let start = text.find("<!-- tunaflow:consensus")?;
    let after_marker = &text[start..];
    let body_start = after_marker.find("-->")? + 3;
    let body_end = after_marker[body_start..].find("<!-- /tunaflow:consensus -->")?;
    let body = after_marker[body_start..body_start + body_end].trim();
    let parsed: Vec<ConsensusItem> = serde_json::from_str(body).ok()?;
    Some(parsed)
}

fn parse_consensus_markdown(text: &str) -> Vec<ConsensusItem> {
    // 섹션 헤더 후보 — synthesizer 가 자연어로 출력했을 때
    let headers = ["## Agreed axes", "## Consensus", "## 합의된 항목", "## Agreements"];
    let mut section_start: Option<usize> = None;
    for h in &headers {
        if let Some(pos) = text.find(h) {
            section_start = Some(pos + h.len());
            break;
        }
    }
    let Some(start) = section_start else { return Vec::new(); };
    let after = &text[start..];
    // 다음 ## 헤더까지 또는 끝까지
    let end = after[1..].find("\n## ").map(|p| p + 1).unwrap_or(after.len());
    let body = &after[..end];

    let mut items: Vec<ConsensusItem> = Vec::new();
    for raw_line in body.lines() {
        let line = raw_line.trim();
        if !line.starts_with("- ") && !line.starts_with("* ") {
            continue;
        }
        let bullet_body = &line[2..];
        // `**axis**: decision` 또는 `axis: decision`
        let (axis, decision) = if let Some(rest) = bullet_body.strip_prefix("**") {
            if let Some(close_idx) = rest.find("**") {
                let axis = rest[..close_idx].trim();
                let after_close = rest[close_idx + 2..].trim_start();
                let decision = after_close.strip_prefix(':').unwrap_or(after_close).trim();
                (axis.to_string(), decision.to_string())
            } else {
                continue;
            }
        } else if let Some(idx) = bullet_body.find(':') {
            let axis = bullet_body[..idx].trim();
            let decision = bullet_body[idx + 1..].trim();
            (axis.to_string(), decision.to_string())
        } else {
            continue;
        };
        if axis.is_empty() || decision.is_empty() {
            continue;
        }
        items.push(ConsensusItem {
            axis,
            decision,
            participants: Vec::new(), // markdown fallback 에선 참여자 미파악
            confidence: 0.5,           // 자연어 추출이라 중립 confidence
        });
    }
    items
}

/// Extract the first N sentences from text (split by `.`).
/// Truncates to ~300 chars using char-boundary-safe slicing.
fn first_sentences(text: &str, n: usize) -> String {
    let mut count = 0;
    let mut end = text.len();
    for (i, _) in text.match_indices('.') {
        count += 1;
        if count >= n {
            end = (i + 1).min(text.len());
            break;
        }
    }
    let result = &text[..end];
    if result.len() > 300 {
        // Walk back to a char boundary
        let safe_end = result
            .char_indices()
            .map(|(i, _)| i)
            .take_while(|&i| i <= 297)
            .last()
            .unwrap_or(0);
        format!("{}...", &result[..safe_end])
    } else {
        result.to_string()
    }
}

// ─── Tests: roundtable consensus persistence (devbug #263 Task 02) ──────────
#[cfg(test)]
mod consensus_tests {
    use super::*;
    use rusqlite::Connection;

    fn build_db_with_v50() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        // Minimal schema dependency: roundtable_consensus + conversations FK target.
        conn.execute_batch(
            "CREATE TABLE conversations (id TEXT PRIMARY KEY);
             CREATE TABLE roundtable_consensus (
                id              TEXT    PRIMARY KEY,
                conversation_id TEXT    NOT NULL,
                round_index     INTEGER NOT NULL,
                axis            TEXT    NOT NULL,
                decision        TEXT    NOT NULL,
                participants    TEXT    NOT NULL DEFAULT '[]',
                confidence      REAL    NOT NULL DEFAULT 0.0,
                created_at      INTEGER NOT NULL
             );
             CREATE INDEX idx_roundtable_consensus_conv_round
                ON roundtable_consensus(conversation_id, round_index);
             INSERT INTO conversations (id) VALUES ('conv-1');",
        )
        .unwrap();
        conn
    }

    /// Devbug #263 시나리오 B 회복 — 라운드 누적 합의가 persist 후 round_index
    /// 순으로 load 되는지 검증.
    #[test]
    fn consensus_persisted_across_rounds() {
        let conn = build_db_with_v50();

        let r1 = vec![ConsensusItem {
            axis: "ContextPack 압축 전략".into(),
            decision: "Lite/Standard/Full 자동 모드 유지, 동적 예산 분배".into(),
            participants: vec!["claude".into(), "codex".into()],
            confidence: 0.9,
        }];
        save_consensus(&conn, "conv-1", 1, &r1);

        let r2 = vec![ConsensusItem {
            axis: "rawq 인덱스 갱신".into(),
            decision: "FS watcher debounce 500ms, 파일당 임베딩 1회".into(),
            participants: vec!["claude".into(), "gemini".into()],
            confidence: 0.85,
        }];
        save_consensus(&conn, "conv-1", 2, &r2);

        // No-op for empty input (MUST not error and MUST not insert anything).
        save_consensus(&conn, "conv-1", 3, &[]);

        let loaded = load_consensus(&conn, "conv-1");
        assert_eq!(loaded.len(), 2, "두 라운드의 합의가 누적되어야 함");

        // round_index ASC 순 — 라운드 1 합의가 먼저
        let (r1_round, r1_item) = &loaded[0];
        assert_eq!(*r1_round, 1);
        assert_eq!(r1_item.axis, "ContextPack 압축 전략");
        assert_eq!(r1_item.participants, vec!["claude", "codex"]);

        let (r2_round, r2_item) = &loaded[1];
        assert_eq!(*r2_round, 2);
        assert_eq!(r2_item.axis, "rawq 인덱스 갱신");
    }

    /// 다른 conversation 의 합의가 누설되면 안 됨 (INV-RTC-7/8 영역).
    #[test]
    fn consensus_isolated_per_conversation() {
        let conn = build_db_with_v50();
        conn.execute("INSERT INTO conversations (id) VALUES ('conv-2')", [])
            .unwrap();

        save_consensus(
            &conn,
            "conv-1",
            1,
            &[ConsensusItem {
                axis: "axis-A".into(),
                decision: "decision-A".into(),
                participants: vec!["claude".into()],
                confidence: 0.8,
            }],
        );
        save_consensus(
            &conn,
            "conv-2",
            1,
            &[ConsensusItem {
                axis: "axis-B".into(),
                decision: "decision-B".into(),
                participants: vec!["codex".into()],
                confidence: 0.7,
            }],
        );

        let conv1 = load_consensus(&conn, "conv-1");
        let conv2 = load_consensus(&conn, "conv-2");
        assert_eq!(conv1.len(), 1);
        assert_eq!(conv2.len(), 1);
        assert_eq!(conv1[0].1.axis, "axis-A");
        assert_eq!(conv2[0].1.axis, "axis-B");
    }

    /// JSON marker fence 추출 — synthesizer 응답의 primary path.
    #[test]
    fn extract_consensus_from_json_marker() {
        let response = r#"
Some preamble text.

## Consensus

Both reviewers agreed on the compression strategy.

<!-- tunaflow:consensus -->
[
  {"axis":"compression","decision":"Lite/Standard/Full automode","participants":["claude","codex"],"confidence":0.9},
  {"axis":"budget","decision":"dynamic per-section budget","participants":["claude","codex","gemini"],"confidence":0.85}
]
<!-- /tunaflow:consensus -->

trailing text.
"#;
        let items = extract_consensus_items(response);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].axis, "compression");
        assert_eq!(items[0].confidence, 0.9);
        assert_eq!(items[1].participants, vec!["claude", "codex", "gemini"]);
    }

    /// Markdown bullet fallback — synthesizer 가 marker 무시했을 때.
    #[test]
    fn extract_consensus_from_markdown_fallback() {
        let response = "\
## Vote tally\n\
2 pass / 1 fail\n\n\
## Agreed axes\n\
- **compression**: Lite/Standard/Full automode preserved.\n\
- **budget**: dynamic per-section budget.\n\n\
## Contested\n\
- Whether to add a memory tier.\n";

        let items = extract_consensus_items(response);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].axis, "compression");
        assert_eq!(items[1].axis, "budget");
        // markdown fallback 은 confidence 0.5 중립 — INV-RTC-2 보존: synthesizer
        // 의 voting 판단을 기계적으로 변형하지 않음.
        assert!((items[0].confidence - 0.5).abs() < 1e-6);
    }

    /// 빈 list 또는 무관 응답에서 합의 추출 = 빈 list.
    #[test]
    fn extract_consensus_empty_when_no_marker_or_section() {
        let response = "Plain reviewer verdict with no consensus block.";
        let items = extract_consensus_items(response);
        assert!(items.is_empty());

        // 빈 JSON array marker
        let empty_marker = "<!-- tunaflow:consensus -->\n[]\n<!-- /tunaflow:consensus -->";
        let items = extract_consensus_items(empty_marker);
        assert!(items.is_empty());
    }
}
