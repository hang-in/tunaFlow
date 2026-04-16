//! Conventions Context Sync (Phase 1).
//!
//! ContextPack의 정적 layer를 conventions 파일(CLAUDE.md/AGENTS.md/GEMINI.md)로
//! 영속화한다. DB(`project_conventions` 테이블)가 SSOT이고 파일은 derived/cache.
//!
//! # 마커 컨벤션
//!
//! ```markdown
//! <!-- tunaflow:managed:start -->
//! @.tunaflow/conventions/platform.md
//! @.tunaflow/conventions/persona-architect.md
//! ...
//! <!-- tunaflow:managed:end -->
//!
//! [사용자 영역 — tunaflow가 절대 건드리지 않음]
//! ```
//!
//! 마커 사이만 우리가 갱신. 마커가 없으면 파일 끝에 추가. 사용자 영역은 보존.
//!
//! # 파일 구조
//!
//! - `<project_root>/CLAUDE.md` — claude용 진입점 (마커 영역만)
//! - `<project_root>/AGENTS.md` — codex/opencode 공유 진입점
//! - `<project_root>/GEMINI.md` — gemini용 진입점
//! - `<project_root>/.tunaflow/conventions/<layer>[-<persona>].md` — 실제 본문
//!   - 진입점에서 `@` 임포트로 인라인됨 (claude/codex 모두 지원)
//!
//! # 크기 가드
//!
//! 각 split 파일 max ~10k chars. 초과 시 truncate + warning. 진입점 자체는 50줄 미만
//! (포인터만).
//!
//! # 호출 시점
//!
//! - 명시 (set_convention API): UI 설정 변경 시 즉시 sync
//! - lazy (send 직전): prepare_engine_run에서 cheap check (`is_dirty`) 후 sync 필요 시만

use std::path::{Path, PathBuf};
use std::fs;
use std::io::Write;

use rusqlite::{params, Connection};
use serde::Serialize;
use tauri::State;

use crate::errors::AppError;
use crate::db::DbState;

const MARKER_START: &str = "<!-- tunaflow:managed:start -->";
const MARKER_END: &str = "<!-- tunaflow:managed:end -->";
const MAX_LAYER_FILE_CHARS: usize = 10_000;
const SPLIT_DIR: &str = ".tunaflow/conventions";

// ─── DB layer ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConventionRow {
    pub id: i64,
    pub layer: String,
    pub persona_label: Option<String>,
    pub content: String,
    pub source_id: Option<String>,
    pub revision: i64,
    pub updated_at: i64,
}

/// 특정 project + (선택) persona에 적용될 conventions 행들을 반환.
/// persona가 Some이면 (공통 row) + (해당 persona row) 둘 다 가져옴.
/// DB 저장 형태는 빈 문자열 sentinel — 반환 시 None으로 환원.
pub fn list_conventions(
    conn: &Connection,
    project_key: &str,
    persona_label: Option<&str>,
) -> Result<Vec<ConventionRow>, AppError> {
    let sql = match persona_label {
        Some(_) => "SELECT id, layer, persona_label, content, source_id, revision, updated_at
                    FROM project_conventions
                    WHERE project_key = ?1 AND (persona_label = '' OR persona_label = ?2)
                    ORDER BY layer, persona_label, source_id, id",
        None => "SELECT id, layer, persona_label, content, source_id, revision, updated_at
                 FROM project_conventions
                 WHERE project_key = ?1 AND persona_label = ''
                 ORDER BY layer, source_id, id",
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = if let Some(p) = persona_label {
        stmt.query_map(params![project_key, p], row_to_convention)?
            .collect::<Result<Vec<_>, _>>()?
    } else {
        stmt.query_map(params![project_key], row_to_convention)?
            .collect::<Result<Vec<_>, _>>()?
    };
    Ok(rows)
}

fn row_to_convention(row: &rusqlite::Row) -> rusqlite::Result<ConventionRow> {
    let pl: String = row.get(2)?;
    let sid: String = row.get(4)?;
    Ok(ConventionRow {
        id: row.get(0)?,
        layer: row.get(1)?,
        persona_label: if pl.is_empty() { None } else { Some(pl) },
        content: row.get(3)?,
        source_id: if sid.is_empty() { None } else { Some(sid) },
        revision: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

/// upsert — (project_key, layer, persona_label, source_id) 키 기준으로 갱신.
/// revision이 더 작거나 같으면 갱신 안 함 (idempotent).
/// None ↔ "" 변환은 여기서 처리.
pub fn upsert_convention(
    conn: &Connection,
    project_key: &str,
    layer: &str,
    persona_label: Option<&str>,
    source_id: Option<&str>,
    content: &str,
    revision: i64,
) -> Result<(), AppError> {
    let now = crate::db::migrations::now_epoch_ms();
    let pl = persona_label.unwrap_or("");
    let sid = source_id.unwrap_or("");
    conn.execute(
        "INSERT INTO project_conventions
            (project_key, layer, persona_label, content, source_id, revision, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(project_key, layer, persona_label, source_id) DO UPDATE SET
            content = excluded.content,
            revision = excluded.revision,
            updated_at = excluded.updated_at
         WHERE excluded.revision >= project_conventions.revision",
        params![project_key, layer, pl, content, sid, revision, now],
    )?;
    Ok(())
}

/// 특정 (project, layer, persona, source) 행 삭제.
pub fn delete_convention(
    conn: &Connection,
    project_key: &str,
    layer: &str,
    persona_label: Option<&str>,
    source_id: Option<&str>,
) -> Result<(), AppError> {
    let pl = persona_label.unwrap_or("");
    let sid = source_id.unwrap_or("");
    conn.execute(
        "DELETE FROM project_conventions
         WHERE project_key = ?1 AND layer = ?2 AND persona_label = ?3 AND source_id = ?4",
        params![project_key, layer, pl, sid],
    )?;
    Ok(())
}

/// project의 conventions 갱신 시각 중 최댓값. file sync 필요성 판단(dirty check)에 사용.
pub fn last_updated(conn: &Connection, project_key: &str) -> Result<i64, AppError> {
    let v: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(updated_at), 0) FROM project_conventions WHERE project_key = ?1",
            params![project_key],
            |row| row.get(0),
        )
        .unwrap_or(0);
    Ok(v)
}

// ─── File sync layer ────────────────────────────────────────────────────────

/// 엔진별 conventions 진입점 파일명.
fn entry_file_for_engine(engine: &str) -> &'static str {
    match engine {
        "claude" | "claude-code" => "CLAUDE.md",
        "gemini" => "GEMINI.md",
        // codex, opencode 등은 AGENTS.md 공유
        _ => "AGENTS.md",
    }
}

/// project 내 split 파일 디렉토리 절대 경로.
fn split_dir(project_root: &Path) -> PathBuf {
    project_root.join(SPLIT_DIR)
}

/// 한 layer의 split 파일 경로. persona가 있으면 `<layer>-<persona>.md`.
fn layer_file_path(project_root: &Path, layer: &str, persona: Option<&str>) -> PathBuf {
    let name = match persona {
        Some(p) => format!("{}-{}.md", layer, sanitize_segment(p)),
        None => format!("{}.md", layer),
    };
    split_dir(project_root).join(name)
}

/// 파일명에 안전한 segment로 변환 (공백 → -, 영숫자/하이픈/_ 외 제거).
fn sanitize_segment(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' { c }
            else if c.is_whitespace() { '-' }
            else { '_' }
        })
        .collect::<String>()
        .to_lowercase()
}

/// 사용자 영역을 보존하면서 진입점 파일의 마커 영역만 교체한다.
/// 마커가 없으면 파일 끝에 새 마커 블록을 추가.
pub fn write_managed_section(file_path: &Path, managed_body: &str) -> Result<(), AppError> {
    let original = fs::read_to_string(file_path).unwrap_or_default();
    let new_block = format!("{}\n{}\n{}", MARKER_START, managed_body.trim(), MARKER_END);

    let updated = if let (Some(start), Some(end)) = (original.find(MARKER_START), original.find(MARKER_END)) {
        // 마커 끝 위치를 marker_end 다음 줄까지 포함해야 깨끗 — end는 marker 시작 인덱스이므로 + len.
        let end_full = end + MARKER_END.len();
        let mut s = String::with_capacity(original.len() + new_block.len());
        s.push_str(&original[..start]);
        s.push_str(&new_block);
        s.push_str(&original[end_full..]);
        s
    } else {
        // 마커 없음 — 파일 끝에 추가. 기존 trailing newline 정리.
        let trimmed = original.trim_end();
        if trimmed.is_empty() {
            new_block + "\n"
        } else {
            format!("{}\n\n{}\n", trimmed, new_block)
        }
    };

    // atomic write — temp file → rename
    if let Some(parent) = file_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| AppError::Agent(format!("conventions: mkdir {}: {}", parent.display(), e)))?;
        }
    }
    let tmp = file_path.with_extension(format!(
        "{}.tmp",
        file_path.extension().and_then(|s| s.to_str()).unwrap_or("md")
    ));
    {
        let mut f = fs::File::create(&tmp)
            .map_err(|e| AppError::Agent(format!("conventions: create tmp {}: {}", tmp.display(), e)))?;
        f.write_all(updated.as_bytes())
            .map_err(|e| AppError::Agent(format!("conventions: write tmp: {}", e)))?;
        f.sync_all().ok();
    }
    fs::rename(&tmp, file_path)
        .map_err(|e| AppError::Agent(format!("conventions: rename {}→{}: {}", tmp.display(), file_path.display(), e)))?;
    Ok(())
}

/// split 파일 작성. 크기 가드 적용 (max 10k chars 초과 시 truncate + 경고).
fn write_split_file(file_path: &Path, body: &str) -> Result<bool, AppError> {
    let (final_body, truncated) = if body.len() > MAX_LAYER_FILE_CHARS {
        let mut end = MAX_LAYER_FILE_CHARS;
        while end > 0 && !body.is_char_boundary(end) {
            end -= 1;
        }
        let truncated_body = format!(
            "{}\n\n<!-- tunaflow: truncated — {} chars exceeded {} limit -->\n",
            &body[..end], body.len(), MAX_LAYER_FILE_CHARS
        );
        (truncated_body, true)
    } else {
        (body.to_string(), false)
    };

    if let Some(parent) = file_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)
                .map_err(|e| AppError::Agent(format!("conventions: mkdir {}: {}", parent.display(), e)))?;
        }
    }
    let tmp = file_path.with_extension("md.tmp");
    {
        let mut f = fs::File::create(&tmp)
            .map_err(|e| AppError::Agent(format!("conventions: create split tmp {}: {}", tmp.display(), e)))?;
        f.write_all(final_body.as_bytes())
            .map_err(|e| AppError::Agent(format!("conventions: write split tmp: {}", e)))?;
        f.sync_all().ok();
    }
    fs::rename(&tmp, file_path)
        .map_err(|e| AppError::Agent(format!("conventions: rename split {}→{}: {}", tmp.display(), file_path.display(), e)))?;
    Ok(truncated)
}

/// project + persona에 대해 conventions 파일 전부를 sync한다.
/// - DB에서 활성 행을 layer/persona별로 그룹화
/// - 각 그룹을 split 파일로 write
/// - 진입점 파일(CLAUDE.md/AGENTS.md/GEMINI.md)의 마커 영역에 `@` 임포트 라인을 채움
///
/// engines: 진입점 파일을 만들 엔진 목록 (예: ["claude", "codex", "gemini"])
/// 일반적으로 활성화된 모든 엔진을 포함하지만, 비용 절약을 위해 호출자가 좁힐 수 있다.
pub fn sync_to_files(
    conn: &Connection,
    project_root: &Path,
    project_key: &str,
    persona_label: Option<&str>,
    engines: &[&str],
) -> Result<SyncReport, AppError> {
    let rows = list_conventions(conn, project_key, persona_label)?;

    // (layer, persona, source_id) 단위로 합쳐 split 파일을 쓰지만,
    // 진입점에서는 layer + persona 단위로 단일 import만 노출 (source는 같은 파일에 합침).
    use std::collections::BTreeMap;
    type Key = (String, Option<String>); // (layer, persona)
    let mut groups: BTreeMap<Key, Vec<&ConventionRow>> = BTreeMap::new();
    for r in &rows {
        groups.entry((r.layer.clone(), r.persona_label.clone()))
            .or_default()
            .push(r);
    }

    let mut split_paths: Vec<PathBuf> = Vec::new();
    let mut truncated_files: Vec<PathBuf> = Vec::new();

    for ((layer, persona), items) in &groups {
        // 같은 (layer, persona)의 모든 source를 한 파일에 합침. source_id별로 섹션 분리.
        let mut body = String::new();
        body.push_str(&format!("<!-- tunaflow auto-managed: layer={} persona={} -->\n",
            layer, persona.as_deref().unwrap_or("(common)")));
        for item in items {
            if let Some(sid) = &item.source_id {
                body.push_str(&format!("\n<!-- source_id={} revision={} -->\n", sid, item.revision));
            }
            body.push_str(item.content.trim());
            body.push_str("\n");
        }

        let split_path = layer_file_path(project_root, layer, persona.as_deref());
        let truncated = write_split_file(&split_path, &body)?;
        if truncated { truncated_files.push(split_path.clone()); }
        split_paths.push(split_path);
    }

    // 진입점 파일에 `@` 임포트 라인 작성. claude code의 `@path` 임포트와 호환.
    let managed_body = if split_paths.is_empty() {
        String::from("<!-- tunaflow: no conventions configured for this project -->")
    } else {
        let mut s = String::from("## Project conventions (managed by tunaFlow)\n\n");
        s.push_str("> 이 영역은 tunaFlow가 자동 갱신합니다. Settings에서 편집하세요.\n\n");
        for p in &split_paths {
            // 진입점 파일 기준 상대 경로
            if let Ok(rel) = p.strip_prefix(project_root) {
                s.push_str(&format!("@{}\n", rel.display()));
            }
        }
        s
    };

    let mut written_entries: Vec<PathBuf> = Vec::new();
    for engine in engines {
        let entry = project_root.join(entry_file_for_engine(engine));
        // codex와 opencode가 같은 AGENTS.md 공유 — 중복 방지
        if written_entries.contains(&entry) { continue; }
        write_managed_section(&entry, &managed_body)?;
        written_entries.push(entry);
    }

    Ok(SyncReport {
        rows_written: rows.len(),
        split_files: split_paths.len(),
        entry_files: written_entries.len(),
        truncated: truncated_files,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncReport {
    pub rows_written: usize,
    pub split_files: usize,
    pub entry_files: usize,
    pub truncated: Vec<PathBuf>,
}

// ─── Tauri commands ─────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_project_conventions(
    state: State<DbState>,
    project_key: String,
    persona_label: Option<String>,
) -> Result<Vec<ConventionRow>, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    list_conventions(&conn, &project_key, persona_label.as_deref())
}

#[tauri::command]
pub fn set_project_convention(
    state: State<DbState>,
    project_key: String,
    layer: String,
    persona_label: Option<String>,
    source_id: Option<String>,
    content: String,
    revision: Option<i64>,
) -> Result<(), AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    upsert_convention(
        &conn, &project_key, &layer,
        persona_label.as_deref(), source_id.as_deref(),
        &content, revision.unwrap_or(0),
    )
}

#[tauri::command]
pub fn delete_project_convention(
    state: State<DbState>,
    project_key: String,
    layer: String,
    persona_label: Option<String>,
    source_id: Option<String>,
) -> Result<(), AppError> {
    let conn = state.write.lock().map_err(|_| AppError::Lock)?;
    delete_convention(&conn, &project_key, &layer,
        persona_label.as_deref(), source_id.as_deref())
}

/// 진입점 파일 + split 파일들을 즉시 sync. UI에서 명시 트리거.
/// engines: ["claude", "codex", "gemini"] 등. 지정 안 하면 4종 모두.
#[tauri::command]
pub fn sync_project_conventions(
    state: State<DbState>,
    project_key: String,
    project_path: String,
    persona_label: Option<String>,
    engines: Option<Vec<String>>,
) -> Result<SyncReport, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    let default_engines = vec!["claude".to_string(), "codex".to_string(), "gemini".to_string()];
    let eng_list = engines.unwrap_or(default_engines);
    let eng_refs: Vec<&str> = eng_list.iter().map(|s| s.as_str()).collect();
    sync_to_files(&conn, Path::new(&project_path), &project_key,
        persona_label.as_deref(), &eng_refs)
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// vec0 등 외부 extension 없이 conventions 테이블만 만든 in-memory DB.
    /// migrations::run을 쓰지 않아 격리 빠름.
    fn test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE project_conventions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_key TEXT NOT NULL,
                layer TEXT NOT NULL,
                persona_label TEXT NOT NULL DEFAULT '',
                content TEXT NOT NULL,
                source_id TEXT NOT NULL DEFAULT '',
                revision INTEGER NOT NULL DEFAULT 0,
                updated_at INTEGER NOT NULL,
                UNIQUE(project_key, layer, persona_label, source_id)
            );"
        ).unwrap();
        conn
    }

    #[test]
    fn sanitize_segment_basic() {
        assert_eq!(sanitize_segment("Architect"), "architect");
        assert_eq!(sanitize_segment("Code Review"), "code-review");
        assert_eq!(sanitize_segment("Special!@#"), "special___");
    }

    #[test]
    fn entry_file_mapping() {
        assert_eq!(entry_file_for_engine("claude"), "CLAUDE.md");
        assert_eq!(entry_file_for_engine("claude-code"), "CLAUDE.md");
        assert_eq!(entry_file_for_engine("gemini"), "GEMINI.md");
        assert_eq!(entry_file_for_engine("codex"), "AGENTS.md");
        assert_eq!(entry_file_for_engine("opencode"), "AGENTS.md");
        assert_eq!(entry_file_for_engine("unknown"), "AGENTS.md");
    }

    #[test]
    fn write_managed_section_creates_when_no_marker() {
        let tmp = TempDir::new().unwrap();
        let f = tmp.path().join("CLAUDE.md");
        write_managed_section(&f, "@test").unwrap();
        let s = fs::read_to_string(&f).unwrap();
        assert!(s.contains(MARKER_START));
        assert!(s.contains("@test"));
        assert!(s.contains(MARKER_END));
    }

    #[test]
    fn write_managed_section_preserves_user_area() {
        let tmp = TempDir::new().unwrap();
        let f = tmp.path().join("CLAUDE.md");
        let initial = format!(
            "# 사용자 헤더\n\nuser-text\n\n{}\nold-managed\n{}\n\n[사용자 footer]\n",
            MARKER_START, MARKER_END
        );
        fs::write(&f, &initial).unwrap();
        write_managed_section(&f, "@new-import").unwrap();
        let s = fs::read_to_string(&f).unwrap();
        assert!(s.contains("# 사용자 헤더"), "헤더 보존");
        assert!(s.contains("user-text"), "본문 보존");
        assert!(s.contains("[사용자 footer]"), "footer 보존");
        assert!(s.contains("@new-import"), "새 managed 영역 적용");
        assert!(!s.contains("old-managed"), "이전 managed 영역 교체됨");
    }

    #[test]
    fn write_split_file_truncates_oversized() {
        let tmp = TempDir::new().unwrap();
        let f = tmp.path().join("layer.md");
        let big = "x".repeat(MAX_LAYER_FILE_CHARS + 1000);
        let truncated = write_split_file(&f, &big).unwrap();
        assert!(truncated);
        let s = fs::read_to_string(&f).unwrap();
        assert!(s.contains("truncated"));
        assert!(s.len() < big.len() + 200); // 경고 푸터 고려
    }

    #[test]
    fn sync_to_files_full_flow() {
        let tmp = TempDir::new().unwrap();
        let conn = test_db();

        let pk = "test-project";
        upsert_convention(&conn, pk, "platform", None, None,
            "## Platform\nbuild: cargo, npm", 1).unwrap();
        upsert_convention(&conn, pk, "agent_role", Some("Architect"), None,
            "## Architect role\nplan first.", 1).unwrap();

        let report = sync_to_files(&conn, tmp.path(), pk, Some("Architect"),
            &["claude", "codex", "gemini"]).unwrap();
        assert_eq!(report.rows_written, 2);
        assert_eq!(report.split_files, 2);
        assert_eq!(report.entry_files, 3);

        // 진입점 파일들 생성 확인
        assert!(tmp.path().join("CLAUDE.md").exists());
        assert!(tmp.path().join("AGENTS.md").exists());
        assert!(tmp.path().join("GEMINI.md").exists());

        // split 파일 생성 확인
        assert!(tmp.path().join(".tunaflow/conventions/platform.md").exists());
        assert!(tmp.path().join(".tunaflow/conventions/agent_role-architect.md").exists());

        // 진입점에 @ 임포트 포함 확인
        let claude_md = fs::read_to_string(tmp.path().join("CLAUDE.md")).unwrap();
        assert!(claude_md.contains("@.tunaflow/conventions/platform.md"));
        assert!(claude_md.contains("@.tunaflow/conventions/agent_role-architect.md"));
    }

    #[test]
    fn upsert_idempotent_lower_revision_ignored() {
        let conn = test_db();
        upsert_convention(&conn, "p", "platform", None, None, "v2", 2).unwrap();
        upsert_convention(&conn, "p", "platform", None, None, "v1-shouldnt-overwrite", 1).unwrap();
        let rows = list_conventions(&conn, "p", None).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].content, "v2");
        assert_eq!(rows[0].revision, 2);
    }

    #[test]
    fn list_conventions_persona_filter() {
        let conn = test_db();
        upsert_convention(&conn, "p", "platform", None, None, "common", 1).unwrap();
        upsert_convention(&conn, "p", "agent_role", Some("Architect"), None, "arch", 1).unwrap();
        upsert_convention(&conn, "p", "agent_role", Some("Reviewer"), None, "rev", 1).unwrap();

        // persona=Architect → 공통 + Architect만
        let rows = list_conventions(&conn, "p", Some("Architect")).unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|r| r.persona_label.is_none()));
        assert!(rows.iter().any(|r| r.persona_label.as_deref() == Some("Architect")));
        assert!(!rows.iter().any(|r| r.persona_label.as_deref() == Some("Reviewer")));

        // persona=None → 공통만
        let rows_common = list_conventions(&conn, "p", None).unwrap();
        assert_eq!(rows_common.len(), 1);
    }

    #[test]
    fn delete_convention_works() {
        let conn = test_db();
        upsert_convention(&conn, "p", "platform", None, None, "x", 1).unwrap();
        delete_convention(&conn, "p", "platform", None, None).unwrap();
        let rows = list_conventions(&conn, "p", None).unwrap();
        assert_eq!(rows.len(), 0);
    }
}
