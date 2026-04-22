# Subtask 01 — Migration v45 & `messages_fts` 스키마 재정의

> 상위 plan: [searchPipelineFromSecallPlan-part2.md](./searchPipelineFromSecallPlan-part2.md)

## Changed files

- `src-tauri/src/db/migrations.rs` — `apply_v45` 함수 신규. 기존 v15 trigger DROP + `messages_fts` DROP/CREATE + 신규 trigger 3종.
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

plan §1.1 SQL 을 그대로 실행. 순서:
1. `ALTER TABLE messages ADD COLUMN content_tokenized TEXT` (idempotent: `add_column_if_missing` 헬퍼 사용)
2. `DROP TRIGGER IF EXISTS messages_fts_insert|update|delete`
3. `DROP TABLE IF EXISTS messages_fts`
4. `CREATE VIRTUAL TABLE messages_fts USING fts5(content, message_id UNINDEXED, tokenize='unicode61')`
5. 신규 trigger 3종 (plan §1.1 참조)
6. `INSERT INTO schema_version (version, applied_at) VALUES (45, ?1)`

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
- `cargo test --test db_integration` — exit 0.
- `cargo check` — exit 0.

## Risks

- **기존 user 데이터 상실 인식**: migration 직후 검색이 빈 결과를 반환 → 사용자 혼란 가능. subtask 05 의 UI 배너로 완화하되, 릴리스 노트에 명시 필요. 본 subtask 구현 시점에는 Developer 가 README 또는 CHANGELOG 에 항목 추가.
- **`DROP TABLE IF EXISTS messages_fts`** 후 `CREATE VIRTUAL TABLE` 실패 시 FTS 테이블이 없는 상태로 앱이 기동됨. `apply_v45` 는 transaction (`BEGIN ... COMMIT`) 안에서 실행해 실패 시 rollback. 다른 v* 함수와 transaction 사용 관례 확인 (기존 `apply_v*` 코드가 transaction 을 명시하지 않으면 이 subtask 에서 도입).
- **v44 (Harness Phase 3b-part1 audit, PR #124) 미머지 상태**: v45 는 v44 다음 순번. PR #124 가 먼저 머지되어야 함 — 본 PR 에서 base branch 를 #124 머지 후 main 으로 설정.
- **schema.rs 변경과 migration 의 이중성**: fresh DB (schema.rs) 와 migrated DB (migrations.rs) 가 다를 경우 회귀 버그 원천. 테스트에서 "fresh DB 에서도 `create_sql` 이 `content=messages` 를 포함하지 않아야" 확인.
