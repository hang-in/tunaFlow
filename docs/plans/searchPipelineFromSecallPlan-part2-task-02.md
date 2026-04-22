# Subtask 02 — Write path 통합 & `tokenize_for_index` helper

> 상위 plan: [searchPipelineFromSecallPlan-part2.md](./searchPipelineFromSecallPlan-part2.md)

## Changed files

- `src-tauri/src/commands/search/tokenizer.rs` — `tokenize_for_index()` 공개 helper 추가 (내부 구현은 `tokenize_query_for_fts` 에 위임).
- `src-tauri/src/commands/messages.rs` — INSERT 3종 (`:129 :158 :191`) 에 `content_tokenized` 컬럼 write 추가.
- `src-tauri/src/commands/agents_helpers/send_common/persistence.rs` — INSERT `:124 :142 :221 :494` 와 finalize UPDATE `:293 :345` 에 `content_tokenized` 반영 (스트리밍 중간 UPDATE 는 제외).
- `src-tauri/src/commands/roundtable_helpers/persist.rs` — INSERT `:20 :54 :146` + UPDATE `:87` 반영.
- `src-tauri/src/commands/branches.rs` — `:566` branch copy INSERT 반영.
- `src-tauri/src/commands/agents_helpers/tool_request.rs` — `:354 :379` INSERT 반영.

## Change description

### 1. Helper 추가

`src-tauri/src/commands/search/tokenizer.rs` 말미에:

```rust
/// Index-side tokenize. Identical to `tokenize_query_for_fts` — aliasing lets
/// callers audit write-path and query-path independently.
pub fn tokenize_for_index(text: &str) -> String {
    tokenize_query_for_fts(text)
}
```

`mod.rs` 의 `pub use tokenizer::{...}` re-export 에 `tokenize_for_index` 추가.

### 2. Non-streaming INSERT (대부분 케이스)

패턴:

```rust
// before
let id = uuid();
conn.execute(
    "INSERT INTO messages (id, conversation_id, role, content, timestamp, status) VALUES (?1,?2,?3,?4,?5,?6)",
    params![id, conv, role, content, ts, status],
)?;

// after
use crate::commands::search::tokenize_for_index;
let id = uuid();
let tokenized = tokenize_for_index(&content);
conn.execute(
    "INSERT INTO messages (id, conversation_id, role, content, content_tokenized, timestamp, status) VALUES (?1,?2,?3,?4,?5,?6,?7)",
    params![id, conv, role, content, tokenized, ts, status],
)?;
```

적용 사이트:
- `messages.rs:129` (user message insert)
- `messages.rs:158, 191` (기타 직접 INSERT — role/engine 조합 확인 후 동일 패턴)
- `roundtable_helpers/persist.rs:20, 54, 146`
- `branches.rs:566`
- `tool_request.rs:354, 379`
- `send_common/persistence.rs:124, 142, 494`

### 2a. Content-only UPDATE 3 사이트 (Codex review 2026-04-22 반영)

초안은 이 사이트들을 고려하지 않아 INV-5 실효성이 깨졌다. 해결: **Rust 쪽 코드는 변경하지 않고** migration v45 의 `messages_fts_update_content` trigger (subtask-01 §2) 가 FTS 를 resync. 즉 아래 사이트들은 content_tokenized 를 채우지 않아도 trigger 가 fallback indexing 수행:

| 파일:라인 | UPDATE 문 | 조치 |
|---|---|---|
| `src-tauri/src/commands/messages.rs:221` (`update_message_status`) | `UPDATE messages SET status=?1, content=?2 WHERE id=?3` | **변경 없음**. trigger 가 content 기준 FTS resync. 이 경로는 사용자가 편집한 메시지나 agent 취소 메시지 content 치환 용. 빈도 낮음. tokenize 없이 whitespace 인덱싱이라도 기존 stale 대비 개선. |
| `src-tauri/src/commands/jobs.rs:136` (`cleanup_stale_jobs`) | `UPDATE messages SET status='error', content=CASE WHEN content='' THEN '(interrupted)' ELSE content END WHERE status='streaming'` | **변경 없음**. content 가 바뀐 경우만 trigger 발화. 값이 유지되는 CASE 에서는 `NEW.content IS NOT OLD.content` false 로 no-op. |
| `src-tauri/src/bootstrap/db.rs:19` (startup stale cleanup) | 위와 동일 패턴 | **변경 없음**. 위와 동일 이유. |

**권장 follow-up (별도 이슈)**: 이 3 사이트가 content_tokenized 를 갱신하지 않으면 `rebuild_messages_fts` 가 다음 실행 시 `WHERE content_tokenized IS NULL` 로 잡지 못해 남는다. 대응:
- (a) `UPDATE messages` 할 때 `content_tokenized = NULL` 도 SET 해 "reset" — rebuild 가 다음 실행 시 재tokenize. 간단하나 모든 사이트 touch.
- (b) `rebuild_messages_fts` 에 "재tokenize" 플래그 추가 — `WHERE content <> '' AND (content_tokenized IS NULL OR content_tokenized != expected)` 같은 강제 재tokenize 경로. 복잡.
- 본 subtask 는 (a) 를 **선택 적용** 권장 — 세 사이트 모두 `content_tokenized=NULL` 을 추가 SET 하여 rebuild-on-change 루프를 닫는다. 1 라인 추가 × 3.

```rust
// messages.rs:221 before
"UPDATE messages SET status = ?1, content = ?2 WHERE id = ?3"
// after
"UPDATE messages SET status = ?1, content = ?2, content_tokenized = NULL WHERE id = ?3"
// jobs.rs:136 / bootstrap/db.rs:19 동일 패턴으로 content_tokenized = NULL 추가
```

이 1 라인 변경 후에는 `AFTER UPDATE OF content_tokenized` trigger 도 발화 (NULL→NULL 은 no-op 처리 여부는 SQLite 문서 확인 — 안전하게는 `WHEN NEW.content_tokenized IS NOT OLD.content_tokenized` 조건 추가 필요할 수 있음).

### 3. Streaming 경로 (`send_common/persistence.rs`)

두 가지 케이스 구분:

**(a) INSERT placeholder (`:221`)**:
```rust
// before: content=""
"INSERT INTO messages(id,conversation_id,role,content,timestamp,status,engine,model,persona) VALUES (...)"

// after: content=""   content_tokenized=NULL 로 명시 — 미 tokenize.
"INSERT INTO messages(id,conversation_id,role,content,content_tokenized,timestamp,status,engine,model,persona) VALUES (?, ?, ?, ?, NULL, ?, ?, ?, ?, ?)"
```
또는 스키마 default 가 NULL 이므로 컬럼 생략해도 동일. 명시 권장 (코드 가독성).

**(b) 중간 update (content 누적 스트리밍)**:
`UPDATE messages SET content = ?` 호출 → `content_tokenized` 그대로 NULL 유지. **절대 tokenize 호출 금지** (INV-5).

**(c) finalize (`:293 :345`)**:
```rust
// before
"UPDATE messages SET content=?1, status='done', timestamp=?2 WHERE id=?3"

// after
use crate::commands::search::tokenize_for_index;
let tokenized = tokenize_for_index(&final_content);
"UPDATE messages SET content=?1, content_tokenized=?2, status='done', timestamp=?3 WHERE id=?4"
params![final_content, tokenized, ts, id]
```

error 경로 (`:345`) 도 동일. 에러 메시지도 검색 대상이므로 tokenize 수행.

### 4. RT `persist.rs:87`

`UPDATE messages SET content=?1, status=?2, timestamp=?3 WHERE id=?4` — RT finalize 경로. (c) 와 동일 방식으로 `content_tokenized` 함께 write.

## Dependencies

depends_on: [01] — `content_tokenized` 컬럼이 먼저 존재해야 함.

## Verification

- **INV-2 동치성 test (Developer review 로 추가)**:
  ```rust
  // src-tauri/src/commands/search/tokenizer.rs::tests
  #[test]
  fn tokenize_helpers_stay_in_sync() {
      for input in [
          "플랜",
          "Rust workspace",
          "seCall의 BM25 검색",
          "아키텍처를 설계한다",
          "",
          "   ",
      ] {
          assert_eq!(
              super::tokenize_for_index(input),
              super::tokenize_query_for_fts(input),
              "index/query tokenize diverged on input: {:?}", input
          );
      }
  }
  ```
  이 테스트가 실패하면 recall 이 silent 0 이 되므로 CI 에서 반드시 green 요구.
- `cargo test --lib commands::messages` — user insert 테스트. 신규 assertion: insert 직후 `SELECT content_tokenized FROM messages WHERE id=?` 가 non-null.
- `cargo test --lib commands::agents_helpers::send_common::persistence` — finalize 경로 테스트. 스트리밍 시뮬레이션:
  ```rust
  // 1) placeholder INSERT
  // 2) 3회 중간 UPDATE content (tokenized 는 NULL 유지 확인)
  // 3) finalize UPDATE (tokenized non-null + FTS match 성공 확인)
  let fts_count: i64 = conn.query_row(
      "SELECT COUNT(*) FROM messages_fts WHERE content MATCH ?1", params![tokenized_query], |r| r.get(0)
  ).unwrap();
  assert!(fts_count >= 1);
  ```
- `cargo test --lib commands::roundtable_helpers::persist`
- `cargo test --lib commands::branches`
- `cargo check` — exit 0. 컬럼 추가로 파라미터 수 틀어진 경우 컴파일 에러로 drop.
- **검증 추가**: 모든 `INSERT INTO messages` / `UPDATE messages SET content` 사이트를 grep 해 변경 누락 없는지 확인. 테스트 코드 (e.g. `vector_search/query.rs:477, 485, 533, 534, 554`) 는 기존 동작 유지 — `content_tokenized` 를 넘기지 않아도 COALESCE fallback (INV-4) 으로 작동. 테스트 수정 불필요하나 새로운 테스트에서는 tokenize 경로를 직접 검증할 것.

## Risks

- **누락 사이트**: messages 테이블 write 는 grep 으로 식별 가능하나 동적 SQL 이나 마이그레이션 스크립트에서 놓칠 수 있음. Developer 는 PR 작성 전 `rg "INSERT INTO messages\b|UPDATE messages SET content" src-tauri/src` 결과를 checklist 에 첨부.
- **Lindera latency**: 스트리밍 finalize 당 1회 호출. 실측 필요. 만약 100ms 이상이면 `spawn_blocking` 으로 이동 검토 (현재 send_common 은 이미 blocking context). `cargo bench` (없으면 `cargo test` 내 `std::time::Instant` 로 간이 측정) 권장.
- **Test DB 의 구스키마**: `vector_search/query.rs:477` 등 테스트는 `content_tokenized` 를 넘기지 않음. v45 trigger 의 COALESCE 가 fallback 을 처리하므로 **기존 테스트는 그대로 통과해야 함**. 통과하지 못하면 trigger 설계 재검토.
- **마이그레이션 미적용 DB 에서 실행 시**: 컬럼이 없는 상태에서 INSERT 가 실패. v45 apply 보장을 assert 할 위치는 `AppState::init` — 마이그레이션이 성공해야만 runtime 경로 진입.
