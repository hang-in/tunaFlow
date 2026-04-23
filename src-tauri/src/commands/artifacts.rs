use rusqlite::{params, Connection};
use serde::Deserialize;
use tauri::State;
use uuid::Uuid;

use crate::db::{migrations::now_epoch_ms, models::{Artifact, ArtifactKind}, DbState};
use crate::errors::AppError;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateArtifactInput {
    pub conversation_id: Option<String>,
    pub branch_id: Option<String>,
    pub subtask_id: Option<String>,
    pub plan_id: Option<String>,
    #[serde(rename = "type")]
    pub artifact_type: String,
    pub title: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateArtifactStatusInput {
    pub id: String,
    pub status: String,
}

fn map_row(row: &rusqlite::Row) -> rusqlite::Result<Artifact> {
    Ok(Artifact {
        id: row.get(0)?,
        conversation_id: row.get(1)?,
        branch_id: row.get(2)?,
        subtask_id: row.get(3)?,
        plan_id: row.get(4)?,
        artifact_type: row.get(5)?,
        title: row.get(6)?,
        content: row.get(7)?,
        status: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

const SELECT_COLS: &str =
    "id, conversation_id, branch_id, subtask_id, plan_id, type, title, content, status, created_at, updated_at";

#[tauri::command]
pub fn list_artifacts(
    conversation_id: String,
    state: State<DbState>,
) -> Result<Vec<Artifact>, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    let sql = format!(
        "SELECT {} FROM artifacts WHERE conversation_id = ?1 ORDER BY updated_at DESC",
        SELECT_COLS
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map([&conversation_id], map_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[tauri::command]
pub fn list_artifacts_by_branch(
    branch_id: String,
    state: State<DbState>,
) -> Result<Vec<Artifact>, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    let sql = format!(
        "SELECT {} FROM artifacts WHERE branch_id = ?1 ORDER BY updated_at DESC",
        SELECT_COLS
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map([&branch_id], map_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

#[tauri::command]
pub fn create_artifact(
    input: CreateArtifactInput,
    state: State<DbState>,
) -> Result<Artifact, AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    let id = Uuid::new_v4().to_string();
    let now = now_epoch_ms();

    // Auto-resolve plan_id from active plan if not provided
    let plan_id = input.plan_id.or_else(|| {
        // Try to find active plan for this conversation (or parent conversation for branches)
        let conv_id = input.conversation_id.as_deref()?;
        // For branch shadow conversations, look up the parent conversation's plan
        let lookup_conv = if conv_id.starts_with("branch:") {
            let branch_id = conv_id.strip_prefix("branch:")?;
            conn.query_row(
                "SELECT conversation_id FROM branches WHERE id = ?1",
                [branch_id], |row| row.get::<_, String>(0),
            ).ok()
        } else {
            Some(conv_id.to_string())
        };
        let lookup = lookup_conv.as_deref()?;
        conn.query_row(
            "SELECT id FROM plans WHERE conversation_id = ?1 AND status = 'active' ORDER BY updated_at DESC LIMIT 1",
            [lookup], |row| row.get::<_, String>(0),
        ).ok()
    });

    conn.execute(
        "INSERT INTO artifacts (id, conversation_id, branch_id, subtask_id, plan_id, type, title, content, status, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'draft', ?9, ?10)",
        params![
            id,
            input.conversation_id,
            input.branch_id,
            input.subtask_id,
            plan_id,
            input.artifact_type,
            input.title,
            input.content,
            now,
            now,
        ],
    )?;

    Ok(Artifact {
        id,
        conversation_id: input.conversation_id,
        branch_id: input.branch_id,
        subtask_id: input.subtask_id,
        plan_id,
        artifact_type: input.artifact_type,
        title: input.title,
        content: input.content,
        status: "draft".into(),
        created_at: now,
        updated_at: now,
    })
}

#[tauri::command]
pub fn update_artifact_status(
    input: UpdateArtifactStatusInput,
    state: State<DbState>,
) -> Result<(), AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    let now = now_epoch_ms();
    conn.execute(
        "UPDATE artifacts SET status = ?1, updated_at = ?2 WHERE id = ?3",
        params![input.status, now, input.id],
    )?;
    Ok(())
}

/// Link an existing artifact to a plan subtask.
/// This is the minimal "weak link" between plan outcomes and artifacts.
#[tauri::command]
pub fn link_artifact_to_subtask(
    artifact_id: String,
    subtask_id: String,
    state: State<DbState>,
) -> Result<(), AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    let now = now_epoch_ms();
    conn.execute(
        "UPDATE artifacts SET subtask_id = ?1, updated_at = ?2 WHERE id = ?3",
        params![subtask_id, now, artifact_id],
    )?;
    Ok(())
}

#[tauri::command]
pub fn delete_artifact(id: String, state: State<DbState>) -> Result<(), AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    conn.execute("DELETE FROM artifacts WHERE id = ?1", [&id])?;
    Ok(())
}

/// Artifact 단건 조회 — Insight Identity 뷰에서 artifact_refs 펼칠 때 사용.
#[tauri::command]
pub fn get_artifact(id: String, state: State<DbState>) -> Result<Artifact, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    let sql = format!("SELECT {} FROM artifacts WHERE id = ?1", SELECT_COLS);
    conn.query_row(&sql, [&id], map_row).map_err(|e| e.into())
}

/// project 별 `identity_summary` artifact list (최신순). frontmatter `project_key`
/// 를 LIKE 매칭. subtask-04 IdentityView 가 history 렌더 + 전환용.
#[tauri::command]
pub fn list_identity_summaries(
    project_key: String,
    state: State<DbState>,
) -> Result<Vec<Artifact>, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    let pattern = format!("%project_key: {}\n%", project_key);
    let sql = format!(
        "SELECT {} FROM artifacts \
         WHERE type = 'identity_summary' AND content LIKE ?1 \
         ORDER BY created_at DESC",
        SELECT_COLS
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map([&pattern], map_row)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

// ─── Identity summary (subtask-03, analyzer 전용 경로) ────────────────────

/// 분석 output artifact 를 저장하는 analyzer 전용 헬퍼. `create_identity_input_artifact`
/// 의 INV-1 enforcement (IdentitySummary kind 거부) 를 우회한다.
/// 호출 site 는 `agents::identity_analyzer` 모듈에 한정 (crate-private).
pub(crate) fn create_identity_summary(
    conn: &Connection,
    project_key: &str,
    title: &str,
    content: &str,
) -> Result<String, AppError> {
    let id = format!("identity-{}-{}", project_key, now_epoch_ms());
    let now = now_epoch_ms();
    conn.execute(
        "INSERT INTO artifacts (id, conversation_id, branch_id, subtask_id, plan_id, type, title, content, status, created_at, updated_at) \
         VALUES (?1, NULL, NULL, NULL, NULL, 'identity_summary', ?2, ?3, 'final', ?4, ?4)",
        params![id, title, content, now],
    )?;
    Ok(id)
}

/// ContextPack 주입 용 — project 별 최신 `identity_summary` artifact 를 가져온다.
/// frontmatter 의 `project_key: <key>` 를 LIKE 매칭. subtask 당 월 수개 수준이라
/// 풀스캔 허용.
pub(crate) fn fetch_latest_identity_summary(
    conn: &Connection,
    project_key: &str,
) -> Result<Option<Artifact>, AppError> {
    let pattern = format!("%project_key: {}\n%", project_key);
    let row = conn
        .query_row(
            "SELECT id, conversation_id, branch_id, subtask_id, plan_id, type, title, content, status, created_at, updated_at \
             FROM artifacts \
             WHERE type = 'identity_summary' AND content LIKE ?1 \
             ORDER BY created_at DESC LIMIT 1",
            [&pattern],
            map_row,
        )
        .ok();
    Ok(row)
}

// ─── Identity input artifacts (projectIdentityAnalysisPlan subtask-01) ───────

/// Dedup window (ms) — 같은 `(conversation_id, type, content-hash)` 튜플이 이 이내
/// 재시도되면 fat-finger 로 간주하고 skip. 1 분.
const IDENTITY_ARTIFACT_DEDUP_WINDOW_MS: i64 = 60_000;

/// 정체성 분석 입력 artifact 를 생성하는 내부 헬퍼.
///
/// **INV-1 enforcement**: `IdentitySummary` kind 는 이 경로로 만들 수 없다 — output
/// 전용이라 subtask-03 의 분석기가 별도 경로로 write 한다. 대화 내용 파싱으로
/// 감정 추론하지 않고, 워크플로 "이벤트 발생 시점" 에만 호출되어야 한다.
///
/// **Dedup**: 1 분 이내 같은 (conversation, type, content) 가 존재하면 skip 후
/// `Ok(None)` 반환. caller 는 새 생성 여부로 필요시 분기.
///
/// content 는 caller 가 구조화된 JSON value 로 전달 — `serde_json::Value` 를
/// 그대로 저장 (text 컬럼 에 stringify).
pub fn create_identity_input_artifact(
    conn: &Connection,
    kind: ArtifactKind,
    conversation_id: Option<&str>,
    plan_id: Option<&str>,
    subtask_id: Option<&str>,
    title: &str,
    content_json: serde_json::Value,
) -> Result<Option<String>, AppError> {
    if !kind.is_identity_input() {
        return Err(AppError::BadRequest(
            "create_identity_input_artifact rejects IdentitySummary kind".into(),
        ));
    }

    let content = content_json.to_string();
    let now = now_epoch_ms();

    // Dedup — 1 분 이내 동일 (conv, type, content) 존재 시 skip
    let cutoff = now - IDENTITY_ARTIFACT_DEDUP_WINDOW_MS;
    let existing: Option<String> = conn
        .query_row(
            "SELECT id FROM artifacts \
             WHERE type = ?1 \
               AND (conversation_id IS ?2 OR (conversation_id IS NULL AND ?2 IS NULL)) \
               AND content = ?3 \
               AND created_at >= ?4 \
             LIMIT 1",
            params![kind.as_str(), conversation_id, &content, cutoff],
            |row| row.get(0),
        )
        .ok();
    if let Some(prior) = existing {
        eprintln!(
            "[identity-artifact] dedup skip kind={} conv={:?} prior_id={}",
            kind.as_str(),
            conversation_id,
            prior
        );
        return Ok(None);
    }

    let id = format!("art-{}", Uuid::new_v4());
    conn.execute(
        "INSERT INTO artifacts (id, conversation_id, branch_id, subtask_id, plan_id, type, title, content, status, created_at, updated_at) \
         VALUES (?1, ?2, NULL, ?3, ?4, ?5, ?6, ?7, 'draft', ?8, ?8)",
        params![
            id,
            conversation_id,
            subtask_id,
            plan_id,
            kind.as_str(),
            title,
            content,
            now,
        ],
    )?;
    Ok(Some(id))
}

// ─── Tauri command wrapper (프론트엔드 호출용) ───────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateIdentityArtifactInput {
    /// `decision` | `review_outcome` | `rework_reason` | `finding_success`
    /// | `finding_failure` | `workflow_milestone`. `identity_summary` 는 거부.
    pub kind: String,
    pub conversation_id: Option<String>,
    pub plan_id: Option<String>,
    pub subtask_id: Option<String>,
    pub title: String,
    /// Arbitrary JSON value — 구조는 kind 별로 plan 에 정의됨.
    pub content: serde_json::Value,
}

#[tauri::command]
pub fn create_identity_artifact(
    input: CreateIdentityArtifactInput,
    state: State<DbState>,
) -> Result<Option<String>, AppError> {
    let kind = ArtifactKind::from_str(&input.kind)
        .ok_or_else(|| AppError::BadRequest(format!("unknown artifact kind: {}", input.kind)))?;
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    create_identity_input_artifact(
        &conn,
        kind,
        input.conversation_id.as_deref(),
        input.plan_id.as_deref(),
        input.subtask_id.as_deref(),
        &input.title,
        input.content,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema;

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(schema::CREATE_SCHEMA_VERSION).unwrap();
        // 최소 스키마 — artifacts 만 필요
        conn.execute_batch(
            "CREATE TABLE artifacts (
                id              TEXT PRIMARY KEY,
                conversation_id TEXT,
                branch_id       TEXT,
                subtask_id      TEXT,
                plan_id         TEXT,
                type            TEXT NOT NULL,
                title           TEXT NOT NULL,
                content         TEXT NOT NULL,
                status          TEXT NOT NULL DEFAULT 'draft',
                created_at      INTEGER NOT NULL,
                updated_at      INTEGER NOT NULL
            );",
        )
        .unwrap();
        conn
    }

    fn sample_content() -> serde_json::Value {
        serde_json::json!({"what": "plan_approved", "plan_slug": "foo-plan"})
    }

    #[test]
    fn creates_for_six_identity_input_kinds() {
        let conn = test_conn();
        let kinds = [
            ArtifactKind::Decision,
            ArtifactKind::ReviewOutcome,
            ArtifactKind::ReworkReason,
            ArtifactKind::FindingSuccess,
            ArtifactKind::FindingFailure,
            ArtifactKind::WorkflowMilestone,
        ];
        for (i, k) in kinds.iter().enumerate() {
            // 각 kind 마다 content 를 달리 해 dedup 회피
            let content = serde_json::json!({"idx": i});
            let out = create_identity_input_artifact(
                &conn, *k, Some("c1"), None, None, "t", content,
            )
            .expect("create");
            assert!(out.is_some(), "{:?} 는 생성되어야 함", k);
        }
        let cnt: i64 = conn
            .query_row("SELECT COUNT(*) FROM artifacts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(cnt, 6);
    }

    #[test]
    fn rejects_identity_summary_kind() {
        let conn = test_conn();
        let err = create_identity_input_artifact(
            &conn,
            ArtifactKind::IdentitySummary,
            Some("c1"),
            None,
            None,
            "should not happen",
            sample_content(),
        )
        .unwrap_err();
        match err {
            AppError::BadRequest(msg) => assert!(msg.contains("IdentitySummary")),
            other => panic!("expected BadRequest, got {:?}", other),
        }
    }

    #[test]
    fn dedup_skips_duplicate_within_window() {
        let conn = test_conn();
        let first = create_identity_input_artifact(
            &conn, ArtifactKind::Decision, Some("c1"), None, None, "t", sample_content(),
        )
        .unwrap();
        assert!(first.is_some());
        let second = create_identity_input_artifact(
            &conn, ArtifactKind::Decision, Some("c1"), None, None, "t", sample_content(),
        )
        .unwrap();
        assert!(second.is_none(), "같은 content 는 1 분 이내 dedup 되어야");
        let cnt: i64 = conn
            .query_row("SELECT COUNT(*) FROM artifacts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(cnt, 1);
    }

    #[test]
    fn dedup_different_content_not_skipped() {
        let conn = test_conn();
        create_identity_input_artifact(
            &conn, ArtifactKind::Decision, Some("c1"), None, None, "t",
            serde_json::json!({"a": 1}),
        )
        .unwrap();
        let second = create_identity_input_artifact(
            &conn, ArtifactKind::Decision, Some("c1"), None, None, "t",
            serde_json::json!({"a": 2}),
        )
        .unwrap();
        assert!(second.is_some(), "content 가 다르면 새 row 생성되어야");
    }

    #[test]
    fn create_identity_summary_persists_with_final_status() {
        let conn = test_conn();
        let id = create_identity_summary(
            &conn,
            "proj-x",
            "Identity — proj-x",
            "---\nproject_key: proj-x\n---\n\n### Project identity\nbody",
        )
        .unwrap();
        assert!(id.starts_with("identity-proj-x-"));
        let (typ, status): (String, String) = conn
            .query_row(
                "SELECT type, status FROM artifacts WHERE id = ?1",
                [&id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(typ, "identity_summary");
        assert_eq!(status, "final");
    }

    #[test]
    fn fetch_latest_identity_summary_returns_most_recent_for_project() {
        let conn = test_conn();
        // 두 project 혼재
        create_identity_summary(
            &conn, "proj-a", "a1",
            "---\nproject_key: proj-a\n---\n\n### Project identity\nv1",
        )
        .unwrap();
        // 살짝 시간차
        std::thread::sleep(std::time::Duration::from_millis(2));
        let id2 = create_identity_summary(
            &conn, "proj-a", "a2",
            "---\nproject_key: proj-a\n---\n\n### Project identity\nv2",
        )
        .unwrap();
        create_identity_summary(
            &conn, "proj-b", "b1",
            "---\nproject_key: proj-b\n---\n\n### Project identity\nother",
        )
        .unwrap();

        let found = fetch_latest_identity_summary(&conn, "proj-a").unwrap().unwrap();
        assert_eq!(found.id, id2, "proj-a 의 가장 최근 summary");
        assert!(found.content.contains("v2"));
    }

    #[test]
    fn fetch_latest_identity_summary_returns_none_when_absent() {
        let conn = test_conn();
        let found = fetch_latest_identity_summary(&conn, "missing-proj").unwrap();
        assert!(found.is_none());
    }

    #[test]
    fn artifact_kind_roundtrip() {
        for k in [
            ArtifactKind::Decision,
            ArtifactKind::ReviewOutcome,
            ArtifactKind::ReworkReason,
            ArtifactKind::FindingSuccess,
            ArtifactKind::FindingFailure,
            ArtifactKind::WorkflowMilestone,
            ArtifactKind::IdentitySummary,
        ] {
            assert_eq!(ArtifactKind::from_str(k.as_str()), Some(k));
        }
        assert_eq!(ArtifactKind::from_str("unknown"), None);
    }
}
