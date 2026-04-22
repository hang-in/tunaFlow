# Subtask 01 — `current_session_key` 재정의 + `promote_pending_to_delivered` 수정

> 상위 plan: [sessionContinuityFixPlan.md](./sessionContinuityFixPlan.md)

## Changed files

- `src-tauri/src/agents/claude_sdk_session.rs` — `current_session_key()` (L229-233) 수정.
- `src-tauri/src/commands/agents_helpers/send_common/session_freshness.rs` — `promote_pending_to_delivered()` (L42-48) 수정. 기존 `test promote_uses_pending_value_not_live_lookup` 의 기대값 역전 (live 우선).

## Change description

### 1. `current_session_key` — RESUME_IDS 우선, SESSIONS fallback

```rust
// src-tauri/src/agents/claude_sdk_session.rs
pub fn current_session_key(conv_id: &str) -> Option<String> {
    // (a) Claude 자체 session identity 우선. WS respawn 후에도 유지되므로 안정적.
    if let Some(sid) = RESUME_IDS.lock().get(conv_id).cloned() {
        return Some(format!("claude-ws:{}", sid));
    }
    // (b) Fallback: SESSIONS 의 router UUID. 첫 send (RESUME_IDS 아직 없음) 전용.
    //     이 값은 LAST_DELIVERED 와 매칭되지 않는 것이 정상 (첫 send 는 fresh).
    //     process_alive=false 면 None — is_session_continuation=false 강제.
    let sessions = SESSIONS.lock();
    let s = sessions.get(conv_id)?;
    if !s.process_alive.load(std::sync::atomic::Ordering::Relaxed) {
        return None;
    }
    Some(format!("claude-ws:router:{}", s.session_id))
}
```

### 2. `promote_pending_to_delivered` — live 우선

```rust
// src-tauri/src/commands/agents_helpers/send_common/session_freshness.rs
pub fn promote_pending_to_delivered(msg_id: &str, conv_id: &str, engine: &str) {
    let live = current_session_key(conv_id, engine);  // finalize 시점의 진짜 session identity
    let stashed = PENDING_DELIVERY.lock().remove(msg_id);
    let key = live.or(stashed);   // ← 순서 반전이 핵심 (before: stashed.or(live))
    if let Some(k) = key {
        LAST_DELIVERED_KEY.lock().insert(conv_id.to_string(), k);
    }
}
```

### 3. 기존 테스트 조정

`promote_uses_pending_value_not_live_lookup` (session_freshness.rs:128-141) 의 기대값을 뒤집는다. 새 의미:

```rust
#[test]
fn promote_uses_live_when_available_stashed_is_fallback() {
    let conv = unique_conv("promote-live-priority");
    let msg = "msg-promote-1";

    // Arrange: stash router UUID 를 두고, live 조회도 None (engine=nonexistent) 이 반환되도록
    stash_pending(msg, "claude-ws:router:stashed-at-prepare");

    // Act: live=None → stashed 사용 (fallback 동작)
    promote_pending_to_delivered(msg, &conv, "nonexistent");

    // Assert: fallback 으로 stashed 가 기록됨
    assert_eq!(
        last_delivered_key(&conv).as_deref(),
        Some("claude-ws:router:stashed-at-prepare"),
        "live 가 None 이면 stashed 가 fallback 으로 사용되어야 함"
    );
}
```

또한 신규 테스트 — live 가 있을 때 stashed 가 무시되는지:
```rust
// 이 테스트는 claude_sdk_session 을 mock 할 수 있어야 함 — 또는 engine 별 분기 우회.
// 실구현에서는 current_session_key(conv, engine) 의 engine 이 claude/claude-code 일 때만
// RESUME_IDS 를 본다. engine="claude" 로 호출하되 RESUME_IDS 에 직접 insert 해 live 가
// Some 을 반환하게 만든다. 이는 integration 성격 — 별도 test 모듈 또는 pub(crate) helper.
```

### 4. 신규 회귀 테스트

`src-tauri/src/agents/claude_sdk_session.rs::tests` (또는 신설 `tests/session_identity.rs` integration test):

```rust
#[test]
fn router_uuid_change_does_not_change_identity_when_resume_id_preserved() {
    // RESUME_IDS 에 claude session_id 고정
    RESUME_IDS.lock().insert("conv-A".into(), "claude-sess-AAA".into());
    let k1 = current_session_key("conv-A").unwrap();

    // SESSIONS 에 두 번 다른 router UUID 로 삽입/제거 (respawn 시뮬레이션)
    // — 구체적으로는 SdkSession 구조를 직접 만들기 어려우므로 SESSIONS 를 건드리지 않고
    //   RESUME_IDS 의 우선순위만 검증.
    let k2 = current_session_key("conv-A").unwrap();
    assert_eq!(k1, k2, "RESUME_IDS 가 있는 한 key 는 router UUID 와 무관");
    assert!(k1.starts_with("claude-ws:") && !k1.contains("router:"), "router fallback prefix 가 아니어야");

    RESUME_IDS.lock().remove("conv-A");  // cleanup (전역 lazy_static 격리)
}

#[test]
fn dead_session_without_resume_id_returns_none() {
    // RESUME_IDS 없고 SESSIONS 에 dead entry 가 있는 경우 None 반환 — is_session_continuation=false 강제.
    // SESSIONS 에 직접 entry 주입은 어렵다 (Arc<SdkSession> 필요). 이 테스트는 구현 시점에
    // test hook 을 추가하거나 skip. 대체: integration — 실제 spawn 후 kill.
}
```

## Dependencies

depends_on: 없음.

## Verification

- `cargo test --lib agents::claude_sdk_session` — 기존 + 신규 tests 통과.
- `cargo test --lib commands::agents_helpers::send_common::session_freshness` — 기존 5 tests 중 `promote_uses_pending_value_not_live_lookup` 는 새 이름/의도로 교체. 나머지 4 tests 유지.
- `cargo check` — exit 0.
- **수동 검증**: 로컬 `npm run tauri dev` 기동 후 claude-code 대화 2턴 이상 — `eprintln!("[session_freshness] continuation conv=…")` 로그가 2턴째에 찍히는지. 찍히지 않고 계속 "new session" 로그가 나오면 버그 미해결.

## Risks

- **기존 behavior 신뢰도 의존**: `LAST_DELIVERED_KEY` 에 router UUID 가 기록되던 경우 (현재 동작) 를 가정하는 다른 코드 경로가 있을 수 있다. 본 PR 로 key 포맷이 `claude-ws:<claude_sid>` 또는 `claude-ws:router:<uuid>` 로 변경됨. grep 으로 `claude-ws:` prefix 를 검색해 hardcoded 비교가 없는지 확인 필요 (예상: `session_freshness.rs` 외에는 없음).
- **테스트 전역 격리**: `RESUME_IDS` 는 `lazy_static` 이라 테스트 간 공유. 신규 테스트는 고유 conv_id + 테스트 종료 시 `RESUME_IDS.lock().remove(conv)` 로 cleanup.
- **Race**: `current_session_key` 가 RESUME_IDS lock → SESSIONS lock 순차 획득. 두 lock 모두 `parking_lot::Mutex` 이므로 deadlock 위험 낮음. 그러나 다른 경로가 SESSIONS → RESUME_IDS 순으로 잡는 케이스가 있는지 검토 (grep). 현재 `monitor_task` 는 SESSIONS 만 건드리고, `get_or_create_session` 는 SESSIONS → RESUME_IDS 순. **동일 순서 유지** 위해 `current_session_key` 도 SESSIONS 먼저 잡고 RESUME_IDS 로 확인할지 검토. 하지만 우선순위 역전은 deadlock 요건인 "순환 대기" 가 필요 — 읽기 전용 lock scope 라면 실질 위험 낮음.
- **process_alive 체크의 정당성**: monitor_task 가 SESSIONS.remove 까지 하므로 dead entry 가 남는 window 는 아주 짧다. double-check 는 방어적이지만 중복일 수 있음. Q-4 가 이 판단을 요청.
