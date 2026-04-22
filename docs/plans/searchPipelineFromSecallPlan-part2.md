---
title: 검색 파이프라인 Phase C Part 2 — 인덱스 측 형태소 재구축 (messages_fts rebuild)
status: planned
priority: P1
created_at: 2026-04-22
depends_on:
  - PR #127 (feat/search-pipeline-phase-c-tokenizer — LinderaKoTokenizer + query-side tokenize)
blocks:
  - Feature flag `TUNAFLOW_MORPH_QUERY=1` 을 default ON 으로 승격하는 후속 작업
related:
  - docs/plans/searchPipelineFromSecallPlan.md  # Phase A~C 상위 plan, §6.3
  - docs/plans/harnessVerificationGapPlan.md    # §5 proposer 4-section 규약
  - seCall/crates/secall-core/src/store/schema.rs
  - seCall/crates/secall-core/src/search/bm25.rs
  - src-tauri/src/commands/search/tokenizer.rs
  - src-tauri/src/commands/search/unified.rs
  - src-tauri/src/commands/messages.rs
  - src-tauri/src/db/migrations.rs             # v15 messages_fts triggers
  - src-tauri/src/db/schema.rs                 # messages_fts VIRTUAL TABLE 정의
---

# Phase C Part 2 — `messages_fts` 재구축 & 인덱스 측 형태소화

> Phase C Part 1 (PR #127) 은 **쿼리 측** `tokenize_query_for_fts()` 만 도입했다.
> 그러나 인덱스가 여전히 whitespace-tokenized 상태이므로 `TUNAFLOW_MORPH_QUERY` 를 켜면
> recall 이 오히려 떨어진다 (쿼리는 morphemes, index 는 whole words).
>
> Part 2 는 **인덱스 측** 을 재구축해 위 flag 를 안전하게 ON 할 수 있게 한다.

---

## TL;DR for Developer

1. **Migration v45** — `messages` 테이블에 `content_tokenized TEXT` 컬럼을 추가하고, `messages_fts` 를 **external-content 에서 standalone FTS5 로 교체**한다. 기존 v15 trigger 3종을 DROP 하고 `content_tokenized` 기반 trigger 3종을 재생성. 기존 row 의 FTS 데이터는 비우지 말고 그대로 둔다 (backfill 은 별도 명령에서 수행).
2. **Write path 전환** — `messages` 를 `INSERT`/`UPDATE` 하는 모든 사이트에 `content_tokenized` 동시 write 를 주입한다. Streaming 경로는 finalize 시점 1회만 tokenize 한다 (chunk 마다 Lindera 호출 금지).
3. **`rebuild_messages_fts` Tauri command** — 기존 row 의 `content_tokenized` 를 chunk 단위 (500 row) 로 채우고 진행률 이벤트를 emit. 마지막에 `INSERT INTO messages_fts(messages_fts) VALUES('rebuild')` 로 FTS 정합성 확정. 취소 신호 지원.
4. **`search_messages` 조건부 tokenize** — 이미 `search_unified` 는 `morphological_query_enabled()` 분기가 있다 (`unified.rs:76-82`). `search_messages` (messages.rs:381) 에도 같은 분기를 추가해 통합.
5. **App-level `extract_snippet`** — standalone FTS5 의 `snippet()` 은 tokenized 텍스트를 반환하므로 가독성을 잃는다. secall `bm25.rs:191` 의 `extract_snippet(content, query, max_chars)` 를 이식해 search 결과 snippet 을 **원본 `messages.content`** 에서 생성한다.
6. **Settings UI** — `Settings > Search` 섹션 (신규) 에 "Rebuild search index (Korean morphological)" 버튼 + 진행률 바 + 취소 + 완료 후 `TUNAFLOW_MORPH_QUERY` 활성화 안내.

구현 순서는 위 1→2→3→4→5→6. 1~3 은 반드시 순서 지킬 것 (trigger 변경 전에 write path 를 먼저 건드리면 NULL 제약 위반 또는 silent FTS miss 가 발생).

Feature flag `TUNAFLOW_MORPH_QUERY` 는 default OFF 를 유지하고, 본 PR 에서는 "rebuild 완료 & 플래그 ON" 이 **사용자 명시 행위** 로만 작동해야 한다 (backward compatibility).

---

## Specification

### 1. DB schema

#### 1.1 Migration v45 (`src-tauri/src/db/migrations.rs`)

```sql
-- 1) messages.content_tokenized 컬럼 추가 (NULL 허용; backfill 전까지 비어있어도 OK)
ALTER TABLE messages ADD COLUMN content_tokenized TEXT;

-- 2) 기존 v15 triggers 제거
DROP TRIGGER IF EXISTS messages_fts_insert;
DROP TRIGGER IF EXISTS messages_fts_update;
DROP TRIGGER IF EXISTS messages_fts_delete;

-- 3) 기존 messages_fts 제거 (external content 구조였음)
DROP TABLE IF EXISTS messages_fts;

-- 4) standalone FTS5 재생성 — external content 없음, tokenize 명시
CREATE VIRTUAL TABLE messages_fts USING fts5(
    content,
    message_id UNINDEXED,
    tokenize='unicode61'
);

-- 5) 새 triggers: content_tokenized 기반 (NEW.content_tokenized 우선, 없으면 NEW.content 폴백)
CREATE TRIGGER messages_fts_insert AFTER INSERT ON messages BEGIN
    INSERT INTO messages_fts(rowid, content, message_id)
    VALUES (NEW.rowid, COALESCE(NEW.content_tokenized, NEW.content), NEW.id);
END;

-- tokenize 결과가 갱신될 때 발화 (streaming finalize, rebuild, user edit 등)
CREATE TRIGGER messages_fts_update_tokenized
    AFTER UPDATE OF content_tokenized ON messages BEGIN
    DELETE FROM messages_fts WHERE rowid = OLD.rowid;
    INSERT INTO messages_fts(rowid, content, message_id)
    VALUES (NEW.rowid, COALESCE(NEW.content_tokenized, NEW.content), NEW.id);
END;

-- ★ content-only UPDATE 경로 (streaming 중간 chunk UPDATE, stale-cleanup 등) 에서도 FTS 동기화.
--   Lindera tokenize 는 호출하지 않고 NEW.content 를 fallback 으로 사용한다 (INV-5 참조).
--   streaming finalize 는 content + content_tokenized 를 동시에 UPDATE 하므로 이 trigger 와
--   update_tokenized trigger 둘 다 발화하여 FTS 가 2회 재삽입되지만 결과는 멱등.
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
```

> **설계 변경 (Codex review 2026-04-22 반영)**: 초안은 `AFTER UPDATE OF content` trigger 를 제거했으나, 실제 content-only UPDATE 사이트 (`update_message_status` messages.rs:221, `cleanup_stale_jobs` jobs.rs:136, `bootstrap/db.rs:19`) 에서 FTS 가 stale 해진다. 이를 trigger 레벨에서 resync 하되 tokenize 호출은 Rust 쪽 finalize 경로로 제한한다.

**중요**: migration 안에서 `INSERT INTO messages_fts ... SELECT FROM messages` 는 하지 않는다. 앱 기동 블로킹 방지 목적. 기존 corpus 는 별도 `rebuild_messages_fts` 명령으로 backfill.

**주의**: standalone FTS5 는 `DELETE FROM messages_fts WHERE rowid = ?` 구문을 허용한다 (external content FTS5 의 특수 `INSERT ... 'delete'` 구문과 대비). 위 trigger 는 이 단순화 구문을 사용.

#### 1.2 `src-tauri/src/db/schema.rs` 수정

기존 `CREATE VIRTUAL TABLE messages_fts USING fts5(content, content=messages, content_rowid=rowid);` (schema.rs:274) 를 신규 생성 경로에서도 동일하게 standalone 으로 교체 (fresh DB 와 migrated DB 가 동일 스키마가 되도록).

### 2. Tokenizer helper 확장

`src-tauri/src/commands/search/tokenizer.rs` 에 신규 헬퍼 추가:

```rust
/// Tokenize for index-side storage. Identical behavior to `tokenize_query_for_fts`
/// today, but separated by name so write-path callers can be audited independently
/// of query-path callers.
pub fn tokenize_for_index(text: &str) -> String {
    tokenize_query_for_fts(text)
}
```

> 이유: 인덱싱용과 쿼리용이 같은 함수를 참조하는 사실이 invariant ([INV-2] 참조). 한 쪽만 바꾸는 실수를 방지하기 위해 **이름은 분리하되 내부 구현은 공유**. 추후 정책이 분기될 여지 확보.

### 3. Write path 통합

대상 파일 (grep `INSERT INTO messages\|UPDATE messages SET content` 결과, test 파일 제외):

| 파일 | 라인 | 종류 |
|---|---|---|
| `src-tauri/src/commands/messages.rs` | 129, 158, 191 | 사용자 메시지 INSERT 3종 |
| `src-tauri/src/commands/agents_helpers/send_common/persistence.rs` | 124, 142, 221, 293, 345, 494 | 엔진 경로 INSERT/UPDATE (streaming 포함) |
| `src-tauri/src/commands/roundtable_helpers/persist.rs` | 20, 54, 87, 146 | RT 경로 INSERT/UPDATE |
| `src-tauri/src/commands/branches.rs` | 566 | branch copy INSERT |
| `src-tauri/src/commands/agents_helpers/tool_request.rs` | 354, 379 | tool-request 경로 INSERT |

#### 3.1 비-스트리밍 INSERT

최종 content 가 이미 확정된 경우 (user message, tool-request 응답, branch 복사 등):

```rust
// before
"INSERT INTO messages (id, conversation_id, role, content, timestamp, ...) VALUES (?, ?, ?, ?, ?, ...)"

// after
let content_tokenized = tokenize_for_index(&content);
"INSERT INTO messages (id, conversation_id, role, content, content_tokenized, timestamp, ...) VALUES (?, ?, ?, ?, ?, ?, ...)"
```

#### 3.2 스트리밍 INSERT + 중간 UPDATE

`persistence.rs:221` INSERT (placeholder content=`""`) → `:293` 또는 `:345` 최종 UPDATE 패턴:
- **중간 update 에서는 `content_tokenized` 를 건드리지 않는다** (Lindera 비용 회피).
- **finalize (`status='done' | 'error'`) 시점에만** `content_tokenized = tokenize_for_index(final_content)` 를 함께 UPDATE.

```rust
// finalize
let tokenized = tokenize_for_index(&final_content);
conn.execute(
    "UPDATE messages SET content = ?1, content_tokenized = ?2, status = 'done', timestamp = ?3 WHERE id = ?4",
    params![final_content, tokenized, ts, id],
)?;
```

위 UPDATE 는 **두 컬럼 동시 갱신** 이지만 `AFTER UPDATE OF content_tokenized` trigger 하나만 발화 (SQLite 는 같은 UPDATE 에서 여러 컬럼 바뀌어도 각 trigger 발화 조건은 독립 평가, 그러나 `update_content` trigger 는 존재하지 않으므로 `update_tokenized` 만 발화). 결과적으로 FTS 는 1회 sync.

#### 3.3 배치성 INSERT

`persistence.rs:494` 와 같은 다건 INSERT 는 loop 내부에서 tokenize 호출. chunk 가 크지 않다면 성능 impact 미미 (Lindera 건당 마이크로초 수준).

### 4. Rebuild command

`src-tauri/src/commands/search/rebuild.rs` (신규) + `mod.rs` re-export.

```rust
#[tauri::command]
pub async fn rebuild_messages_fts(
    app: AppHandle,
    state: State<'_, DbState>,
    cancel: State<'_, RebuildCancelFlag>,
) -> Result<RebuildSummary, AppError> {
    cancel.reset();
    tokio::task::spawn_blocking(move || { /* 아래 loop */ })
        .await
        .map_err(|e| AppError::Agent(format!("join: {e}")))?
}
```

루프 의사코드:

```
let total = SELECT COUNT(*) FROM messages WHERE content_tokenized IS NULL;
let mut done = 0;
loop {
    if cancel.load() { break; }
    // 매 chunk 마다 write lock 재획득/해제 — streaming 과의 경합 최소화
    let rows = { let w = state.write.lock()?;
                 w.prepare("SELECT rowid, content FROM messages WHERE content_tokenized IS NULL LIMIT 500")?
                  .query_map(...)?.collect::<Vec<_>>() };
    if rows.is_empty() { break; }
    let tokenized: Vec<(i64, String)> = rows.into_iter()
        .map(|(rowid, c)| (rowid, tokenize_for_index(&c))).collect();
    {
        let w = state.write.lock()?;
        let tx = w.transaction()?;
        for (rowid, t) in &tokenized {
            tx.execute("UPDATE messages SET content_tokenized = ?1 WHERE rowid = ?2", params![t, rowid])?;
        }
        tx.commit()?;
    }
    done += tokenized.len();
    app.emit("messages_fts_rebuild_progress", json!({ "done": done, "total": total }))?;
}
// FTS 일관성 확정 — external content 가 아니므로 'rebuild' 커맨드 의미가 제한적이지만,
// optimize + integrity_check 를 수행해 orphan rowid 제거.
{
    let w = state.write.lock()?;
    w.execute("INSERT INTO messages_fts(messages_fts) VALUES('optimize')", [])?;
    // integrity_check 는 corpus 가 클 때 O(N) — 단 에러 없음을 확인
    let _: i64 = w.query_row("SELECT COUNT(*) FROM messages_fts", [], |r| r.get(0))?;
}
app.emit("messages_fts_rebuild_complete", json!({ "done": done, "total": total, "canceled": cancel.load() }))?;
```

`RebuildCancelFlag` 은 `Arc<AtomicBool>` wrapper. 별도 command `cancel_rebuild_messages_fts` 가 set.

**이벤트 스펙**:
- `messages_fts_rebuild_progress` → `{ done: u64, total: u64 }`
- `messages_fts_rebuild_complete` → `{ done: u64, total: u64, canceled: bool }`
- `messages_fts_rebuild_error` → `{ error: string }` (Rust 측 panic/error 를 UI 가 catch)

### 5. Search path 전환

#### 5.1 `search_messages` (`messages.rs:381`)

현재 `effective_query` 를 곧바로 FTS MATCH 에 넘긴다. Part 1 의 morphological 분기를 동일하게 주입:

```rust
let fts_query = if crate::commands::search::morphological_query_enabled() {
    crate::commands::search::tokenize_query_for_fts(&effective_query)
} else {
    effective_query.clone()
};
// 기존 MATCH ?1 자리에 fts_query 사용
```

#### 5.2 `search_unified` (`unified.rs`)

이미 분기 있음 (Part 1). 변경 불필요. 단 주석에서 "until Phase C Part 2 rebuild" 언급을 제거한다.

### 6. App-level snippet — char-window 추출 (하이라이트 없음)

`src-tauri/src/commands/search/snippet.rs` (신규). **Codex review 2026-04-22 (2차) 반영**: 초안의 byte-offset 기반 역매핑 + `**..**` 하이라이트 마커 주입은 Unicode case expansion 에서 boundary panic 위험. 순수 char-window 추출로 단순화하고, 하이라이트는 UI 측 client-side 렌더로 위임.

```rust
pub fn extract_snippet(content: &str, query: &str, max_chars: usize) -> String {
    // 1) query 의 첫 term 을 **char-level** case-insensitive 로 content 에서 탐색 (byte offset 사용 X)
    // 2) 매칭 지점 중심 ±(max_chars/2) char window
    // 3) 양 끝 ellipsis `…`
    // 4) 매칭 없으면 앞 max_chars char
    // 5) Unicode case expansion (İ→i̇, ß→ss) 은 first-char 비교로 false negative 수용 (panic 은 없음)
}
```

**변경 적용지**:
- `search_messages` (messages.rs:381) — `snippet(messages_fts, 0, '**', '**', '…', 40)` SQL 호출을 제거, `SELECT m.content` 로 바꾸고 Rust 측에서 `extract_snippet(content, effective_query, 120)` 호출. **반환값은 하이라이트 마커를 포함하지 않는다**.
- `fts_conversation_search` (unified.rs:96) — 동일 패턴.

**UI 하이라이트**: 검색 결과 컴포넌트가 `query` 를 별도 prop 으로 받아 React 단에서 client-side `<mark>` 렌더. 기존 `**...**` markdown 경로는 해체. 상세는 Subtask 05 에서 조율.

> **비고**: secall 은 snippet 을 200 char 로 자른다. tunaFlow 기존 `…` + 40 char 는 화면 폭과 어긋나지 않게 `120` 권장 (Subtask 05 에서 Frontend 조율).

### 7. Settings UI

`src/components/settings/SearchSettings.tsx` (신규) 또는 기존 `SettingsPanel` 확장.

- 섹션 타이틀: "검색 / Search"
- 토글: "한국어 형태소 검색 활성화" — FE localStorage `tunaflow.search.morphEnabled` 에 persist. Backend 는 `SEARCH_MORPH_FLAG: AtomicBool` 로 런타임 관리. Toggle 시 `invoke('set_morphological_query_enabled', { enabled })` → AtomicBool set. 앱 재시작 후 FE startup hook 이 localStorage 값을 읽어 같은 invoke 로 AtomicBool 에 주입.
  - **DB 저장 없음** (Q-4 확정). `morphological_query_enabled()` 는 env var 우선 → AtomicBool fallback.
- 버튼: "인덱스 재구축 (Rebuild search index)" → `invoke('rebuild_messages_fts')`
- 진행률 바: `messages_fts_rebuild_progress` 이벤트 구독, `done/total * 100%`
- 취소 버튼: `invoke('cancel_rebuild_messages_fts')`
- 완료 시 토스트 + "이제 한국어 형태소 검색을 활성화할 수 있습니다"

진행률 바 구현은 `src/components/settings/DocumentIndexSettings.tsx` 등에 이미 사용 중인 패턴을 재사용. 별도 dep 도입 금지.

---

## Invariants

- **[INV-1]** Migration v45 는 `messages_fts` 를 DROP/CREATE 하지만 기존 `messages` row 는 건드리지 않는다. 앱 첫 기동 후 `rebuild_messages_fts` 실행 전까지 기존 corpus 는 검색 결과에서 사라진다 — 이 기대치를 UI 에 명시해야 한다. **이유**: migration 안에서 N 백만 row tokenize 시 기동 블록. **검증**: migration 테스트에서 v44→v45 적용 직후 `SELECT COUNT(*) FROM messages_fts` 가 0 이고 `SELECT COUNT(*) FROM messages` 는 불변인지 assert.

- **[INV-2]** `tokenize_for_index` 와 `tokenize_query_for_fts` 는 **동일 함수 본체를 공유**해야 한다 (alias 또는 동일 helper 위임). 검색 recall 은 index 와 query 가 같은 토큰 분해를 적용해야만 성립. **이유**: 둘이 분기하면 silent recall 0. **검증**: Rust unit test — 동일 입력에 대해 두 함수 출력 동치 확인.

- **[INV-3]** `rebuild_messages_fts` 는 write lock 을 **chunk 경계마다 release** 한다 (단일 long-hold 금지). **이유**: `finalize_engine_run` deadlock 사례 (work-safety.md 2026-04-22) 및 streaming 메시지 write 와의 경합. **검증**: code review — `state.write.lock()` scope 가 chunk loop body 안에 국한됨을 확인.

- **[INV-4]** 신규 `messages` INSERT 사이트에서 `content_tokenized` 를 누락한 경우, `COALESCE(NEW.content_tokenized, NEW.content)` trigger 덕분에 기존 whitespace 동작으로 **graceful fallback** 한다 (검색 miss 아님). **이유**: 점진적 write-path 마이그레이션 허용. **검증**: `cargo test` — content_tokenized 를 주지 않고 INSERT 한 뒤 `SELECT * FROM messages_fts WHERE content MATCH ?` 로 원본 content 기반 매칭 성공 확인.

- **[INV-5]** 스트리밍 중간 UPDATE (`content` 만 바뀌고 `content_tokenized` 는 NULL/고정) 경로에서 **Lindera tokenize 를 호출하지 않는다**. FTS 재삽입 자체는 `messages_fts_update_content` trigger 로 수행되며, 그 값은 `NEW.content` fallback (tokenize 결과가 아님). **이유**: chunk 당 Lindera 호출 시 CPU 폭증. trigger 는 순수 SQL 이라 tokenize 를 호출할 수단이 없으므로 이 경로는 "tokenize 없이 content 원본으로 fallback indexing". **검증**: (a) Rust 코드에서 streaming 경로의 UPDATE 문에 `tokenize_for_index` 호출이 없음을 grep 확인. (b) Integration 테스트에서 streaming 시뮬레이션 후 Lindera 호출 카운트가 finalize 시점 1회만임을 확인 (카운터 mock).

- **[INV-6]** `rebuild_messages_fts` 취소 시 이미 tokenized 된 row 는 원복하지 않는다 (idempotent). 재실행 시 `WHERE content_tokenized IS NULL` 로 skip. **이유**: 대규모 corpus 에서 재시도 비용 최소화. **검증**: 테스트 — 100건 중 50건 처리 후 취소 → 재실행 → 나머지 50건만 처리 확인.

- **[INV-7]** Feature flag `TUNAFLOW_MORPH_QUERY` 를 OFF 로 되돌리면 `search_messages` / `search_unified` 둘 다 원본 쿼리를 **그대로** FTS MATCH 에 넘긴다 (query 경로만 원복). 단 rebuilt index 가 tokenized 상태라면 원본 surface form (예: "아키텍처를") 에 대한 매칭은 miss 할 수 있다. 완전한 rollback (인덱스까지 whitespace 로 되돌림) 은 별도 `rebuild_messages_fts_whitespace` 커맨드가 필요하며 본 plan 의 scope 가 아니다. **이유**: 부분 rollback 은 "코드 변경 없이 env var 만으로 query 동작 원복" 의 실용 경로로 충분. **검증**: query 경로에서 `morphological_query_enabled()=false` 일 때 tokenize 가 호출되지 않음을 unit test 로 확인. rebuilt index 에 대한 재매칭 보장은 하지 않는다 (Developer 설명 문서화).

- **[INV-8]** Migration v45 전체는 **단일 SQLite transaction** 안에서 실행된다 (`conn.transaction()` 또는 명시 `BEGIN; ... COMMIT;`). DROP TRIGGER × 3 / DROP TABLE / CREATE VIRTUAL TABLE / CREATE TRIGGER × 3 / ALTER TABLE ADD COLUMN / INSERT INTO schema_version 모두 실패 시 rollback. **이유**: 앱이 중간에 크래시하면 `messages_fts` 가 DROP 된 채 다음 기동에서 `CREATE IF NOT EXISTS` 로 재생성되기 전까지 다른 경로가 `messages_fts` 를 참조하다 에러. SQLite 는 FTS5 virtual table DROP/CREATE 를 포함해 대부분 DDL 을 transactional 로 지원한다. **검증**: 마이그레이션 중간에 강제 panic 을 주입한 integration test — panic 후 재기동 시 schema_version=44, `messages_fts` 가 여전히 v15 external-content 스키마로 남아있는지 확인.

---

## Rationale (reviewer-only)

### 설계 결정 — external content 포기 이유

원래 `messages_fts USING fts5(content, content=messages, content_rowid=rowid)` 구조는 FTS5 external-content 모드로, `snippet()` / `highlight()` 호출 시 **외부 테이블 (`messages.content`) 에서 원문을 읽어온다**. 이 모드에서 "tokenized 저장 + 원문 snippet" 을 동시에 노리기는 불가능한데, FTS5 가 snippet 복원 시 **token offset 을 external content 의 byte offset 에 매핑** 하기 때문. tokenized 스트림 ("아키텍처 설계") 과 원문 ("아키텍처를 설계한다") 의 offset 이 다르면 snippet 결과는 garbage 가 된다 (SQLite FTS5 docs §4.4 참조).

secall 이 `CREATE VIRTUAL TABLE turns_fts USING fts5(content, session_id UNINDEXED, turn_id UNINDEXED, tokenize='unicode61')` 처럼 standalone FTS5 + app-level `extract_snippet` 을 쓰는 이유가 정확히 이것. 본 plan 은 secall 의 입증된 경로를 따른다.

### 대안 비교

| Option | 구조 | 장점 | 단점 | 결정 |
|---|---|---|---|---|
| A | external content 유지 + `snippet()` 그대로 | 최소 변경 | snippet offset mismatch → 깨진 snippet | 기각 |
| B | external content 유지 + tokenized 컬럼 mapping | storage 1x | snippet 은 여전히 깨짐 | 기각 |
| **C (채택)** | standalone FTS5 + content_tokenized + app-level snippet | secall 검증 패턴, snippet 원문 보존 | storage ~1.5x (tokenized 별도 저장) | ✅ |
| D | Dual-index (whitespace + morph 두 벌) | flag 전환 무비용 | storage 2x, trigger 복잡 | 기각 |
| E | FTS5 custom tokenizer C extension | 순수 DB 레이어 | fts5_api 등록 + Rust FFI 복잡 | 기각 |

### 비용/위험

- **Storage overhead (재계산, 2026-04-22 Developer 지적 반영)**: 초안의 ~1.6x 는 `content_tokenized` 컬럼만 센 오산. standalone FTS5 의 shadow tables 포함 시:
  | 컴포넌트 | 배수 | 설명 |
  |---|---|---|
  | `messages.content` (원본) | 1.0x | 기존 |
  | `messages.content_tokenized` | ~0.7x | 조사/어미 제거 + 1글자 드롭 |
  | `messages_fts_content` | ~0.7x | standalone FTS5 가 자체 보유 (external content 모드에는 없던 shadow table) |
  | `messages_fts_idx` + `_data` + `_docsize` + `_config` | ~0.5x | inverted postings + metadata |
  | **합계** | **~2.9x** | 기존 대비. 대용량 corpus 에서 non-trivial. |
  Subtask 05 Settings UI 에 "재구축 후 예상 추가 용량" 을 `pending_content_bytes × 1.9` (rough estimate) 로 사전 표시해 사용자 동의 절차를 둔다. 초안의 "DB 파일 크기 × 2" 는 Codex review 2차 반영으로 corpus-based 계산으로 교정됨.
- **Tokenize CPU**: Lindera ko-dic 은 건당 마이크로초 단위. 스트리밍 finalize 시 1회만 호출하므로 hot-path 영향 미미. Rebuild 시 500 row/chunk 기준 chunk 당 <1 초 예상 (로컬 benchmark 필요).
- **Write lock 경합**: INV-3 으로 완화. 실측 후 chunk size 조정 여지.
- **점진적 마이그레이션 실수**: INV-4 (COALESCE fallback) 로 silent miss 대신 legacy 동작 유지.
- **UI 혼선**: rebuild 전까지 검색 결과가 empty — Settings UI 에서 명시적으로 "재구축이 필요합니다" 안내 필수 (subtask 05 스펙).

### Scope 밖 (별도 plan 권장)

- **영문 stemmer** (Porter/Snowball) — 한국어 해결 후 논의.
- **rawq 결과 hybrid 합류** — `hybrid.rs` extensibility 는 있으나 별도 plan.
- **FTS index 를 per-project DB 로 sharding** — `perProjectDatabaseSplitPlan.md` 와 합산 설계 필요.
- **Snippet 하이라이트 멀티-term 지원** — secall extract_snippet 은 first term 기준. 후속 UX 작업.

### Codex review 반영 (2026-04-22, 2차)

Codex blind verifier 가 BLOCKER 2건 / MAJOR 1건 / MINOR 1건 식별:

1. **BLOCKER (resolved)** — `AFTER UPDATE OF content` trigger 제거 시 content-only UPDATE 3 사이트 (`update_message_status`, `jobs::cleanup_stale`, `bootstrap/db::stale_cleanup`) 에서 FTS stale. → `messages_fts_update_content` trigger 를 추가해 tokenize 없이 `NEW.content` fallback 으로 resync. INV-5 의 의미를 "Lindera 호출 없음" 으로 축소.
2. **BLOCKER (resolved)** — `extract_snippet` 의 `content.to_lowercase()` 가 Unicode case expansion (Turkish `İ`→`i̇`, German `ß`→`ss`) 시 byte length 가 변해 lowered string 의 byte offset 을 원본에 역매핑하면 boundary panic. → 하이라이트 마커 `**...**` 를 제거하고 **순수 char-window 추출** 로 단순화. Unicode 하이라이트는 후속 plan 으로 분리 (UI 측 클라이언트 하이라이트 또는 별도 safe impl).
3. **MAJOR (resolved)** — INV-7 의 "OFF 시 빈 배열 아님" 주장은 tokenizer 가 조사/어미를 drop 하므로 근거 부족. → INV-7 의미를 "query path 만 원복" 으로 축소, 인덱스 측 rollback 은 별도 커맨드 영역으로 분리.
4. **MINOR (resolved)** — Storage estimate 가 DB 전체 파일 크기 × 2 였으나 corpus 무관. → `SUM(length(content))` 기반으로 교정.

### Open questions

> **Developer 1차 검토 (2026-04-22) 로 Q-1, Q-4, Q-6 해소**. 나머지는 구현 단계에서 결정.

1. ~~**Tokenized 저장 시점 (Q-1)**~~ — **Resolved**. Developer 합의: INSERT-time 계산. Lindera 건당 마이크로초 수준이므로 스트리밍 I/O noise 이내. Spec §3.1 원안 확정.

2. **Rebuild 실패 시 content_tokenized 정합성 (Q-2)**: 중간 실패 row 가 부분적으로 tokenized 된 상태에서 다음 rebuild 시 재처리 필요 여부. 현재 설계는 `IS NULL` 체크만 하므로 "partial tokenized" 는 건너뜀. 재처리 강제는 별도 "force" 플래그로.

3. **Search index 크기 한계 (Q-3)**: 현재 tunaFlow 는 단일 SQLite. 대용량 프로젝트에서 messages 가 100만건 이상이 될 때 rebuild 시간 (~수 분) 이 허용 가능한지. 허용 불가능하면 per-project DB 분리가 선결 조건.

4. ~~**Flag 활성화 UX (Q-4)**~~ — **Resolved** (2026-04-22 2차 Codex review 반영). **Env var 우선 + `SEARCH_MORPH_FLAG: AtomicBool` 런타임 + FE localStorage (persist)**. `morphological_query_enabled()` 는 env 값이 있으면 env, 없으면 AtomicBool 반환. FE 는 startup hook 에서 localStorage 값을 읽어 `invoke('set_morphological_query_enabled', { enabled })` 로 AtomicBool 에 주입. **DB 저장 없음** — 기존 codebase 에 `app_settings` 테이블 부재, 신규 도입 시 scope 확대. Subtask 05 도 이 단일 소스로 통일.

5. **Snippet max_chars 기본값 (Q-5)**: secall=200, 기존 tunaFlow snippet()=40 (의미적 토큰 개수). UI 에서 `line-clamp-2` 와 정합하려면 120 권장 — 최종값은 Frontend 검증.

6. ~~**Migration v45 atomicity (Q-6)**~~ — **Resolved → INV-8 로 승격**. SQLite 는 FTS5 virtual table DROP/CREATE 를 포함해 대부분 DDL 을 transactional 로 지원. `apply_v45` 전체를 `conn.transaction()` 으로 감싸 실패 시 rollback 보장. Subtask 01 의 verification 에 atomicity 테스트 추가.

---

## Subtask 구조

| # | 파일 | 범위 | 의존 |
|---|---|---|---|
| 01 | [-task-01.md](./searchPipelineFromSecallPlan-part2-task-01.md) | Migration v45 + schema.rs + trigger 재작성 | — |
| 02 | [-task-02.md](./searchPipelineFromSecallPlan-part2-task-02.md) | Write path 통합 (10+ sites) + tokenize_for_index helper | 01 |
| 03 | [-task-03.md](./searchPipelineFromSecallPlan-part2-task-03.md) | `rebuild_messages_fts` + 취소 + 진행률 이벤트 | 01, 02 |
| 04 | [-task-04.md](./searchPipelineFromSecallPlan-part2-task-04.md) | `search_messages` morph 분기 + app-level extract_snippet | 01 |
| 05 | [-task-05.md](./searchPipelineFromSecallPlan-part2-task-05.md) | Settings UI (rebuild 버튼 + 진행률) | 03 |

총 5 subtask. 06 (snippet 단독) 를 분리하지 않은 이유: search_messages 전환과 한 PR 에 묶는 편이 자연스럽다 (같은 호출부).

---

## 관련 문서

- 상위 plan: `docs/plans/searchPipelineFromSecallPlan.md` §6 Phase C
- 선행 PR: #127 `feat/search-pipeline-phase-c-tokenizer`
- RT 규약: `docs/plans/harnessVerificationGapPlan.md` §5 proposer 2-track output
- secall 참조: `seCall/crates/secall-core/src/store/schema.rs:44-51`, `seCall/crates/secall-core/src/search/bm25.rs:191`
