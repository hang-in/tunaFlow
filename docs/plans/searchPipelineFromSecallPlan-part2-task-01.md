# Subtask 01 — Migration v45 & `messages_fts` 스키마 재정의

> 상위 plan: [searchPipelineFromSecallPlan-part2.md](./searchPipelineFromSecallPlan-part2.md)

## Changed files

- `src-tauri/src/db/migrations.rs` — `apply_v45` 함수 신규. 기존 v15 trigger DROP + `messages_fts` DROP/CREATE + 신규 trigger **4종** (insert / update_tokenized / update_content / delete).
- `src-tauri/src/db/schema.rs` — `messages_fts` 정의를 **standalone FTS5** (`content=messages` 제거, `tokenize='unicode61'` 명시) 로 교체. `messages` 테이블 정의에 `content_tokenized TEXT` 컬럼 추가.
- `src-tauri/src/db/migrations.rs` 의 마이그레이션 registry (`run_migrations` / `apply_all` 내부 match arm) 에 v45 엔트리 추가.

## Change description

### 1. `schema.rs`

`messages` CREATE 문에 `content_tokenized TEXT` 를 **맨 끝에** 추가 (기존 컬럼 순서 유지 — migration 과 fresh DB 의 columns 순서 일치가 조회 쉬움).

`messages_fts` CREATE 문 교체:

```sql
-- 기존 (schema.rs:273-275)
CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts
    USING fts5(content, content=messages, content_rowid=rowid);

-- 신규
CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
    content,
    message_id UNINDEXED,
    tokenize='unicode61'
);
```

### 2. `migrations.rs` — `apply_v45`

**INV-8 요구**: 전체를 하나의 transaction 으로 감싼다 (`let tx = conn.transaction()?; ... tx.commit()?;`). 중간 실패 시 rollback 되어 v44 상태로 복원.

```rust
fn apply_v45(conn: &mut Connection) -> Result<(), AppError> {
    let tx = conn.transaction()?;  // SQLite 는 FTS5 virtual table DDL 도 transactional
    add_column_if_missing_tx(&tx, "messages", "content_tokenized", "TEXT")?;  // helper 신설 또는 기존 helper 가 Connection 대신 Transaction 도 받도록
    tx.execute_batch("
        DROP TRIGGER IF EXISTS messages_fts_insert;
        DROP TRIGGER IF EXISTS messages_fts_update;
        DROP TRIGGER IF EXISTS messages_fts_delete;
        DROP TABLE IF EXISTS messages_fts;
        CREATE VIRTUAL TABLE messages_fts USING fts5(
            content, message_id UNINDEXED, tokenize='unicode61'
        );
        CREATE TRIGGER messages_fts_insert AFTER INSERT ON messages BEGIN
            INSERT INTO messages_fts(rowid, content, message_id)
            VALUES (NEW.rowid, COALESCE(NEW.content_tokenized, NEW.content), NEW.id);
        END;
        -- tokenized 가 갱신될 때 발화 (finalize/rebuild/user edit)
        CREATE TRIGGER messages_fts_update_tokenized AFTER UPDATE OF content_tokenized ON messages BEGIN
            DELETE FROM messages_fts WHERE rowid = OLD.rowid;
            INSERT INTO messages_fts(rowid, content, message_id)
            VALUES (NEW.rowid, COALESCE(NEW.content_tokenized, NEW.content), NEW.id);
        END;
        -- ★ content-only UPDATE (streaming chunk, stale-cleanup 등) 에서도 FTS resync.
        --   Lindera tokenize 는 호출하지 않고 NEW.content 를 fallback 인덱싱. INV-5 참조.
        CREATE TRIGGER messages_fts_update_content
            AFTER UPDATE OF content ON messages
            WHEN NEW.content IS NOT OLD.content
            BEGIN
            DELETE FROM messages_fts WHERE rowid = OLD.rowid;
            INSERT INTO messages_fts(rowid, content, message_id)
            VALUES (NEW.rowid, COALESCE(NEW.content_tokenized, NEW.content), NEW.id);
        END;
        CREATE TRIGGER messages_fts_delete AFTER DELETE ON messages BEGIN
            DELETE FROM messages_fts WHERE rowid = OLD.rowid;
        END;
    ")?;
    tx.execute("INSERT INTO schema_version (version, applied_at) VALUES (45, ?1)", [now_epoch()])?;
    tx.commit()?;
    Ok(())
}
```

**함수 시그니처 변경**: 기존 `apply_v*` 는 `&Connection` 을 받는다. transaction 을 쓰려면 `&mut Connection` 필요. 호출자 (migration registry) 도 mut 으로 전환. 다른 migration 과의 호환성 확인 — 기존 관례가 `conn.execute_batch("BEGIN; ...; COMMIT;")` 식 명시 BEGIN 을 쓰고 있다면 그 패턴을 따르고, 그렇지 않다면 이번 기회에 `&mut Connection + conn.transaction()` 으로 통일 (scope 초과 시 본 PR 에선 명시 BEGIN 만 사용).

**절대 금지**: 이 마이그레이션 안에서 `INSERT INTO messages_fts ... SELECT FROM messages` 수행. 앱 기동 블록 방지.

### 3. Backfill 정책

기존 row 의 `content_tokenized` 는 NULL 로 남고, FTS 는 비어있는 상태로 출발. 사용자는 Settings 에서 "Rebuild search index" 를 실행해야 과거 메시지 검색 가능 — 이 UX 는 subtask 05 에서 처리. 마이그레이션 로그에 `eprintln!("[migration v45] messages_fts rebuilt empty; invoke rebuild_messages_fts to backfill")` 1 회 남긴다.

## Dependencies

depends_on: 없음 (v44 까지 적용된 DB 기준).

## Verification

- `cargo test --lib db::migrations` — 기존 tests 통과.
- 신규 test `src-tauri/tests/db_integration.rs` (또는 `tests/migration_v45.rs`):
  ```rust
  #[test]
  fn v45_drops_external_content_and_adds_tokenized_column() {
      let conn = fresh_v44_db();
      // seed 10 messages with dummy content
      apply_v45(&conn).unwrap();
      // 1) content_tokenized 컬럼 존재 + NULL
      let null_cnt: i64 = conn.query_row(
          "SELECT COUNT(*) FROM messages WHERE content_tokenized IS NULL", [], |r| r.get(0)
      ).unwrap();
      assert_eq!(null_cnt, 10);
      // 2) messages_fts 가 비어있음
      let fts_cnt: i64 = conn.query_row("SELECT COUNT(*) FROM messages_fts", [], |r| r.get(0)).unwrap();
      assert_eq!(fts_cnt, 0);
      // 3) standalone 확인 — content= external 이 없음
      let create_sql: String = conn.query_row(
          "SELECT sql FROM sqlite_master WHERE name='messages_fts'", [], |r| r.get(0)
      ).unwrap();
      assert!(!create_sql.contains("content=messages"));
      assert!(create_sql.contains("tokenize"));
  }
  ```
- 신규 test — trigger fallback (INV-4):
  ```rust
  #[test]
  fn trigger_falls_back_to_content_when_tokenized_null() {
      // content_tokenized 없이 INSERT
      conn.execute("INSERT INTO messages (id, conversation_id, role, content, timestamp) VALUES ('m1','c1','user','hello world',0)", []).unwrap();
      let cnt: i64 = conn.query_row("SELECT COUNT(*) FROM messages_fts WHERE content MATCH 'hello'", [], |r| r.get(0)).unwrap();
      assert_eq!(cnt, 1);
  }
  ```
- 신규 test — content-only UPDATE 도 FTS resync (update_content trigger, Codex review):
  ```rust
  #[test]
  fn content_only_update_syncs_fts_without_tokenize() {
      let conn = apply_v45_on(fresh_v44_db());
      conn.execute(
          "INSERT INTO messages (id, conversation_id, role, content, content_tokenized, timestamp, status)
           VALUES ('m1','c1','user','original text', 'orig tok', 0, 'done')",
          [],
      ).unwrap();
      // content-only UPDATE (streaming chunk 또는 stale cleanup 시뮬레이션)
      conn.execute("UPDATE messages SET content = 'changed text' WHERE id = 'm1'", []).unwrap();
      // FTS 는 NEW.content_tokenized 우선이므로 'orig tok' 로 indexing 유지되어야 (tokenize 는 Rust 쪽에서만)
      let cnt: i64 = conn.query_row(
          "SELECT COUNT(*) FROM messages_fts WHERE content MATCH 'orig'", [], |r| r.get(0)
      ).unwrap();
      assert_eq!(cnt, 1, "tokenized fallback 이 남아있어야 함");
  }

  #[test]
  fn content_only_update_with_null_tokenized_uses_content_fallback() {
      let conn = apply_v45_on(fresh_v44_db());
      conn.execute(
          "INSERT INTO messages (id, conversation_id, role, content, timestamp, status)
           VALUES ('m2','c1','user','first', 0, 'streaming')",
          [],
      ).unwrap();
      conn.execute("UPDATE messages SET content = 'second version' WHERE id = 'm2'", []).unwrap();
      let cnt: i64 = conn.query_row(
          "SELECT COUNT(*) FROM messages_fts WHERE content MATCH 'second'", [], |r| r.get(0)
      ).unwrap();
      assert_eq!(cnt, 1, "NEW.content fallback 이 FTS 에 반영되어야");
  }
  ```
- 신규 test — migration atomicity (INV-8):
  ```rust
  #[test]
  fn v45_rolls_back_on_mid_migration_failure() {
      let mut conn = fresh_v44_db();
      // force 실패 시뮬레이션: apply_v45 와 동일 SQL 을 직접 실행하되 중간에 고의 에러
      let tx = conn.transaction().unwrap();
      tx.execute("DROP TRIGGER messages_fts_insert", []).unwrap();
      tx.execute("DROP TABLE messages_fts", []).unwrap();
      // 여기서 에러 — tx drop 되면 rollback
      drop(tx);
      // 검증: schema_version 은 여전히 44, messages_fts 는 여전히 존재 (external content)
      let ver: i64 = conn.query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0)).unwrap();
      assert_eq!(ver, 44);
      let sql: String = conn.query_row("SELECT sql FROM sqlite_master WHERE name='messages_fts'", [], |r| r.get(0)).unwrap();
      assert!(sql.contains("content=messages"), "rollback 이 적용되지 않음");
  }
  ```
- `cargo test --test db_integration` — exit 0.
- `cargo check` — exit 0.

## Risks

- **기존 user 데이터 상실 인식**: migration 직후 검색이 빈 결과를 반환 → 사용자 혼란 가능. subtask 05 의 UI 배너로 완화하되, 릴리스 노트에 명시 필요. 본 subtask 구현 시점에는 Developer 가 README 또는 CHANGELOG 에 항목 추가.
- **DROP/CREATE 사이 window (INV-8 참조)**: transaction 안 감쌀 시 앱 크래시하면 `messages_fts` 가 소실된 채 남음. 본 subtask 의 apply_v45 는 `conn.transaction()` 필수. 다른 v* 함수의 transaction 관례 확인 — 기존 관례가 `execute_batch("BEGIN; ...; COMMIT;")` 이라면 그 패턴과 호환. 본 PR 에서 전체 apply_v* 를 `&mut Connection + transaction()` 으로 통일할지 여부는 scope 초과 → 최소 apply_v45 만 atomic 보장.
- **v44 (Harness Phase 3b-part1 audit, PR #124) 미머지 상태**: v45 는 v44 다음 순번. PR #124 가 먼저 머지되어야 함 — 본 PR 에서 base branch 를 #124 머지 후 main 으로 설정.
- **schema.rs 변경과 migration 의 이중성**: fresh DB (schema.rs) 와 migrated DB (migrations.rs) 가 다를 경우 회귀 버그 원천. 테스트에서 "fresh DB 에서도 `create_sql` 이 `content=messages` 를 포함하지 않아야" 확인.
