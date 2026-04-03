use rusqlite::params;
use uuid::Uuid;

use crate::commands::agents_helpers::trace_log::{insert_trace_log, SpanInfo, new_span_id};
use crate::db::migrations::now_epoch_ms;
use crate::db::models::Message;
use crate::errors::AppError;

use super::executor::ParticipantResult;

/// Persist a round header (system message) and return it.
pub fn persist_header(
    conn: &rusqlite::Connection,
    conversation_id: &str,
    text: &str,
) -> Result<Message, AppError> {
    let id = Uuid::new_v4().to_string();
    let now = now_epoch_ms();
    conn.execute(
        "INSERT INTO messages (id, conversation_id, role, content, timestamp, status, engine)
         VALUES (?1, ?2, 'assistant', ?3, ?4, 'done', 'system')",
        params![id, conversation_id, text, now],
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

/// Persist a single participant result, update conversation usage, and write trace log.
/// `trace_id` / `root_span_id` are passed from the roundtable command for parent linkage.
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
