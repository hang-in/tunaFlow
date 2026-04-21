//! Server-side tool-request orchestration for the HTTP API path.
//!
//! The desktop client runs this logic in `src/lib/toolRequestHandler.ts` —
//! it detects `<!-- tunaflow:tool-request:KIND:QUERY -->` markers in an
//! assistant response, invokes the relevant Tauri command, and re-sends a
//! follow-up system message. The HTTP path (mobile) has no such client-side
//! loop, so an agent ending its turn with "원문을 가져오겠습니다" plus a
//! tool-request marker used to leave mobile users dangling forever — the
//! marker was just text with no executor.
//!
//! This module is intentionally narrow. It covers the handful of
//! marker kinds that unblock the "직전 turn/메시지를 확인" use case which
//! showed up in practice (`recent_turns`, `probe_message`, `fetch_slice`,
//! `full_message`) and explicitly reports "server-side not yet supported"
//! for the remaining kinds so agents can react rather than loop silently.
use regex::Regex;
use rusqlite::Connection;

/// Upper bound for how many markers we process per assistant response.
/// Matches the frontend contract in `toolRequestHandler.ts:29` (`.slice(0, 3)`).
const MAX_TOOL_REQUESTS_PER_TURN: usize = 3;

/// Per-turn char cap for `recent_turns` body — stays in sync with
/// `commands::conversation_memory::RECENT_TURN_MAX_CHARS`.
const RECENT_TURN_MAX_CHARS: usize = 2_000;

/// Preview window for the probe head / tail, same defaults as the Tauri
/// command in `conversation_memory::probe_message`.
const PROBE_HEAD_LEN: usize = 200;
const PROBE_TAIL_LEN: usize = 200;

/// Hard cap for a single `fetch_slice` response — matches
/// `conversation_memory::MESSAGE_SLICE_MAX_LEN`.
const SLICE_MAX_LEN: usize = 16_000;

const MARKER_RE: &str = r"<!--\s*tunaflow:tool-request:([\w-]+):(.+?)\s*-->";

#[derive(Debug, Clone)]
pub struct ToolRequest {
    pub kind: String,
    pub query: String,
}

/// Extract at most `MAX_TOOL_REQUESTS_PER_TURN` tool-request markers from an
/// assistant response. Silent on regex compile failure — the caller treats
/// an empty vector as "no follow-up needed".
pub fn extract_tool_requests(content: &str) -> Vec<ToolRequest> {
    let re = match Regex::new(MARKER_RE) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    re.captures_iter(content)
        .filter_map(|cap| {
            let kind = cap.get(1)?.as_str().to_string();
            let query = cap.get(2)?.as_str().trim().to_string();
            if query.is_empty() { None } else { Some(ToolRequest { kind, query }) }
        })
        .take(MAX_TOOL_REQUESTS_PER_TURN)
        .collect()
}

/// Execute the given tool requests and assemble a follow-up prompt the
/// HTTP agent loop can feed back in as the next turn. Returns `None` when
/// there is nothing useful to say (e.g. all kinds were unsupported AND
/// produced no message — practically we always report something, so this
/// is also `None` when `requests` is empty).
pub fn execute_tool_requests(
    conn: &Connection,
    conversation_id: &str,
    requests: &[ToolRequest],
) -> Option<String> {
    if requests.is_empty() {
        return None;
    }
    let mut blocks: Vec<String> = Vec::new();
    for req in requests {
        let block = match req.kind.as_str() {
            "recent_turns" => exec_recent_turns(conn, conversation_id, &req.query),
            "probe_message" => exec_probe_message(conn, &req.query),
            "fetch_slice" => exec_fetch_slice(conn, &req.query),
            "full_message" => exec_full_message(conn, &req.query),
            other => Some(format!(
                "> `{other}` 는 HTTP API 경로에서 아직 지원되지 않는 tool-request 입니다. \
                 (데스크톱 경로에서는 `src/lib/toolRequestHandler.ts` 가 처리) \
                 지금 턴에서는 해당 도구 없이 답하거나, 필요하면 `recent_turns` / \
                 `probe_message` / `fetch_slice` / `full_message` 로 접근 가능한 \
                 정보 범위로 조정해 주세요."
            )),
        };
        if let Some(b) = block {
            blocks.push(b);
        }
    }
    if blocks.is_empty() {
        return None;
    }
    Some(format!(
        "### 🛠️ 도구 호출 결과\n\n{}\n\n> 위 정보를 참고하여 작업을 계속하세요.",
        blocks.join("\n\n"),
    ))
}

// ─── kind-specific implementations ─────────────────────────────────────

fn exec_recent_turns(conn: &Connection, conv_id: &str, query: &str) -> Option<String> {
    let n: i64 = query
        .trim()
        .parse::<i64>()
        .ok()
        .filter(|v| *v > 0)
        .map(|v| v.min(10))
        .unwrap_or(3);
    let mut stmt = conn
        .prepare(
            "SELECT role, persona, engine, content, timestamp FROM messages
             WHERE conversation_id = ?1 AND role IN ('user','assistant') AND status = 'done'
             ORDER BY timestamp DESC LIMIT ?2",
        )
        .ok()?;
    let mut rows: Vec<(String, Option<String>, Option<String>, String, i64)> = stmt
        .query_map(rusqlite::params![conv_id, n], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })
        .ok()?
        .filter_map(|r| r.ok())
        .collect();
    if rows.is_empty() {
        return Some("> 현재 대화에서 최근 turn 을 찾지 못했습니다.".to_string());
    }
    rows.reverse(); // oldest → newest, reading order
    let lines: Vec<String> = rows
        .iter()
        .map(|(role, persona, engine, raw, _ts)| {
            let label = if role == "assistant" {
                format!(
                    "[assistant{}{}]",
                    persona.as_deref().map(|p| format!(":{p}")).unwrap_or_default(),
                    engine.as_deref().map(|e| format!(" ({e})")).unwrap_or_default(),
                )
            } else {
                "[user]".to_string()
            };
            let content = if raw.chars().count() > RECENT_TURN_MAX_CHARS {
                let head: String = raw.chars().take(RECENT_TURN_MAX_CHARS).collect();
                format!("{head}\n…(tail truncated)")
            } else {
                raw.clone()
            };
            format!("{label}\n{content}")
        })
        .collect();
    Some(format!(
        "## 🕒 현재 대화 최근 {} turn (전문)\n\n{}",
        rows.len(),
        lines.join("\n\n---\n\n"),
    ))
}

fn exec_probe_message(conn: &Connection, query: &str) -> Option<String> {
    let msg_id = query.trim();
    if msg_id.is_empty() {
        return None;
    }
    let row: Result<(String, Option<String>, Option<String>, String, i64), _> = conn.query_row(
        "SELECT role, persona, engine, content, timestamp FROM messages WHERE id = ?1",
        [msg_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
    );
    let (role, persona, engine, content, _ts) = match row {
        Ok(v) => v,
        Err(_) => return Some(format!("> probe_message 실패: 메시지 `{msg_id}` 를 찾지 못했습니다.")),
    };
    let total = content.chars().count();
    let head: String = content.chars().take(PROBE_HEAD_LEN).collect();
    let tail: String = if total > PROBE_HEAD_LEN + PROBE_TAIL_LEN {
        content.chars().skip(total - PROBE_TAIL_LEN).collect()
    } else {
        String::new()
    };
    let author = format_author(&role, persona.as_deref(), engine.as_deref());
    let tail_display = if tail.is_empty() {
        "(message shorter than head+tail — see head only)".to_string()
    } else {
        tail
    };
    Some(format!(
        "## 🔍 Probe {}\n\n- **Length**: {} chars\n- **Author**: {}\n\n### Head ({})\n{}\n\n### Tail ({})\n{}",
        short_id(msg_id),
        total,
        author,
        head.chars().count(),
        head,
        PROBE_TAIL_LEN,
        tail_display,
    ))
}

fn exec_fetch_slice(conn: &Connection, query: &str) -> Option<String> {
    let parts: Vec<&str> = query.split(':').collect();
    if parts.len() < 3 {
        return Some(
            "> fetch_slice 인자 부족: `<messageId>:<offset>:<len>` 형식 필요".to_string(),
        );
    }
    let msg_id = parts[0].trim();
    let offset: i64 = match parts[1].trim().parse() {
        Ok(v) => v,
        Err(_) => return Some(format!("> fetch_slice offset 파싱 실패: `{}`", parts[1])),
    };
    let len: i64 = match parts[2].trim().parse() {
        Ok(v) => v,
        Err(_) => return Some(format!("> fetch_slice len 파싱 실패: `{}`", parts[2])),
    };
    if msg_id.is_empty() {
        return None;
    }
    let off = offset.max(0) as usize;
    let cap = len.clamp(0, SLICE_MAX_LEN as i64) as usize;
    let content: String = match conn.query_row(
        "SELECT content FROM messages WHERE id = ?1",
        [msg_id],
        |r| r.get(0),
    ) {
        Ok(c) => c,
        Err(_) => {
            return Some(format!(
                "> fetch_slice 실패: 메시지 `{msg_id}` 를 찾지 못했습니다."
            ))
        }
    };
    let total = content.chars().count();
    let slice: String = content.chars().skip(off).take(cap).collect();
    let end = (off + slice.chars().count()).min(total);
    let body = if slice.is_empty() {
        "(empty — offset beyond message length)".to_string()
    } else {
        slice
    };
    Some(format!(
        "## 🪄 Slice {} [{}..{}] / {}\n\n{}",
        short_id(msg_id),
        off,
        end,
        total,
        body,
    ))
}

fn exec_full_message(conn: &Connection, query: &str) -> Option<String> {
    let msg_id = query.trim();
    if msg_id.is_empty() {
        return None;
    }
    let row: Result<(String, Option<String>, Option<String>, String), _> = conn.query_row(
        "SELECT role, persona, engine, content FROM messages WHERE id = ?1",
        [msg_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    );
    let (role, persona, engine, content) = match row {
        Ok(v) => v,
        Err(_) => return Some(format!("> full_message 실패: 메시지 `{msg_id}` 를 찾지 못했습니다.")),
    };
    let length = content.chars().count();
    let author = format_author(&role, persona.as_deref(), engine.as_deref());
    Some(format!(
        "## 📄 Full message {} ({} chars)\n- {}\n\n{}",
        short_id(msg_id),
        length,
        author,
        content,
    ))
}

fn format_author(role: &str, persona: Option<&str>, engine: Option<&str>) -> String {
    let persona_str = persona.map(|p| format!(":{p}")).unwrap_or_default();
    let engine_str = engine.map(|e| format!(" ({e})")).unwrap_or_default();
    format!("{role}{persona_str}{engine_str}")
}

fn short_id(id: &str) -> String {
    let n = id.chars().count();
    if n <= 8 {
        id.to_string()
    } else {
        let head: String = id.chars().take(8).collect();
        format!("{head}…")
    }
}

// ─── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_parses_single_marker() {
        let content = "some text <!-- tunaflow:tool-request:full_message:abc-123 --> more";
        let reqs = extract_tool_requests(content);
        assert_eq!(reqs.len(), 1);
        assert_eq!(reqs[0].kind, "full_message");
        assert_eq!(reqs[0].query, "abc-123");
    }

    #[test]
    fn extract_caps_at_three_markers() {
        let content = (0..10)
            .map(|i| format!("<!-- tunaflow:tool-request:recent_turns:{i} -->"))
            .collect::<Vec<_>>()
            .join(" ");
        let reqs = extract_tool_requests(&content);
        assert_eq!(reqs.len(), 3);
    }

    #[test]
    fn extract_skips_empty_query() {
        let content = "<!-- tunaflow:tool-request:docs:  -->";
        let reqs = extract_tool_requests(content);
        assert!(reqs.is_empty());
    }

    #[test]
    fn extract_supports_hyphenated_kind() {
        let content = "<!-- tunaflow:tool-request:insight-update:fid|resolved|note -->";
        let reqs = extract_tool_requests(content);
        assert_eq!(reqs.len(), 1);
        assert_eq!(reqs[0].kind, "insight-update");
    }

    #[test]
    fn unsupported_kind_returns_graceful_note() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE messages (id TEXT PRIMARY KEY, conversation_id TEXT,
             role TEXT, persona TEXT, engine TEXT, content TEXT, status TEXT,
             timestamp INTEGER);",
        )
        .unwrap();
        let reqs = vec![ToolRequest {
            kind: "docs".into(),
            query: "react hooks".into(),
        }];
        let out = execute_tool_requests(&conn, "conv-1", &reqs).unwrap();
        assert!(out.contains("HTTP API 경로에서 아직 지원되지 않는"));
    }

    #[test]
    fn recent_turns_returns_reversed_chronological_slice() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE messages (id TEXT PRIMARY KEY, conversation_id TEXT,
             role TEXT, persona TEXT, engine TEXT, content TEXT, status TEXT,
             timestamp INTEGER);
             INSERT INTO messages VALUES
               ('m1','c1','user',NULL,NULL,'first user','done',1),
               ('m2','c1','assistant',NULL,'claude','first reply','done',2),
               ('m3','c1','user',NULL,NULL,'second user','done',3);",
        )
        .unwrap();
        let reqs = vec![ToolRequest {
            kind: "recent_turns".into(),
            query: "2".into(),
        }];
        let out = execute_tool_requests(&conn, "c1", &reqs).unwrap();
        // most recent 2 in reading order (m2 then m3)
        let m2_pos = out.find("first reply").expect("m2 present");
        let m3_pos = out.find("second user").expect("m3 present");
        assert!(m2_pos < m3_pos, "m2 should come before m3 in reading order");
        assert!(!out.contains("first user"), "m1 should be out of the window");
    }

    #[test]
    fn probe_message_reports_head_and_tail() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE messages (id TEXT PRIMARY KEY, conversation_id TEXT,
             role TEXT, persona TEXT, engine TEXT, content TEXT, status TEXT,
             timestamp INTEGER);
             INSERT INTO messages VALUES
               ('m-long','c1','assistant',NULL,'claude',
                'A' || hex(randomblob(500)) || 'Z','done',1);",
        )
        .unwrap();
        let reqs = vec![ToolRequest {
            kind: "probe_message".into(),
            query: "m-long".into(),
        }];
        let out = execute_tool_requests(&conn, "c1", &reqs).unwrap();
        assert!(out.contains("Probe"));
        assert!(out.contains("Length"));
        assert!(out.contains("Head"));
        assert!(out.contains("Tail"));
    }

    #[test]
    fn probe_message_missing_id_reports_error_gracefully() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE messages (id TEXT PRIMARY KEY, conversation_id TEXT,
             role TEXT, persona TEXT, engine TEXT, content TEXT, status TEXT,
             timestamp INTEGER);",
        )
        .unwrap();
        let reqs = vec![ToolRequest {
            kind: "probe_message".into(),
            query: "missing-id".into(),
        }];
        let out = execute_tool_requests(&conn, "c1", &reqs).unwrap();
        assert!(out.contains("probe_message 실패"));
    }
}
