//! Meta notifications — Meta agent 알림 영속화 + 조회.
//!
//! 설계: docs/plans/metaAgentPlan.md
//! 원칙: "제안하되 결정하지 않는다" — 메타는 사용자 승인 게이트만 제공.
//!
//! DB 테이블: `meta_notifications` (v38)
//! Create/List/MarkRead/Dismiss/Clear.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;

use crate::db::{migrations::now_epoch_ms, DbState};
use crate::errors::AppError;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MetaNotificationRow {
    pub id: String,
    pub project_key: Option<String>,
    pub kind: String,
    pub title: String,
    pub summary: Option<String>,
    pub route_json: Option<String>,
    pub created_at: i64,
    pub read_at: Option<i64>,
    pub dismissed_at: Option<i64>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CreateMetaNotificationInput {
    pub project_key: Option<String>,
    pub kind: String,
    pub title: String,
    pub summary: Option<String>,
    pub route_json: Option<String>,
}

#[tauri::command]
pub fn create_meta_notification(
    input: CreateMetaNotificationInput,
    state: State<DbState>,
) -> Result<MetaNotificationRow, AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    let id = Uuid::new_v4().to_string();
    let now = now_epoch_ms();
    conn.execute(
        "INSERT INTO meta_notifications (id, project_key, kind, title, summary, route_json, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![id, input.project_key, input.kind, input.title, input.summary, input.route_json, now],
    )?;
    Ok(MetaNotificationRow {
        id,
        project_key: input.project_key,
        kind: input.kind,
        title: input.title,
        summary: input.summary,
        route_json: input.route_json,
        created_at: now,
        read_at: None,
        dismissed_at: None,
    })
}

#[tauri::command]
pub fn list_meta_notifications(
    project_key: Option<String>,
    limit: Option<i64>,
    state: State<DbState>,
) -> Result<Vec<MetaNotificationRow>, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    let lim = limit.unwrap_or(50).clamp(1, 200);
    // dismissed 제외. project_key 가 주어지면 해당 프로젝트 또는 NULL(글로벌) 포함.
    // query_map 의 결과를 한 번에 collect — stmt 수명 스코프 내에서 Vec 으로 받기.
    if let Some(pk) = project_key {
        let mut stmt = conn.prepare(
            "SELECT id, project_key, kind, title, summary, route_json, created_at, read_at, dismissed_at
             FROM meta_notifications
             WHERE dismissed_at IS NULL AND (project_key = ?1 OR project_key IS NULL)
             ORDER BY created_at DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![pk, lim], map_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    } else {
        let mut stmt = conn.prepare(
            "SELECT id, project_key, kind, title, summary, route_json, created_at, read_at, dismissed_at
             FROM meta_notifications
             WHERE dismissed_at IS NULL
             ORDER BY created_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([lim], map_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

#[tauri::command]
pub fn mark_meta_notification_read(id: String, state: State<DbState>) -> Result<(), AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    conn.execute(
        "UPDATE meta_notifications SET read_at = ?1 WHERE id = ?2 AND read_at IS NULL",
        params![now_epoch_ms(), id],
    )?;
    Ok(())
}

#[tauri::command]
pub fn mark_all_meta_notifications_read(
    project_key: Option<String>,
    state: State<DbState>,
) -> Result<(), AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    let now = now_epoch_ms();
    if let Some(pk) = project_key {
        conn.execute(
            "UPDATE meta_notifications SET read_at = ?1
             WHERE read_at IS NULL AND dismissed_at IS NULL AND (project_key = ?2 OR project_key IS NULL)",
            params![now, pk],
        )?;
    } else {
        conn.execute(
            "UPDATE meta_notifications SET read_at = ?1
             WHERE read_at IS NULL AND dismissed_at IS NULL",
            [now],
        )?;
    }
    Ok(())
}

#[tauri::command]
pub fn dismiss_meta_notification(id: String, state: State<DbState>) -> Result<(), AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    conn.execute(
        "UPDATE meta_notifications SET dismissed_at = ?1 WHERE id = ?2",
        params![now_epoch_ms(), id],
    )?;
    Ok(())
}

#[tauri::command]
pub fn clear_meta_notifications(
    project_key: Option<String>,
    state: State<DbState>,
) -> Result<(), AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    let now = now_epoch_ms();
    // 실제 DELETE 대신 dismissed_at 세팅 (나중에 복구 가능하도록)
    if let Some(pk) = project_key {
        conn.execute(
            "UPDATE meta_notifications SET dismissed_at = ?1
             WHERE dismissed_at IS NULL AND (project_key = ?2 OR project_key IS NULL)",
            params![now, pk],
        )?;
    } else {
        conn.execute(
            "UPDATE meta_notifications SET dismissed_at = ?1 WHERE dismissed_at IS NULL",
            [now],
        )?;
    }
    Ok(())
}

fn map_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<MetaNotificationRow> {
    Ok(MetaNotificationRow {
        id: r.get(0)?,
        project_key: r.get(1)?,
        kind: r.get(2)?,
        title: r.get(3)?,
        summary: r.get(4)?,
        route_json: r.get(5)?,
        created_at: r.get(6)?,
        read_at: r.get(7)?,
        dismissed_at: r.get(8)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE projects (key TEXT PRIMARY KEY, name TEXT, path TEXT);
             CREATE TABLE meta_notifications (
                id TEXT PRIMARY KEY, project_key TEXT, kind TEXT NOT NULL,
                title TEXT NOT NULL, summary TEXT, route_json TEXT,
                created_at INTEGER NOT NULL, read_at INTEGER, dismissed_at INTEGER
             );"
        ).unwrap();
        conn.execute("INSERT INTO projects(key,name,path) VALUES('p1','Test','/tmp')", []).unwrap();
        conn
    }

    #[test]
    fn insert_and_list() {
        let conn = test_conn();
        let id = "n1";
        conn.execute(
            "INSERT INTO meta_notifications(id,project_key,kind,title,created_at) VALUES(?1,?2,?3,?4,?5)",
            params![id, "p1", "review_passed", "Plan X done", 1000i64],
        ).unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, project_key, kind, title, summary, route_json, created_at, read_at, dismissed_at
             FROM meta_notifications WHERE dismissed_at IS NULL ORDER BY created_at DESC"
        ).unwrap();
        let rows: Vec<_> = stmt.query_map([], map_row).unwrap().collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].kind, "review_passed");
    }

    #[test]
    fn dismissed_rows_filtered() {
        let conn = test_conn();
        conn.execute(
            "INSERT INTO meta_notifications(id,project_key,kind,title,created_at,dismissed_at) VALUES('n1','p1','x','t',1000,2000)",
            []
        ).unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, project_key, kind, title, summary, route_json, created_at, read_at, dismissed_at
             FROM meta_notifications WHERE dismissed_at IS NULL"
        ).unwrap();
        let rows: Vec<_> = stmt.query_map([], map_row).unwrap().collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(rows.len(), 0);
    }
}
