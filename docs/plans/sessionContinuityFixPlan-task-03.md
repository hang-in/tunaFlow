# Subtask 03 — 신규 session_id 감지 시 auto-invalidate + 측정 지표

> 상위 plan: [sessionContinuityFixPlan.md](./sessionContinuityFixPlan.md)

## Changed files

- `src-tauri/src/agents/claude_sdk_session.rs` — L714-717 부근 `parsed.session_id` 갱신 로직에 prior-vs-new 비교 + `clear_delivered_key` 호출 추가.
- `tests/session_continuity_integration.rs` (신규) — 통합 테스트: 5턴 시나리오에서 trace_log.ctx_sections 기대값 검증.
- `docs/how-to/` — 없음 (내부 로직 변경만, 외부 API 불변).

## Change description

### 1. Auto-invalidate on new session_id

```rust
// src-tauri/src/agents/claude_sdk_session.rs L714-717
// before
if let Some(sid) = &parsed.session_id {
    RESUME_IDS.lock().insert(conv_id_owned.clone(), sid.clone());
}

// after
if let Some(sid) = &parsed.session_id {
    let prior = RESUME_IDS.lock().insert(conv_id_owned.clone(), sid.clone());
    if let Some(p) = prior {
        if p != *sid {
            eprintln!(
                "[sdk-session] claude returned new session_id (prior={} new={}) — \
                 --resume likely rejected; invalidating LAST_DELIVERED for conv={}",
                p, sid, conv_id_owned
            );
            crate::commands::agents_helpers::send_common::session_freshness::clear_delivered_key(
                &conv_id_owned,
            );
        }
    }
}
```

### 2. Integration test (신규 파일)

```rust
// src-tauri/tests/session_continuity_integration.rs
//! Plan: docs/plans/sessionContinuityFixPlan.md
//! Verifies is_session_continuation remains true across WS respawn
//! and flips to false when claude returns a new session_id.

#[test]
fn continuation_survives_router_uuid_change() {
    // Setup: 동일 conv_id 에 대해 RESUME_IDS = claude_sid_1, LAST_DELIVERED = claude-ws:claude_sid_1
    //        SESSIONS entry 를 두 번 (서로 다른 router UUID) 로 교체해도 current_session_key 불변.
    // Assert: is_session_continuation == true 유지.
    // (SESSIONS 조작은 test hook 또는 pub(crate) 접근 필요 — 구현 시점 결정)
}

#[test]
fn continuation_breaks_when_claude_starts_new_session() {
    // Setup: RESUME_IDS = sid1, LAST_DELIVERED = claude-ws:sid1
    // Act: parsed.session_id = sid2 (sid1 ≠ sid2) 로 갱신 시나리오 (unit-level mock of event_loop).
    //      해당 코드 경로에서 clear_delivered_key 호출되면 LAST_DELIVERED 비어야.
    // Assert:
    //   - RESUME_IDS = sid2
    //   - LAST_DELIVERED_KEY = None
    //   - 다음 is_session_continuation 호출 = false
}

#[test]
fn trace_log_continuation_drops_context_section() {
    // End-to-end 수준 (가벼운 DB + mock engine):
    //   1. prepare + finalize turn 1 → LAST_DELIVERED 기록
    //   2. prepare turn 2 (동일 RESUME_IDS) → data.is_session_continuation = true
    //   3. assemble_prompt 의 ctx_sections 에서 "context" 섹션 없음
    //   4. trace_log INSERT 직전의 ContextPackMeta 에 section 목록 확인
}
```

### 3. 측정 지표 자동화

Golden scenario 실행 스크립트 또는 기존 `evals/` 인프라 재사용:

```bash
# 예시 (실제 명령은 프로젝트의 evals/ 구조에 맞춤)
cargo run --bin eval -- session-continuity \
    --conv <new-uuid> --turns 5 --kill-at turn=3
```

기대 출력:
```
turn=1 mode=Standard ctx_sections=[identity, context, memory, user_prompt]   [fresh: ✅]
turn=2 mode=Standard ctx_sections=[identity, user_prompt]                    [cont: ✅]
turn=3 mode=Standard ctx_sections=[identity, context, memory, user_prompt]   [post-kill fresh: ✅]
turn=4 mode=Standard ctx_sections=[identity, user_prompt]                    [cont: ✅]
turn=5 mode=Standard ctx_sections=[identity, user_prompt]                    [cont: ✅]

continuation_rate = 4/5 = 80%  (fresh turns 제외)
context_section_unnecessary_reinjection = 0/3 (cont turns) ✅
```

현재 증상 baseline: `context_section_unnecessary_reinjection = 3/3` (100%).

## Dependencies

depends_on: [01]. [02] 없이도 동작하지만, [02] 없으면 앱 재시작 후 시나리오가 재현 안 됨 — 측정의 완전성을 위해 [02] 머지 후 실행 권장.

## Verification

- `cargo test --test session_continuity_integration` — 3 tests 모두 pass.
- `cargo check` — exit 0.
- **수동 acceptance**:
  1. 새 conversation 생성, claude-code 엔진.
  2. 5턴 송수신 (각 3~10분 간격). 중간 (turn 3 직전) 에 `pkill -9 claude` 로 claude 프로세스 강제 종료.
  3. DB 확인:
     ```sql
     SELECT turn_index, ctx_sections FROM trace_log
       WHERE conversation_id = '<conv>' ORDER BY recorded_at;
     ```
  4. 기대:
     - turn 1, 3 (post-kill) 에만 `"context"` 포함
     - turn 2, 4, 5 에는 `"context"` 미포함
  5. Architect 재현 (이 버그 발견 시나리오):
     - tunaReader 유사 conversation 에서 ~5분 간격 5 턴 후 마지막 Architect 가 1턴 전 응답의 뒷부분을 **tool-request 없이** 언급할 수 있어야 함 (Claude 내부 buffer 의존).

## Risks

- **`parsed.session_id` 이 `None` 인 응답**: 일부 event 에선 session_id 가 없을 수 있다. 현 코드는 `Some` 일 때만 insert. invalidate 로직도 `Some` 분기 안에서만 실행 — 안전.
- **RESUME_IDS 와 LAST_DELIVERED_KEY 동시 갱신 ordering**: invalidate 가 RESUME_IDS insert 이후 실행됨. race 는 없으나 다른 thread 가 중간에 `current_session_key` 를 호출하면 새 RESUME_IDS 값 (sid2) 를 보고 LAST_DELIVERED 는 sid1. → `is_session_continuation = false`. 정확한 동작 (sid1 세션은 이미 끝남). ✅
- **수동 테스트의 자동화 부담**: 측정 지표 §3 는 수동이어도 blocker 아님. 자동화는 후속 `promptRegressionEvalPlan.md` 연동. 본 PR 은 trace_log 로그 해석 가이드만 문서화하면 충분.
- **`clear_delivered_key` 는 LAST_DELIVERED 만 삭제**. PENDING_DELIVERY 의 현재 in-flight 키는 건드리지 않음. 해당 send 는 이미 prepare 시 결정된 mode 로 진행. 이는 의도 — 진행 중인 응답을 invalidate 하면 사용자 혼란.
- **Integration test 의 SESSIONS 조작**: SdkSession 구조체 직접 생성이 어렵다 (child Child + 여러 channel). test hook (pub(crate) 생성자 또는 builder) 이 필요. 구현 난이도 있음 — 필요 시 이 test 는 unit 대신 수동 manual test 로 대체하고 issue 로 남김.
