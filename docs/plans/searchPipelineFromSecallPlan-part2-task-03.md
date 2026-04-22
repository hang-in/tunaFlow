# Subtask 03 — `rebuild_messages_fts` Tauri command + 진행률/취소

> 상위 plan: [searchPipelineFromSecallPlan-part2.md](./searchPipelineFromSecallPlan-part2.md)

## Changed files

- `src-tauri/src/commands/search/rebuild.rs` — 신규. rebuild loop + cancel flag + Tauri commands.
- `src-tauri/src/commands/search/mod.rs` — `pub mod rebuild;` + re-export.
- `src-tauri/src/lib.rs` (또는 tauri command registry) — `rebuild_messages_fts`, `cancel_rebuild_messages_fts` 등록.
- `src/types/events.ts` (FE) — 이벤트 타입 정의 (선택, subtask 05 에서 써도 됨).

## Change description

### 1. 상태 구조체

```rust
// src-tauri/src/commands/search/rebuild.rs
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Default)]
pub struct RebuildCancelFlag(pub Arc<AtomicBool>);

impl RebuildCancelFlag {
    pub fn reset(&self) { self.0.store(false, Ordering::SeqCst); }
    pub fn set(&self) { self.0.store(true, Ordering::SeqCst); }
    pub fn is_cancelled(&self) -> bool { self.0.load(Ordering::Relaxed) }
}
```

Tauri state 로 `app.manage(RebuildCancelFlag::default())` 등록 (Tauri builder 쪽). 상위 plan §4 의 state 시그니처 확인.

### 2. 이벤트

```rust
// payloads
#[derive(serde::Serialize, Clone)]
struct ProgressPayload { done: u64, total: u64 }

#[derive(serde::Serialize, Clone)]
struct CompletePayload { done: u64, total: u64, canceled: bool }

#[derive(serde::Serialize, Clone)]
struct ErrorPayload { error: String }
```

이벤트 이름:
- `messages_fts_rebuild_progress`
- `messages_fts_rebuild_complete`
- `messages_fts_rebuild_error`

### 3. Command 시그니처

```rust
#[derive(serde::Serialize)]
pub struct RebuildSummary { pub done: u64, pub total: u64, pub canceled: bool }

#[tauri::command]
pub async fn rebuild_messages_fts(
    app: tauri::AppHandle,
    state: tauri::State<'_, DbState>,
    cancel: tauri::State<'_, RebuildCancelFlag>,
) -> Result<RebuildSummary, AppError> { /* spec 아래 */ }

#[tauri::command]
pub fn cancel_rebuild_messages_fts(
    cancel: tauri::State<'_, RebuildCancelFlag>,
) -> Result<(), AppError> {
    cancel.set();
    Ok(())
}
```

### 4. Rebuild loop (상세)

```rust
pub async fn rebuild_messages_fts(...) -> Result<RebuildSummary, AppError> {
    cancel.reset();
    let cancel_clone = cancel.0.clone();
    let state_db = state.inner().clone();   // DbState 가 Clone 이거나 Arc. 확인 필요.
    let app_clone = app.clone();
    let res = tokio::task::spawn_blocking(move || -> Result<RebuildSummary, AppError> {
        let total: u64 = {
            let r = state_db.read.lock().map_err(|_| AppError::Lock)?;
            r.query_row(
                "SELECT COUNT(*) FROM messages WHERE content_tokenized IS NULL",
                [], |row| row.get::<_, i64>(0)
            )? as u64
        };

        let mut done: u64 = 0;
        const CHUNK: usize = 500;

        loop {
            if cancel_clone.load(Ordering::Relaxed) { break; }

            // (1) read chunk
            let rows: Vec<(i64, String)> = {
                let w = state_db.write.lock().map_err(|_| AppError::Lock)?;
                let mut stmt = w.prepare(
                    "SELECT rowid, content FROM messages
                      WHERE content_tokenized IS NULL
                      LIMIT ?1"
                )?;
                let iter = stmt.query_map(params![CHUNK as i64], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                })?;
                iter.filter_map(Result::ok).collect()
            };
            if rows.is_empty() { break; }

            // (2) tokenize (lock 밖에서 CPU 작업)
            let tokenized: Vec<(i64, String)> = rows.into_iter()
                .map(|(rowid, c)| (rowid, tokenize_for_index(&c)))
                .collect();

            // (3) write chunk — 단일 transaction
            {
                let mut w = state_db.write.lock().map_err(|_| AppError::Lock)?;
                let tx = w.transaction()?;
                {
                    let mut stmt = tx.prepare(
                        "UPDATE messages SET content_tokenized = ?1 WHERE rowid = ?2"
                    )?;
                    for (rowid, t) in &tokenized {
                        stmt.execute(params![t, rowid])?;
                    }
                }
                tx.commit()?;
            }

            done += tokenized.len() as u64;
            app_clone.emit("messages_fts_rebuild_progress", ProgressPayload { done, total })
                .map_err(|e| AppError::Agent(format!("emit: {e}")))?;
        }

        // (4) optimize — 'rebuild' 명령은 external content 모드에서 external 테이블을 다시
        //     읽어 FTS 를 재구성하지만, standalone FTS5 에는 external source 가 없으므로
        //     의미가 없다. 우리는 trigger 가 이미 각 UPDATE 마다 FTS 를 sync 했다고 가정하고
        //     index merge 를 요청하는 'optimize' 만 호출한다. 'rebuild' 를 잘못 쓰면 FTS 가
        //     자신의 content 테이블을 재읽는 self-reference 가 되며 의도와 맞지 않다.
        {
            let w = state_db.write.lock().map_err(|_| AppError::Lock)?;
            w.execute("INSERT INTO messages_fts(messages_fts) VALUES('optimize')", [])?;
        }

        let canceled = cancel_clone.load(Ordering::Relaxed);
        let summary = RebuildSummary { done, total, canceled };
        app_clone.emit("messages_fts_rebuild_complete",
            CompletePayload { done, total, canceled }
        ).map_err(|e| AppError::Agent(format!("emit: {e}")))?;
        Ok(summary)
    }).await.map_err(|e| AppError::Agent(format!("join: {e}")))??;
    Ok(res)
}
```

**중요 포인트**:
- `tokenize_for_index` 호출은 **write lock 밖에서** 수행 (CPU 작업).
- write lock 은 (1) 읽기, (3) 쓰기, (4) optimize 각각 별도 scope — long hold 방지 (INV-3).
- chunk size 500 은 예시. 실측 후 조정. 상수로 `const REBUILD_CHUNK: usize = 500;` 두고 컬렉션.
- `total` 은 loop 시작 시 fix. 새 메시지가 rebuild 중 들어오면 progress 가 100% 를 초과할 수 있음 — UI 가 `min(done,total)` 로 clamp.

### 5. 에러 경로

spawn_blocking 내 `Result<_, AppError>` 반환 시 바깥에서 catch. 에러 이벤트 emit 은 호출자 측 (Frontend) 이 `invoke(...).catch(...)` 로 받을 수 있으므로 선택적.

### 6. `cancel_rebuild_messages_fts` 재호출 안전성

sync command. 동일 state 에 `set()` 만 수행 — 여러 번 호출해도 no-op. rebuild 시작 전 `reset()` 으로 초기화.

## Dependencies

depends_on: [01, 02]

## Verification

- **Unit**: `src-tauri/src/commands/search/rebuild.rs::tests`
  ```rust
  #[tokio::test]
  async fn rebuild_processes_only_null_tokenized() {
      // seed 3 messages: 2 with NULL tokenized, 1 with existing value
      // run rebuild
      // assert: done==2, existing value unchanged
  }

  #[tokio::test]
  async fn rebuild_respects_cancel() {
      // seed 1500 messages (3 chunks). Cancel after first progress event.
      // assert: done < 1500, canceled==true
  }

  #[tokio::test]
  async fn rebuild_is_idempotent() {
      // Run twice. Second run: total==0, done==0.
  }
  ```
- **Integration**: FTS 측 검증 — rebuild 후 `SELECT COUNT(*) FROM messages_fts` 가 `SELECT COUNT(*) FROM messages` 와 같거나 이상 (trigger 가 자동 sync 된 row 포함) 인지.
- **Lock 검증**: integration 테스트에서 rebuild 진행 중 다른 thread 가 `INSERT INTO messages` 를 수행할 수 있는지 (deadlock 아닌지) 확인. Rust native thread spawn → write lock reacquire → INSERT 성공.
- `cargo check` + `cargo test --lib commands::search::rebuild`

## Risks

- **chunk size 부적정**: 500 이 너무 크면 UI freeze, 너무 작으면 overhead. Developer 는 처음에 500 으로 릴리스하고 측정 후 튜닝.
- **`INSERT INTO messages_fts VALUES('optimize')`** 는 standalone FTS5 에서 정상 동작 (`'rebuild'` 는 external content 에서만 의미). 잘못 쓰지 않도록 주의.
- **state.write.lock() 에서 poison**: `.map_err(|_| AppError::Lock)` 로 처리. 다른 worker 가 panic 해 lock 을 poison 한 경우 error 이벤트 emit 후 종료.
- **emit 실패**: Tauri app 이 shutdown 중 emit 실패 가능. `.map_err(|e| AppError::Agent(format!("emit: {e}")))` 로 전파.
- **`content_tokenized IS NULL` 의 row 가 없어도** optimize 단계는 수행 — 이는 기대 동작 (no-op on empty FTS).
- **Tokenize 스레드 이슈**: `global_tokenizer()` 는 `OnceLock<Box<dyn Tokenizer>>`. `Send + Sync` 만족. `spawn_blocking` 에서 안전.
