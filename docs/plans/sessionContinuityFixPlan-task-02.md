# Subtask 02 — `RESUME_IDS` bootstrap from DB `resume_token`

> 상위 plan: [sessionContinuityFixPlan.md](./sessionContinuityFixPlan.md)

## Changed files

- `src-tauri/src/agents/claude_sdk_session.rs` — `bootstrap_resume_id_from_db()` 신설. `get_or_create_session()` 진입부에서 호출.
- `src-tauri/src/agents/claude.rs` (또는 sdk-url 진입점) — `get_or_create_session` 호출 시 AppState / DB 참조 전달.
- `src-tauri/src/lib.rs` — 필요 시 시그니처 변경에 따른 builder 조정.

## Change description

### 1. bootstrap helper

```rust
// src-tauri/src/agents/claude_sdk_session.rs
/// DB 의 conversations.resume_token 을 RESUME_IDS 로 끌어올린다.
/// 앱 재시작 후 첫 접근 시 1회 실행. 이미 메모리에 있으면 no-op.
///
/// 호출부 시점에 DB 읽기 가능해야 한다 (Tauri State<AppState> 주입).
fn bootstrap_resume_id_from_db<DB>(conv_id: &str, db: &DB) -> Option<String>
where DB: crate::db::ReadConnProvider  // trait 또는 concrete AppState.db
{
    if RESUME_IDS.lock().contains_key(conv_id) {
        return RESUME_IDS.lock().get(conv_id).cloned();
    }
    let conn = db.read_conn().ok()?;
    let token: Option<String> = conn.query_row(
        "SELECT resume_token FROM conversations
          WHERE id = ?1
            AND resume_token IS NOT NULL
            AND resume_token_engine IN ('claude','claude-code')",
        [conv_id],
        |row| row.get(0),
    ).ok();
    if let Some(ref t) = token {
        RESUME_IDS.lock().insert(conv_id.to_string(), t.clone());
        eprintln!("[sdk-session] bootstrapped RESUME_IDS conv={} from DB resume_token", conv_id);
    }
    token
}
```

### 2. `get_or_create_session` 시그니처 변경

```rust
// before
pub async fn get_or_create_session(
    conv_id: &str, project_path: Option<&str>, model: Option<&str>,
) -> Result<Arc<SdkSession>, AppError>

// after — AppState (또는 DbState) 를 받아 bootstrap 수행
pub async fn get_or_create_session(
    conv_id: &str,
    project_path: Option<&str>,
    model: Option<&str>,
    db: &crate::state::DbHandle,   // 최소한 read lock 만 필요
) -> Result<Arc<SdkSession>, AppError> {
    // ... 기존 SESSIONS 조회 로직 ...

    if !SESSIONS.lock().contains_key(conv_id) {
        // 신규 spawn 전 bootstrap 시도
        bootstrap_resume_id_from_db(conv_id, db);
    }

    let resume_id = RESUME_IDS.lock().get(conv_id).cloned();
    let session = spawn_session(conv_id, project_path, effective_model, resume_id.as_deref()).await?;
    SESSIONS.lock().insert(conv_id.to_string(), Arc::clone(&session));
    Ok(session)
}
```

### 3. 호출부 업데이트

`get_or_create_session` 호출 지점을 grep 으로 확인:
```
rg "get_or_create_session\(" src-tauri/src/
```
각 site 에서 `AppState` / `DbState` 를 전달. 현재 호출 site 는 (예상):
- `src-tauri/src/agents/claude.rs` — sdk-url 경로의 run 함수
- `src-tauri/src/agents/claude_sdk_session.rs::prewarm_session`
- 테스트 코드

`prewarm_session` 도 bootstrap 해야 "프로젝트 오픈 직후 첫 send" 에서 --resume 이 작동한다.

## Dependencies

depends_on: [01]

## Verification

- Unit test (mock DB):
  ```rust
  #[test]
  fn bootstrap_loads_resume_token_when_memory_empty() {
      // Arrange: 메모리 비어있음, DB 에 resume_token='sess-XYZ', engine='claude'
      RESUME_IDS.lock().remove("conv-boot");
      let db = test_db_with_conversation("conv-boot", Some("sess-XYZ"), Some("claude"));

      // Act
      let got = bootstrap_resume_id_from_db("conv-boot", &db);

      // Assert
      assert_eq!(got.as_deref(), Some("sess-XYZ"));
      assert_eq!(RESUME_IDS.lock().get("conv-boot").cloned().as_deref(), Some("sess-XYZ"));
  }

  #[test]
  fn bootstrap_is_noop_when_memory_has_value() {
      RESUME_IDS.lock().insert("conv-boot-2".into(), "sess-MEM".into());
      let db = test_db_with_conversation("conv-boot-2", Some("sess-DB"), Some("claude"));
      let got = bootstrap_resume_id_from_db("conv-boot-2", &db);
      // 메모리 값 유지, DB 값 미사용
      assert_eq!(got.as_deref(), Some("sess-MEM"));
  }

  #[test]
  fn bootstrap_skips_non_claude_engine() {
      let db = test_db_with_conversation("conv-boot-3", Some("sess-WRONG"), Some("codex"));
      RESUME_IDS.lock().remove("conv-boot-3");
      let got = bootstrap_resume_id_from_db("conv-boot-3", &db);
      assert!(got.is_none());
  }
  ```
- Integration E2E (manual):
  1. claude-code 대화 2턴 송수신
  2. `SELECT resume_token FROM conversations WHERE id = '<conv>'` 으로 토큰 확인
  3. 앱 완전 종료 후 재기동
  4. 같은 conversation 에서 세 번째 메시지 송신
  5. 확인:
     - `[sdk-session] bootstrapped RESUME_IDS conv=...` 로그
     - claude spawn 인자에 `--resume <sess-id>` 포함
     - claude 응답이 2턴 전 맥락을 기억
     - (INV-7 수용) 이 첫 send 자체는 `context` 섹션 포함 (fresh LAST_DELIVERED)
     - (핵심) 네 번째 메시지는 continuation → `context` 섹션 미포함
- `cargo check` — exit 0.

## Risks

- **시그니처 변경 범위**: `get_or_create_session` 호출 site 가 예상보다 많으면 리팩토링 범위 확대. 미리 grep 으로 count. 3 이하면 단순, 10+ 면 adapter 함수 도입 고려.
- **DB access 순환**: `claude_sdk_session` 은 `agents/` 레이어, DB 접근은 `AppState` 레이어. 레이어 crossing 이 관례와 충돌하는지 — 기존 `persistence.rs:338` 이 이미 conversations.resume_token 을 쓰므로 crossing 자체는 허용. 다만 `agents/` 가 직접 DB 접근하면 circular 위험 — trait (예: `ReadConnProvider`) 추상화 권장.
- **Lock order**: bootstrap 은 DB read lock → RESUME_IDS lock 순. persistence.rs finalize 는 DB write → (다른 코드경로 RESUME_IDS 갱신). 순환 여부 grep 으로 확인. 현재까지는 위험 없음.
- **DB resume_token 이 stale**: claude CLI 가 수동으로 `--session-id` 바꿔 실행한 이력이 있으면 DB 값과 실제 claude session 불일치. 본 fix 는 claude 가 응답 시 parsed.session_id 로 덮어쓰므로 첫 응답 이후 자연스럽게 정합. 그 전까지 `--resume` 시도가 claude 측 "no such session" 에러로 실패 가능 → Subtask 03 의 INV-6 경로가 이 edge 를 커버.
- **AppState clone 부담**: DB handle 만 전달하면 되므로 Arc 기반 trait object 로 충분. 전체 AppState 전달 금지 (과도한 결합).
