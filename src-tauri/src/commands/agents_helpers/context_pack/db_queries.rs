/// DB-dependent ContextPack queries.
///
/// All functions here take `&rusqlite::Connection` and return pre-built section strings
/// or raw data. Pure assembly (no DB) lives in `section_builders.rs`.
use rusqlite::params;

use super::utils::{format_section, truncate_str};

/// Maximum number of prior messages for non-Claude lite context prefix.
pub const LITE_CONTEXT_MESSAGES_LIMIT: i64 = 4;
/// Maximum total characters for the lite context prefix.
const LITE_CONTEXT_MAX_CHARS: usize = 4000;

/// Maximum characters for the anchor message included in inheritance context.
const ANCHOR_MAX_CHARS: usize = 600;
/// Maximum recent parent turns for thread inheritance.
const THREAD_PARENT_RECENT: i64 = 3;
/// Maximum recent parent turns for RT inheritance (more concise).
const RT_PARENT_RECENT: i64 = 2;

// ─── Plan ────────────────────────────────────────────────────────────────────

/// multiDeveloperActivePlanIsolationPlan §Layer A′: brand 진입 시 해당
/// brand_id 와 매칭되는 plan 을 우선 lookup.
///
/// 본 함수는 `build_plan_section` 과 `load_context_data` 가 공유하는 단일
/// 진입점이다. brand conv 라면:
///   1) `branches.id` 를 추출
///   2) `plans.implementation_branch_id` 또는 `plans.review_branch_id` 가 그
///      brand_id 와 매칭되는 plan 을 가장 최근 갱신 순으로 1건
/// non-brand 또는 매칭 0건이면 fallback 으로 main conv 의 active plan.
///
/// 같은 conv 안에서 multi-Developer 가 동시 작업할 때 한쪽 Developer 의
/// active plan 이 다른 Developer 의 ContextPack 으로 누출되는 문제를 차단한다.
/// 매칭이 0건이면 (옛 plan 또는 임시 brand) 기존 동작 (main 의 active) 으로
/// graceful fallback 하므로 회귀 0.
pub fn lookup_plan_for_conversation(
    conn: &rusqlite::Connection,
    conversation_id: &str,
) -> Option<(String, String, Option<String>, String, Option<String>)> {
    if let Some(branch_id) = conversation_id.strip_prefix("branch:") {
        // Layer A′: brand 매핑 plan 우선
        if let Ok(row) = conn.query_row(
            "SELECT id, title, description, phase, slug FROM plans
             WHERE (implementation_branch_id = ?1 OR review_branch_id = ?1)
               AND status NOT IN ('done','abandoned')
             ORDER BY updated_at DESC LIMIT 1",
            [branch_id],
            |row| Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
            )),
        ) {
            return Some(row);
        }
    }

    // Fallback: main conv 의 active plan (비-brand 또는 매칭 0건)
    let plan_conv_id = resolve_plan_conversation_id(conn, conversation_id);
    conn.query_row(
        "SELECT id, title, description, phase, slug FROM plans
         WHERE conversation_id = ?1 AND status = 'active'
         ORDER BY updated_at DESC LIMIT 1",
        [&plan_conv_id],
        |row| Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, Option<String>>(4)?,
        )),
    ).ok()
}

/// Build `## Active Plan` section from DB for the given conversation.
pub fn build_plan_section(
    conn: &rusqlite::Connection,
    conversation_id: &str,
) -> Option<String> {
    let (plan_id, title, description, phase, slug) =
        lookup_plan_for_conversation(conn, conversation_id)?;

    let mut stmt = conn
        .prepare(
            "SELECT idx, title, status, details, depends_on, parallel_group FROM plan_subtasks
             WHERE plan_id = ?1 ORDER BY idx",
        )
        .ok()?;
    let subtasks: Vec<(i64, String, String, Option<String>, Option<String>, Option<String>)> = stmt
        .query_map([&plan_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?))
        })
        .ok()?
        .filter_map(|r| r.ok())
        .collect();

    let mut out = format!("## Active Plan (phase: {})\n\n### {}\n", phase, title);
    if let Some(desc) = &description {
        if !desc.is_empty() {
            out.push_str(&format!("{}\n", desc));
        }
    }
    // Canonical slug — the only correct filename prefix for this plan's
    // documents. Architect must write `docs/plans/{slug}-task-NN.md` using
    // this exact value (not a slugify of the title, not a manual
    // abbreviation). Reviewer and result/review writers all read the same
    // source, so any other prefix will not be discovered downstream.
    if let Some(s) = &slug {
        if !s.is_empty() {
            out.push_str(&format!(
                "\n> **Plan slug (canonical):** `{}`\n> Task file prefix MUST be `{}-task-NN.md` — use this slug verbatim.\n",
                s, s
            ));
        }
    }
    out.push_str(&format!("\n> **Current phase: {}** — Do NOT create a new plan. Propose revisions to this plan if needed.\n", phase));

    if !subtasks.is_empty() {
        out.push_str("\n**Subtasks:**\n");
        for (idx, title, status, _, depends_on, group) in &subtasks {
            let icon = match status.as_str() {
                "done" => "✅",
                "in_progress" => "🔧",
                _ => "⬜",
            };
            let mut suffix = String::new();
            if let Some(g) = group {
                if !g.is_empty() { suffix.push_str(&format!(" [{}]", g)); }
            }
            if let Some(deps) = depends_on {
                if deps != "[]" && !deps.is_empty() {
                    let dep_str = deps.trim_matches(|c| c == '[' || c == ']').replace(' ', "");
                    if !dep_str.is_empty() {
                        suffix.push_str(&format!(" (depends: {})", dep_str));
                    }
                }
            }
            out.push_str(&format!("- {} Task {:02}: {}{}\n", icon, idx, title, suffix));
        }
    }

    let done_count = subtasks.iter().filter(|(_, _, s, _, _, _)| s == "done").count();
    let total = subtasks.len();
    if total > 0 {
        out.push_str(&format!("\n**Progress:** {}/{} done\n", done_count, total));
    }

    Some(out)
}

/// Resolve branch conversation_id to its parent for plan lookup.
pub fn resolve_plan_conversation_id(conn: &rusqlite::Connection, conversation_id: &str) -> String {
    if !conversation_id.starts_with("branch:") {
        return conversation_id.to_string();
    }
    let branch_id = &conversation_id["branch:".len()..];
    conn.query_row(
        "SELECT conversation_id FROM branches WHERE id = ?1",
        [branch_id],
        |row| row.get::<_, String>(0),
    )
    .unwrap_or_else(|_| conversation_id.to_string())
}

// ─── Findings & Artifacts ────────────────────────────────────────────────────

/// Build `## Recent Agent Findings` from the most recent roundtable_brief memos.
///
/// Queries briefs from the conversation and any RT branch shadow conversations.
pub fn build_findings_section(
    conn: &rusqlite::Connection,
    conversation_id: &str,
) -> Option<String> {
    let mut stmt = conn
        .prepare(
            "SELECT content FROM memos
             WHERE type = 'roundtable_brief'
               AND (conversation_id = ?1
                    OR conversation_id IN (
                      SELECT 'branch:' || id FROM branches WHERE conversation_id = ?1
                    ))
             ORDER BY created_at DESC LIMIT 3",
        )
        .ok()?;
    let briefs: Vec<String> = stmt
        .query_map([conversation_id], |row| row.get::<_, String>(0))
        .ok()?
        .filter_map(|r| r.ok())
        .collect();

    if briefs.is_empty() {
        return None;
    }

    let mut out = String::from("## Recent Agent Findings\n");
    for (i, brief) in briefs.iter().enumerate() {
        let truncated = if brief.len() > 600 {
            let end = brief.char_indices().map(|(i, _)| i).take_while(|&i| i <= 597).last().unwrap_or(0);
            format!("{}...", &brief[..end])
        } else {
            brief.clone()
        };
        if i > 0 {
            out.push_str("\n---\n");
        }
        out.push('\n');
        out.push_str(&truncated);
        out.push('\n');
    }

    Some(out)
}

/// Build `## Recent Artifacts` from the most recent approved/draft artifacts.
pub fn build_artifact_handoff_section(
    conn: &rusqlite::Connection,
    conversation_id: &str,
) -> Option<String> {
    let mut stmt = conn
        .prepare(
            "SELECT title, type, status, content FROM artifacts
             WHERE conversation_id = ?1
             ORDER BY updated_at DESC LIMIT 3",
        )
        .ok()?;
    let rows: Vec<(String, String, String, String)> = stmt
        .query_map([conversation_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .ok()?
        .filter_map(|r| r.ok())
        .collect();

    if rows.is_empty() {
        return None;
    }

    let mut out = String::from("## Recent Artifacts\n");
    for (title, art_type, status, content) in &rows {
        let preview = if content.len() > 120 {
            let end = content.char_indices().map(|(i, _)| i).take_while(|&i| i <= 117).last().unwrap_or(0);
            format!("{}...", &content[..end])
        } else {
            content.clone()
        };
        out.push_str(&format!(
            "\n**{}** ({}·{}): {}\n",
            title, art_type, status, preview
        ));
    }

    Some(out)
}

// ─── Lite context ────────────────────────────────────────────────────────────

/// Build a lightweight context prefix for non-Claude engines.
///
/// For branch conversations, also includes anchor message + recent parent turns
/// so non-Claude engines in threads still get inherited context.
pub fn build_lite_context_prompt(
    conn: &rusqlite::Connection,
    conversation_id: &str,
    user_prompt: &str,
) -> String {
    use crate::commands::context_queries::{load_anchor_message, load_recent_messages, parent_conversation_id};

    let is_branch = conversation_id.starts_with("branch:");

    let mut parent_prefix = String::new();
    if is_branch {
        if let Some((_role, content)) = load_anchor_message(conn, conversation_id) {
            let truncated = truncate_str(&content, 400);
            parent_prefix.push_str(&format!("Thread anchor:\n{}\n\n", truncated));
        }
        if let Some(parent_id) = parent_conversation_id(conn, conversation_id) {
            let parent_rows = load_recent_messages(conn, &parent_id, 2);
            if !parent_rows.is_empty() {
                parent_prefix.push_str("Parent conversation:\n");
                for (role, content) in &parent_rows {
                    parent_prefix.push_str(&format!("[{}] {}\n", role, truncate_str(content, 200)));
                }
                parent_prefix.push_str("\n---\n\n");
            }
        }
    }

    let Ok(mut stmt) = conn.prepare(
        "SELECT role, content FROM messages
         WHERE conversation_id = ?1
         ORDER BY timestamp DESC LIMIT ?2",
    ) else {
        return format!("{}{}", parent_prefix, user_prompt);
    };

    let mut rows: Vec<(String, String)> = stmt
        .query_map(params![conversation_id, LITE_CONTEXT_MESSAGES_LIMIT], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map(|mapped| mapped.filter_map(|r| r.ok()).collect())
        .unwrap_or_default();
    rows.reverse();

    if rows.is_empty() && parent_prefix.is_empty() {
        return user_prompt.to_string();
    }

    let mut context = parent_prefix;
    if !rows.is_empty() {
        context.push_str("Recent conversation:\n");
        let mut char_count = context.len();
        for (role, content) in &rows {
            let truncated = truncate_str(content, 600);
            let line = format!("[{}] {}\n", role, truncated);
            if char_count + line.len() > LITE_CONTEXT_MAX_CHARS {
                break;
            }
            context.push_str(&line);
            char_count += line.len();
        }
        context.push_str("\n---\n\n");
    }

    format!("{}{}", context, user_prompt)
}

// ─── Thread / RT inheritance ─────────────────────────────────────────────────

/// Build an inheritance context section for a thread (branch) conversation.
///
/// Priority: anchor message > recent parent turns.
pub fn build_thread_inheritance_section(
    conn: &rusqlite::Connection,
    branch_conv_id: &str,
) -> Option<String> {
    use crate::commands::context_queries::{load_anchor_message, load_recent_messages, parent_conversation_id};

    let mut parts: Vec<String> = Vec::new();

    if let Some((_role, content)) = load_anchor_message(conn, branch_conv_id) {
        let truncated = truncate_str(&content, ANCHOR_MAX_CHARS);
        parts.push(format!("### Thread Anchor\n\nThis thread was started from the following message:\n{}", truncated));
    }

    if let Some(parent_id) = parent_conversation_id(conn, branch_conv_id) {
        let recent = load_recent_messages(conn, &parent_id, THREAD_PARENT_RECENT);
        if !recent.is_empty() {
            parts.push(format_section("Recent parent conversation", &recent, 300));
        }
    }

    if parts.is_empty() {
        return None;
    }
    Some(format!("## Thread Context\n\n{}", parts.join("\n\n")))
}

/// Build an inheritance context section for a roundtable conversation.
///
/// Priority: explicit source > anchor message > recent parent turns.
pub fn build_rt_inheritance_section(
    conn: &rusqlite::Connection,
    conversation_id: &str,
    explicit_source: Option<&str>,
) -> Option<String> {
    use crate::commands::context_queries::{load_anchor_message, load_recent_messages, parent_conversation_id};

    let mut parts: Vec<String> = Vec::new();

    if let Some(source) = explicit_source {
        if !source.is_empty() {
            let truncated = truncate_str(source, 800);
            parts.push(format!("### Discussion Source\n\n{}", truncated));
        }
    }

    if parts.is_empty() {
        if let Some((_role, content)) = load_anchor_message(conn, conversation_id) {
            let truncated = truncate_str(&content, ANCHOR_MAX_CHARS);
            parts.push(format!("### Discussion Anchor\n\n{}", truncated));
        }
    }

    if let Some(parent_id) = parent_conversation_id(conn, conversation_id) {
        let recent = load_recent_messages(conn, &parent_id, RT_PARENT_RECENT);
        if !recent.is_empty() {
            parts.push(format_section("Recent parent context", &recent, 200));
        }
    }

    if parts.is_empty() {
        return None;
    }
    Some(format!("## Roundtable Context\n\n{}", parts.join("\n\n")))
}

/// Build a `## Roundtable Consensus` section from `roundtable_consensus` rows.
///
/// devbug #263 Task 04 — Architect 가 dispatch 받는 ContextPack 에 RT 누적
/// 합의를 명시 인계하는 helper. 라운드별 합의 항목 (axis / decision /
/// participants / round_index) 을 prompt 본문에 *"## Roundtable Consensus"*
/// 섹션으로 조립.
///
/// 정책:
/// - INV-RTC-7/8 (RT 미사용 영향 0): row 0건이면 None 반환 → caller 측 섹션
///   skip → ContextPack 변경 0
/// - INV-RTC-4 (기존 ContextPack 섹션 보존): build_findings_section 의
///   `roundtable_brief` LIMIT 3 fallback 은 보조로 그대로 호출 — 본 helper 는
///   *추가 섹션* 만, 기존 섹션 변경 0
/// - 토큰 budget 가드: 라운드별 decision 600자 truncate, 한 conv 의 모든
///   누적 합의를 보여주되 한 합의가 prompt 본문 잠식하지 않게
/// - branch shadow conv 도 cover: `roundtable_consensus.conversation_id` 가
///   `branch:<id>` 형식이면 부모 conv 의 합의도 collect (RT 가 brand session
///   에서 진행된 케이스)
pub fn build_rt_consensus_section(
    conn: &rusqlite::Connection,
    conversation_id: &str,
) -> Option<String> {
    // Collect from this conv + any branch-shadow convs whose parent is this conv.
    let mut stmt = conn
        .prepare(
            "SELECT round_index, axis, decision, participants
               FROM roundtable_consensus
              WHERE conversation_id = ?1
                 OR conversation_id IN (
                    SELECT 'branch:' || id FROM branches WHERE conversation_id = ?1
                 )
              ORDER BY round_index ASC, created_at ASC",
        )
        .ok()?;

    let rows: Vec<(i64, String, String, String)> = stmt
        .query_map([conversation_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .ok()?
        .filter_map(|r| r.ok())
        .collect();

    if rows.is_empty() {
        return None;
    }

    let mut lines: Vec<String> = Vec::with_capacity(rows.len());
    for (round_index, axis, decision, participants_json) in rows {
        let decision_truncated = truncate_str(&decision, 600);
        let participants: Vec<String> =
            serde_json::from_str::<Vec<String>>(&participants_json).unwrap_or_default();
        let by = if participants.is_empty() {
            String::new()
        } else {
            format!(" _(by {})_", participants.join(", "))
        };
        lines.push(format!(
            "- **R{}** **{}**{}: {}",
            round_index, axis, by, decision_truncated
        ));
    }

    Some(format!(
        "## Roundtable Consensus\n\n\
         These axes are *already agreed* in roundtable rounds — Architect / single\n\
         agent dispatch builds on top of these without re-litigating.\n\n{}",
        lines.join("\n")
    ))
}

// ─── Layer A′ tests: brand-aware plan lookup ─────────────────────────────────

#[cfg(test)]
mod plan_isolation_tests {
    use super::*;
    use rusqlite::Connection;

    fn build_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE plans (
                id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL,
                title TEXT NOT NULL,
                description TEXT,
                phase TEXT NOT NULL DEFAULT 'design',
                slug TEXT,
                status TEXT NOT NULL DEFAULT 'active',
                implementation_branch_id TEXT,
                review_branch_id TEXT,
                updated_at INTEGER NOT NULL DEFAULT 0
             );
             CREATE TABLE branches (
                id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL
             );",
        )
        .unwrap();
        conn
    }

    fn insert_plan(
        conn: &Connection,
        id: &str,
        conv: &str,
        title: &str,
        phase: &str,
        slug: Option<&str>,
        status: &str,
        impl_branch: Option<&str>,
        review_branch: Option<&str>,
        ts: i64,
    ) {
        conn.execute(
            "INSERT INTO plans (id, conversation_id, title, description, phase, slug, status, implementation_branch_id, review_branch_id, updated_at)
             VALUES (?1, ?2, ?3, NULL, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![id, conv, title, phase, slug, status, impl_branch, review_branch, ts],
        )
        .unwrap();
    }

    /// Layer A′ INV-2: brand 진입 시 brand_id 매핑 plan 만 노출. main conv 의
    /// 다른 active plan 이 누출되지 않는다.
    #[test]
    fn brand_lookup_isolates_developer_plan() {
        let conn = build_db();
        // main conv 에 두 plan 이 있고, 각각 자기 impl_branch 를 갖는다.
        // 사용자 보고 케이스: Coder Claude → readme-memento, Codex → role-adapter.
        // 두 plan 다 status='active' 이지만 main conv 는 plan-A (가장 최근).
        insert_plan(&conn, "p-readme", "conv-main", "readme-memento", "implementation", Some("readme-memento"), "active", Some("br-readme"), None, 100);
        insert_plan(&conn, "p-role",   "conv-main", "Role Adapter Phase 1", "implementation", Some("role-adapter"), "active", Some("br-role"), None, 200);

        // brand 진입 시각각의 plan 만 보여야 한다.
        let r = lookup_plan_for_conversation(&conn, "branch:br-readme").unwrap();
        assert_eq!(r.1, "readme-memento", "Coder Claude 의 brand 는 readme-memento plan 만 본다");

        let r = lookup_plan_for_conversation(&conn, "branch:br-role").unwrap();
        assert_eq!(r.1, "Role Adapter Phase 1", "Codex 의 brand 는 role-adapter plan 만 본다");
    }

    /// Fallback: brand_id 매핑 plan 이 없으면 main conv 의 active plan 으로 graceful fallback.
    #[test]
    fn unmapped_brand_falls_back_to_main_active() {
        let conn = build_db();
        insert_plan(&conn, "p1", "conv-main", "main-plan", "design", Some("main-plan"), "active", None, None, 100);
        // branches 테이블에 br-temp 를 main conv 로 매핑 (옛 brand 또는 임시 brand)
        conn.execute("INSERT INTO branches (id, conversation_id) VALUES (?1, ?2)",
                    rusqlite::params!["br-temp", "conv-main"]).unwrap();

        let r = lookup_plan_for_conversation(&conn, "branch:br-temp").unwrap();
        assert_eq!(r.1, "main-plan", "매핑 plan 없을 때 main conv 의 active plan 으로 fallback");
    }

    /// Reviewer 가 review_branch_id 매핑으로 들어가면 그 plan 만 본다.
    #[test]
    fn review_brand_isolates_reviewer_plan() {
        let conn = build_db();
        insert_plan(&conn, "p1", "conv-main", "feature-a", "review", Some("feature-a"), "active", Some("br-impl"), Some("br-rev"), 100);
        insert_plan(&conn, "p2", "conv-main", "feature-b", "implementation", Some("feature-b"), "active", Some("br-other-impl"), None, 200);

        let r = lookup_plan_for_conversation(&conn, "branch:br-rev").unwrap();
        assert_eq!(r.1, "feature-a", "review brand 는 매핑된 plan 만 본다");
    }

    /// done/abandoned plan 은 brand 매핑 lookup 에서 제외된다 (휴면 plan 누출 방지).
    #[test]
    fn done_plans_excluded_from_brand_lookup() {
        let conn = build_db();
        insert_plan(&conn, "p1", "conv-main", "old-feature", "implementation", None, "done", Some("br-old"), None, 100);

        // brand 매핑 lookup 결과 None → fallback 으로 main 도 active 없으므로 None
        let r = lookup_plan_for_conversation(&conn, "branch:br-old");
        assert!(r.is_none(), "done plan 은 brand 매핑 lookup 에서 제외");
    }

    /// 비-brand conv (main conv 또는 일반 chat) 는 기존 동작 — 자기 conv 의 active plan.
    #[test]
    fn non_brand_uses_existing_active_lookup() {
        let conn = build_db();
        insert_plan(&conn, "p1", "conv-main", "main-plan", "design", None, "active", None, None, 100);

        let r = lookup_plan_for_conversation(&conn, "conv-main").unwrap();
        assert_eq!(r.1, "main-plan");
    }
}

// ─── RT consensus section tests (devbug #263 Task 04) ─────────────────────
#[cfg(test)]
mod rt_consensus_tests {
    use super::*;
    use rusqlite::Connection;

    fn build_db_with_rt() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE conversations (id TEXT PRIMARY KEY);
             CREATE TABLE branches (
                id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL
             );
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
             INSERT INTO conversations (id) VALUES ('conv-main');
             INSERT INTO conversations (id) VALUES ('conv-other');",
        )
        .unwrap();
        conn
    }

    /// Architect dispatch 가 받는 ContextPack 에 RT 누적 합의가 *"## Roundtable
    /// Consensus"* 섹션으로 등장 — Plan §3 Task 04 의 핵심 회귀 가드.
    #[test]
    fn architect_context_pack_includes_consensus_section() {
        let conn = build_db_with_rt();
        // 라운드 1, 2 에서 axis 별 합의 누적
        conn.execute_batch(
            "INSERT INTO roundtable_consensus
                (id, conversation_id, round_index, axis, decision, participants, confidence, created_at)
              VALUES
                ('c1', 'conv-main', 1, 'compression', 'Lite/Standard/Full automode', '[\"claude\",\"codex\"]', 0.9, 100),
                ('c2', 'conv-main', 2, 'budget', 'dynamic per-section budget', '[\"gemini\"]', 0.85, 200);",
        ).unwrap();

        let result = build_rt_consensus_section(&conn, "conv-main").unwrap();
        assert!(result.starts_with("## Roundtable Consensus"));
        assert!(result.contains("**R1** **compression**"));
        assert!(result.contains("Lite/Standard/Full automode"));
        assert!(result.contains("**R2** **budget**"));
        assert!(result.contains("by claude, codex"));
        assert!(result.contains("by gemini"));
    }

    /// 빈 결과 (RT 미진행 / 다른 conv) → None — Architect ContextPack 에서
    /// 섹션 자체 skip (INV-RTC-7/8 fast path).
    #[test]
    fn empty_consensus_returns_none() {
        let conn = build_db_with_rt();
        // conv-other 는 합의 row 없음
        let result = build_rt_consensus_section(&conn, "conv-other");
        assert!(result.is_none());
    }

    /// 다른 conversation 의 합의가 누설되지 않음 — INV-RTC-7 격리.
    #[test]
    fn consensus_isolated_per_conversation() {
        let conn = build_db_with_rt();
        conn.execute_batch(
            "INSERT INTO roundtable_consensus
                (id, conversation_id, round_index, axis, decision, participants, confidence, created_at)
              VALUES
                ('c1', 'conv-main',  1, 'A', 'main A', '[]', 0.8, 100),
                ('c2', 'conv-other', 1, 'B', 'other B', '[]', 0.8, 100);",
        )
        .unwrap();

        let main_result = build_rt_consensus_section(&conn, "conv-main").unwrap();
        assert!(main_result.contains("**A**"));
        assert!(!main_result.contains("**B**"));

        let other_result = build_rt_consensus_section(&conn, "conv-other").unwrap();
        assert!(other_result.contains("**B**"));
        assert!(!other_result.contains("**A**"));
    }

    /// branch shadow conv 의 RT 도 부모 conv 검색에 포함 — branch 에서 RT
    /// 진행 후 main conv Architect dispatch 시 합의 인계.
    #[test]
    fn consensus_includes_branch_shadow_conversations() {
        let conn = build_db_with_rt();
        conn.execute_batch(
            "INSERT INTO branches (id, conversation_id) VALUES ('br-1', 'conv-main');
             INSERT INTO conversations (id) VALUES ('branch:br-1');
             INSERT INTO roundtable_consensus
                (id, conversation_id, round_index, axis, decision, participants, confidence, created_at)
              VALUES
                ('c1', 'branch:br-1', 1, 'branch-axis', 'branch decision', '[]', 0.7, 100);",
        )
        .unwrap();

        let result = build_rt_consensus_section(&conn, "conv-main").unwrap();
        assert!(result.contains("**branch-axis**"));
    }
}
