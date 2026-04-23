> ⚠️ **SUPERSEDED (2026-04-23)** — 본 subtask 는 구현되지 않음.
>
> Artifacts 재사용이 사용자 원 설계 의도임이 확인됨 (2026-04-23 세션). 별도 `preference_events` / `preference_snapshots` 테이블 신설 대신 `docs/plans/projectIdentityAnalysisPlan.md` 의 **subtask-01** (artifact 자동 생성 6 타입) 로 이관됨.
>
> 본 파일은 git history 보존 목적으로 유지. Developer 는 `docs/archive/plans/superseded/` 로 git mv 고려.

# Subtask 02 — Migration v46: `preference_events` + `preference_snapshots` + `agent_jobs` 확장

> 상위 plan: [userWorldviewInjectionPlan.md](./userWorldviewInjectionPlan.md)

## Changed files

- `src-tauri/src/db/migrations.rs` — `apply_v46` 함수 신규.
- `src-tauri/src/db/schema.rs` — fresh DB 스키마에도 새 테이블/컬럼 반영.
- `src-tauri/src/commands/preference_timeline.rs` (신규) — write path helper + Tauri commands.
- `src-tauri/src/db/models.rs` — `PreferenceEvent`, `PreferenceSnapshot` 구조체 + (선택) `AgentJob` 구조체 확장.
- `src-tauri/src/lib.rs` — 신규 command 등록.

## Change description

### 1. Migration v46 SQL

v45 다음 번호. INV-8 (이전 plan) 원칙 유지 — 단일 transaction.

```rust
fn apply_v46(conn: &mut Connection) -> Result<(), AppError> {
    let tx = conn.transaction()?;
    tx.execute_batch("
        -- preference_events: append-only 변곡점 로그
        CREATE TABLE IF NOT EXISTS preference_events (
            id              TEXT PRIMARY KEY,
            memory_name     TEXT NOT NULL,
            field           TEXT NOT NULL,
            stance_from     TEXT,
            stance_to       TEXT NOT NULL,
            reason_text     TEXT,
            reason_tags     TEXT,            -- JSON array
            confidence      REAL NOT NULL DEFAULT 1.0,
            source          TEXT NOT NULL,   -- 'user' | 'agent_inferred'
            changed_at      INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_preference_events_field
            ON preference_events(memory_name, field, changed_at DESC);

        -- preference_snapshots: 현재 활성 stance (events 에서 파생, resume 속도 용)
        CREATE TABLE IF NOT EXISTS preference_snapshots (
            memory_name     TEXT NOT NULL,
            field           TEXT NOT NULL,
            current_stance  TEXT NOT NULL,
            last_event_id   TEXT NOT NULL,
            updated_at      INTEGER NOT NULL,
            PRIMARY KEY (memory_name, field),
            FOREIGN KEY (last_event_id) REFERENCES preference_events(id)
        );
    ")?;

    // agent_jobs 컬럼 확장 (idempotent)
    crate::db::migrations::add_column_if_missing_tx(&tx, "agent_jobs", "priority", "INTEGER NOT NULL DEFAULT 0")?;
    crate::db::migrations::add_column_if_missing_tx(&tx, "agent_jobs", "dedupe_key", "TEXT")?;
    crate::db::migrations::add_column_if_missing_tx(&tx, "agent_jobs", "visibility", "TEXT NOT NULL DEFAULT 'visible'")?;
    tx.execute_batch("
        CREATE INDEX IF NOT EXISTS idx_agent_jobs_queue
            ON agent_jobs(priority, status, updated_at);
    ")?;

    tx.execute("INSERT INTO schema_version (version, applied_at) VALUES (46, ?1)", [now_epoch()])?;
    tx.commit()?;
    Ok(())
}
```

**INV-3** 준수: embedding 관련 테이블/컬럼은 본 migration 에 추가하지 않음.

**Sanitization 규약** (Codex round-2 review 2026-04-23 반영):
`memory_name` 과 `field` 컬럼은 **`:` (colon) 를 포함하지 않는다**. Write helper 진입부에서 validation:

```rust
pub fn validate_pref_identifier(s: &str) -> Result<(), AppError> {
    if s.contains(':') {
        return Err(AppError::Agent(format!(
            "memory_name/field must not contain ':' (got {:?})", s
        )));
    }
    Ok(())
}
```

이유: stance-conflict marker (`<!-- tunaflow:stance-conflict:<memory_name>:<field>:<rationale> -->`) 의 구분자가 `:` 이므로 두 필드에 `:` 가 들어가면 parser 가 rationale 경계를 오판. future 값 보호 목적의 스키마 레벨 규약화.

### 2. Write path helper

```rust
// src-tauri/src/commands/preference_timeline.rs
pub fn record_event(
    conn: &Connection,
    memory_name: &str,
    field: &str,
    stance_from: Option<&str>,
    stance_to: &str,
    reason_text: Option<&str>,
    reason_tags: &[&str],     // JSON serialize
    confidence: f64,
    source: EventSource,      // enum: User, AgentInferred
) -> Result<String, AppError> {
    // Codex round-3 review 반영 — colon 금지 규약을 공용 helper 진입부에서 강제.
    // 모든 write 경로 (user 명시 / agent_inferred 자동 감지 / migration backfill 등) 가
    // 이 helper 를 거치도록 설계하면 wrapper command 층에서의 validation 누락이 있어도
    // 일관된 보호가 가능.
    validate_pref_identifier(memory_name)?;
    validate_pref_identifier(field)?;

    let event_id = Uuid::new_v4().to_string();
    let now = now_epoch_ms();
    let tags_json = serde_json::to_string(reason_tags).unwrap_or_else(|_| "[]".into());

    conn.execute(
        "INSERT INTO preference_events (id, memory_name, field, stance_from, stance_to, reason_text, reason_tags, confidence, source, changed_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        params![event_id, memory_name, field, stance_from, stance_to, reason_text, tags_json, confidence, source.as_str(), now],
    )?;

    // Snapshot upsert (user source 또는 confidence >= threshold 일 때만)
    if matches!(source, EventSource::User) || confidence >= 0.9 {
        conn.execute(
            "INSERT INTO preference_snapshots (memory_name, field, current_stance, last_event_id, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(memory_name, field) DO UPDATE SET
                current_stance = excluded.current_stance,
                last_event_id = excluded.last_event_id,
                updated_at = excluded.updated_at",
            params![memory_name, field, stance_to, event_id, now],
        )?;
    }
    // agent_inferred 이고 confidence 낮으면 snapshot 업데이트 안 함 — modal 에서 사용자 확인 후 기록.
    Ok(event_id)
}

pub fn load_recent_events(conn: &Connection, limit: usize) -> Result<Vec<PreferenceEvent>, AppError> {
    // 최근 N 변곡점 — ContextPack 용
    conn.prepare("SELECT ... FROM preference_events ORDER BY changed_at DESC LIMIT ?1")?
        .query_map(params![limit as i64], row_to_event)?
        .collect::<Result<_, _>>()
        .map_err(Into::into)
}

pub fn load_snapshots(conn: &Connection) -> Result<Vec<PreferenceSnapshot>, AppError> {
    // 전체 현재 stance — session resume 시 전량 load
    conn.prepare("SELECT ... FROM preference_snapshots")?
        .query_map([], row_to_snapshot)?
        .collect::<Result<_, _>>()
        .map_err(Into::into)
}
```

### 3. Tauri commands (Settings UI 가 직접 호출할 수 있게)

```rust
#[tauri::command]
pub fn list_preference_events(
    limit: Option<usize>,
    state: State<DbState>,
) -> Result<Vec<PreferenceEvent>, AppError> { /* read lock + load_recent_events */ }

#[tauri::command]
pub fn list_preference_snapshots(
    state: State<DbState>,
) -> Result<Vec<PreferenceSnapshot>, AppError> { /* read lock + load_snapshots */ }

#[tauri::command]
pub fn record_user_preference_change(
    memory_name: String, field: String,
    stance_from: Option<String>, stance_to: String,
    reason_text: Option<String>, reason_tags: Vec<String>,
    state: State<DbState>,
) -> Result<String, AppError> {
    validate_pref_identifier(&memory_name)?;
    validate_pref_identifier(&field)?;
    let w = state.write.lock().map_err(|_| AppError::Lock)?;
    let tags: Vec<&str> = reason_tags.iter().map(AsRef::as_ref).collect();
    record_event(&w, &memory_name, &field, stance_from.as_deref(), &stance_to,
                 reason_text.as_deref(), &tags, 1.0, EventSource::User)
}

/// Codex round-1/2 review 반영 — stance-conflict modal 이 단일 snapshot 조회용.
#[tauri::command]
pub fn get_preference_snapshot(
    memory_name: String,
    field: String,
    state: State<DbState>,
) -> Result<Option<PreferenceSnapshot>, AppError> {
    let r = state.read.lock().map_err(|_| AppError::Lock)?;
    r.query_row(
        "SELECT memory_name, field, current_stance, last_event_id, updated_at
           FROM preference_snapshots
          WHERE memory_name = ?1 AND field = ?2",
        params![memory_name, field], row_to_snapshot,
    ).optional().map_err(Into::into)
}
```

## Dependencies

depends_on: 없음 (v45 까지 적용된 DB 기준).

## Verification

- `cargo test --lib db::migrations`:
  - v46 이 기존 tests 통과 (add_column_if_missing 재호출 idempotent)
  - 신규 test: `apply_v46` 후 `preference_events`, `preference_snapshots` 테이블 존재 + agent_jobs 에 priority/dedupe_key/visibility 컬럼 추가
  - 신규 test — INV-3 검증: migration DDL 에서 `embedding` / `vec0` 키워드 grep 결과 0
- `cargo test --lib commands::preference_timeline`:
  - `record_event` — user source 는 snapshot upsert, agent_inferred 저신뢰는 snapshot 변경 없음
  - `load_recent_events` — limit 적용 + 최신순 정렬
  - 동일 memory_name/field 재기록 시 upsert 동작
  - **Codex round-3 review 반영**: colon validation 강제 — `record_event(conn, "engine:prefs", "field", ...)` 시 `Err` 반환 확인. `record_event(conn, "memory", "a:b", ...)` 시 `Err`. wrapper command (`record_user_preference_change`) 를 우회해 helper 직접 호출 시에도 보호 확인.
- `cargo check` — exit 0.

## Risks

- **`add_column_if_missing` 가 transaction 과 호환**: 기존 구현이 `&Connection` 받는 경우 tx 내부에서 못 쓸 수 있음. Part 2 plan subtask-01 의 `add_column_if_missing_tx` 헬퍼를 먼저 도입하거나 재사용.
- **Migration 순번 충돌**: Part 2 plan 의 v45 가 먼저 머지돼야 v46 번호가 유효. PR 순서 확인.
- **`source` 컬럼의 TEXT vs ENUM**: SQLite 는 enum 없음. 문자열 리터럴 ('user' | 'agent_inferred') 사용 + Rust enum 으로 parse. 다른 값 들어오면 에러.
- **JSON reason_tags**: SQLite 1.38+ 의 json1 extension 필요. 현재 tunaFlow 가 이미 다른 곳에서 json 사용 중이면 OK.
- **Backward compat**: 기존 agent_jobs 에 priority 0 default — 기존 foreground 동작 영향 없음. visibility='visible' default 도 기존 동작 유지.
