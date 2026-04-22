---
title: Session Continuity Fix — ContextPack 재주입 방지 (claude WS session identity 재정의)
status: planned
priority: P0
created_at: 2026-04-22
related:
  - src-tauri/src/agents/claude_sdk_session.rs
  - src-tauri/src/commands/agents_helpers/send_common/session_freshness.rs
  - src-tauri/src/commands/agents_helpers/send_common/persistence.rs
  - docs/plans/harnessVerificationGapPlan.md           # §5 proposer 4-section
  - docs/prompts/architectFollowup_sessionPersistence_2026-04-22.md  # Developer 진단 + 사용자 방침
supersedes:
  - (was-deferred) fix/contextpack-handoff-recall       # 브랜치는 존재하지 않음, 본 plan 이 대체
---

# Session Continuity Fix Plan

> **사용자 방침 (강제 범위)**: preview → full content 주입 / recent_turns cap 확대 / anchor N 턴 확대 — **전부 금지**.
> 근본 수정은 `is_session_continuation` 판정 정확화 + `--resume` 신뢰도 개선. 판정이 맞으면 tunaFlow 는 아무 context 도 주입하지 않는다.

---

## TL;DR for Developer

1. **`claude_sdk_session::current_session_key` 를 "claude 의 실제 session_id" 기반으로 재정의** — 현재는 `s.session_id` (= `spawn_session` 에서 `Uuid::new_v4()` 로 생성되는 router UUID, L272) 를 반환하는데, 이 값은 WS respawn 마다 바뀐다. 올바른 identity 는 `RESUME_IDS[conv_id]` (= claude `--resume` 타깃, L133, parsed `result.session_id` 로 갱신 L715). 이 변경 하나가 증상의 대부분을 해결한다.
2. **`promote_pending_to_delivered` 를 finalize-time current_session_key 우선으로 수정** — 지금은 prepare 시점 stashed 값을 우선 사용 (`session_freshness.rs:44`). 첫 send 에서 pending 에 router UUID 가 들어가면 LAST_DELIVERED 에 router UUID 가 기록되어 다음 send 와 불일치. finalize 시점에 RESUME_IDS 가 이미 채워져 있으므로 그 값으로 **덮어쓰기** 해야 연속성 유지.
3. **`RESUME_IDS` 를 `conversations.resume_token` 에서 bootstrap** — 현재 앱 재시작 시 RESUME_IDS 는 빈 상태로 시작하고, `get_or_create_session` 이 빈 RESUME_IDS 로 spawn 하므로 `--resume` 자체가 동작하지 않는다. DB 의 `resume_token` 값을 최초 세션 요청 시 주입해 앱 재시작 ↔ claude process 연속성을 회복.
4. **Claude 가 새 `session_id` 를 반환하면 (= `--resume` 실패로 fresh claude 세션 시작) LAST_DELIVERED_KEY 를 자동 invalidate** — 현재는 RESUME_IDS 만 새 값으로 덮어써서 다음 send 가 "claude 가 기억 없는 세션" 인데도 tunaFlow 는 continuation=true 로 판단. parsed.session_id 가 RESUME_IDS 의 기존 값과 다르면 `clear_delivered_key` 호출.
5. **측정 지표** — `trace_log.ctx_sections` 의 `context` 섹션이 **연속 송수신 시 0%** 출현해야 한다. 현재 증상은 100%. Fix 후 golden scenario (같은 conv, 5-8 분 간격 5 턴) 에서 turn 2+ 는 전부 미포함.

구현 순서: 1 → 2 → 3 → 4 → 5 (측정). subtask 01 (§1+2 합침), subtask 02 (§3), subtask 03 (§4+5 합침) 로 3 PR 로 분해.

**하지 말 것**: ContextPack `context` 섹션 preview→full 확대, recent_turns cap 상향, anchor N 턴 확대. 이는 재주입 **경로의 규모** 만 키우며 사용자 방침에 위배.

---

## Specification

### 1. `current_session_key` 재정의

파일: `src-tauri/src/agents/claude_sdk_session.rs` L229-233

```rust
// before
pub fn current_session_key(conv_id: &str) -> Option<String> {
    SESSIONS.lock().get(conv_id).map(|s| format!("claude-ws:{}", s.session_id))
}

// after
pub fn current_session_key(conv_id: &str) -> Option<String> {
    // 1) Claude 자체 session identity (--resume 타깃) 우선.
    //    이 값은 respawn 이후에도 유지되므로 LAST_DELIVERED_KEY 와 안정적으로 비교 가능.
    if let Some(sid) = RESUME_IDS.lock().get(conv_id).cloned() {
        return Some(format!("claude-ws:{}", sid));
    }
    // 2) RESUME_IDS 가 아직 없는 케이스 (첫 send, finalize 전) — SESSIONS 의 router UUID 로 fallback.
    //    첫 send 는 어차피 LAST_DELIVERED 가 비어있어 is_session_continuation=false 로 흐르므로
    //    fallback 키가 다음 send 와 매칭되지 않는 문제는 발생하지 않는다.
    let sessions = SESSIONS.lock();
    let s = sessions.get(conv_id)?;
    // process_alive 가 false 이면 세션은 dead → None 반환하여 is_session_continuation=false 강제
    if !s.process_alive.load(std::sync::atomic::Ordering::Relaxed) {
        return None;
    }
    Some(format!("claude-ws:router:{}", s.session_id))  // prefix 로 구분 — LAST_DELIVERED 에는 보통 매치 안 됨
}
```

**핵심**: `claude-ws:<sid>` 포맷의 `sid` 는 **claude 가 결정하는 UUID** 여야 한다. router UUID 는 안전장치 fallback 으로만 사용하고, prefix 를 `claude-ws:router:` 로 분리해 누수 시 쉽게 식별 가능하게 한다.

### 2. `promote_pending_to_delivered` 수정

파일: `src-tauri/src/commands/agents_helpers/send_common/session_freshness.rs` L42-48

```rust
// before
pub fn promote_pending_to_delivered(msg_id: &str, conv_id: &str, engine: &str) {
    let from_pending = PENDING_DELIVERY.lock().remove(msg_id);
    let key = from_pending.or_else(|| current_session_key(conv_id, engine));
    if let Some(k) = key {
        LAST_DELIVERED_KEY.lock().insert(conv_id.to_string(), k);
    }
}

// after
pub fn promote_pending_to_delivered(msg_id: &str, conv_id: &str, engine: &str) {
    // prepare 시점의 stashed 키는 "이 send 가 시작될 때 어떤 세션에 바인딩되었는가" 를
    // 기록하기 위한 것 (A2 race 보호). 그러나 **LAST_DELIVERED 의 의미** 는
    // "이번 send 가 끝난 시점에 claude 가 어떤 세션을 유지 중인가" 이다.
    // finalize 시점에 RESUME_IDS 가 갱신되어 있으므로 current_session_key 를 우선 사용.
    let live = current_session_key(conv_id, engine);
    let stashed = PENDING_DELIVERY.lock().remove(msg_id);
    let key = live.or(stashed);  // live 가 있으면 우선, 없으면 stashed fallback
    if let Some(k) = key {
        LAST_DELIVERED_KEY.lock().insert(conv_id.to_string(), k);
    }
}
```

**검증 포인트**: `live.or(stashed)` 의 순서 반전이 이 패치의 핵심. 기존 `stashed.or(live)` 는 prepare 시점 router UUID 가 승급되는 반면, 변경 후는 finalize 시점 claude session_id 가 승급된다.

### 3. `RESUME_IDS` bootstrap from DB

파일: `src-tauri/src/agents/claude_sdk_session.rs` — `get_or_create_session` 진입부 (L140-177)

```rust
// 추가: RESUME_IDS 가 비어있다면 DB 에서 1회 로드 시도
fn bootstrap_resume_id_from_db(conv_id: &str, state: &crate::AppState) -> Option<String> {
    // 이미 메모리에 있으면 skip
    if RESUME_IDS.lock().contains_key(conv_id) { return None; }

    let r = state.db.read.lock().ok()?;
    let token: Option<String> = r.query_row(
        "SELECT resume_token FROM conversations WHERE id=?1 AND resume_token_engine IN ('claude','claude-code')",
        [conv_id], |row| row.get(0),
    ).ok().flatten();
    if let Some(t) = token.clone() {
        RESUME_IDS.lock().insert(conv_id.to_string(), t);
    }
    token
}
```

`get_or_create_session` 에서 `SESSIONS` 가 비어있는 branch 로 진입하기 전에 이 함수 호출. `AppState` 접근 시그니처 변경 필요 — `get_or_create_session` 호출부 (claude.rs / sdk-url 경로) 에서 AppState 를 전달하거나, 별도 `tauri::State` 파라미터로 받아 Arc clone.

**주의**: DB 쓰기 lock 대신 **read lock** 만 사용 (bootstrap 은 read-only). Session persistence 가 finalize_engine_run 에서 DB 를 갱신하므로 write race 없음.

### 4. Claude 가 새 session_id 반환 시 자동 invalidate

파일: `src-tauri/src/agents/claude_sdk_session.rs` L714-717 부근

```rust
// before
if let Some(sid) = &parsed.session_id {
    RESUME_IDS.lock().insert(conv_id_owned.clone(), sid.clone());
}

// after
if let Some(sid) = &parsed.session_id {
    let prior = RESUME_IDS.lock().insert(conv_id_owned.clone(), sid.clone());
    // prior 가 있고 sid 와 다르면: --resume 이 실패 또는 불인정되어 claude 가 새 세션을 시작한 것.
    // LAST_DELIVERED_KEY 를 invalidate 해 다음 send 가 full 로 흐르게 한다.
    if let Some(p) = prior {
        if p != *sid {
            eprintln!("[sdk-session] claude returned new session_id (prior={} new={}) — invalidating delivery cache", p, sid);
            crate::commands::agents_helpers::send_common::session_freshness::clear_delivered_key(&conv_id_owned);
            // 이번 send 는 이미 prepare 시 Continuation/Fresh 결정이 끝났으므로 다음 send 부터 effect.
        }
    }
}
```

### 5. 측정 지표

`trace_log` 는 각 턴의 `ctx_sections` 를 기록한다 (persistence.rs:343 `insert_trace_log_with_context`). Fix 전후 비교를 위해:

**수동 E2E (acceptance)**:
- 새 conversation 생성, claude-code 엔진
- 5 턴 송수신 (각 간격 1~8 분, 중간에 claude 프로세스 강제 kill 1회 포함)
- `SELECT ctx_sections FROM trace_log WHERE conversation_id=? ORDER BY recorded_at` 결과에서:
  - turn 1: `context` 섹션 포함 (fresh) ✅
  - turn 2~5: `context` 섹션 **미포함** ✅ (continuation)
  - 강제 kill 직후 turn: `context` 섹션 포함 ✅ (정당한 fresh)

**자동 unit/integration**:
- `session_freshness.rs::tests` 에 "router UUID 변경 with same claude_session_id → continuation=true" 케이스 추가.
- `claude_sdk_session.rs::tests` (또는 별도) 에 RESUME_IDS bootstrap 테스트.

---

## Invariants

- **[INV-1]** `ContextPack` 의 `recent_context` / `compressed_memory` 섹션은 `is_session_continuation == true` 일 때 **절대 주입되지 않는다**. Claude 자체 세션 히스토리가 source of truth. **이유**: 사용자 방침 명시. **검증**: `prepare_engine_run` 분기 (persistence.rs:258-266) 가 `data.is_session_continuation = true` 시 assemble_prompt 의 context 섹션을 drop. 기존 구현이 이 경로를 이미 보장함 — INV 는 이 경로가 **실제로 타는 빈도** 를 높여야 함을 의미.

- **[INV-2]** `is_session_continuation` 판정의 key 는 **claude 자체 `session_id`** (= `--resume` 타깃, `parsed.session_id` 로 갱신되는 값) 이다. tunaFlow 측 프로세스 식별자 (spawn 마다 새로 생성되는 router UUID) 는 continuation 판정 key 로 쓰지 않는다. **이유**: router UUID 는 WS respawn 마다 변해 false negative 양산. **검증**: `current_session_key` 유닛 테스트 — 같은 claude session_id 에 대해 router UUID 가 변경되어도 동일 key 반환.

- **[INV-3]** WS 세션 respawn 은 (a) `kill_session*` 명시 호출, (b) `monitor_task` 의 child process exit 감지, (c) `get_or_create_session` 에서 `SESSIONS` 에 없을 때 중 하나의 경로로만 발생한다. send 경로가 "활성 세션이 있는데 임의로 재생성" 하지 않는다. **이유**: 불필요 respawn 은 claude session buffer 를 잃게 만들어 `--resume` 성공률을 낮춘다. **검증**: `spawn_session` 호출지점 grep — `get_or_create_session` 내 단일 호출만 존재.

- **[INV-4]** `RESUME_IDS[conv_id]` 는 **앱 생애주기 전반에서 claude 가 최종적으로 알려준 session_id 와 동기** 상태를 유지한다. (a) 앱 시작 시 `conversations.resume_token` 에서 lazy bootstrap, (b) 매 응답 `parsed.session_id` 로 갱신, (c) `kill_session_clear_resume` 으로만 제거. **이유**: claude `--resume` 성공률 = RESUME_IDS 정확도. 부정확하면 "내가 기억하는 세션" 오해로 토큰 낭비. **검증**: bootstrap 단위 테스트 + parsed.session_id 갱신 unit test.

- **[INV-5]** `LAST_DELIVERED_KEY[conv_id]` 는 **마지막으로 claude 가 실제 응답을 완성한 send 종료 시점의 `current_session_key`** 를 기록한다. prepare 시점 stashed 값은 `current_session_key` 가 None 인 경우에만 fallback. **이유**: prepare 시점 router UUID 가 승급되면 다음 send 와 불일치. 현 버그의 직접 원인. **검증**: `promote_pending_to_delivered` 테스트 — live current key 우선 사용되는지.

- **[INV-6]** Claude 응답의 `parsed.session_id` 가 `RESUME_IDS` 에 이미 있는 기존 값과 **다른 경우** (= `--resume` 실패 후 claude 가 fresh 세션 시작), `clear_delivered_key` 를 호출해 LAST_DELIVERED_KEY 를 즉시 invalidate 한다. **이유**: 이 상황에서 다음 send 가 continuation=true 로 가면 "claude 는 기억 없는데 tunaFlow 는 있다고 착각" — 사용자 맥락 유실의 다른 경로. **검증**: "새 session_id 반환" 시뮬레이션 테스트 (변조된 JSONL fixture).

- **[INV-7]** 앱 재시작 후 첫 send 는 `LAST_DELIVERED_KEY` 가 in-memory 이므로 `is_session_continuation=false` 로 흘러 full ContextPack 을 생성한다. 이는 수용된 behavior — claude 프로세스도 앱과 함께 죽었다가 `--resume` 으로 복원되므로 tunaFlow 가 "첫 send 에 한 번 전체 맥락을 건네는 것" 은 버퍼 warm-up 비용으로 정당하다. **이유**: 이 경우까지 재주입을 skip 하려면 LAST_DELIVERED 를 DB persist 해야 하고, 복잡도 증가 대비 이득 작음. **검증**: 스펙 문서화 + trace_log 첫 턴에 `context` 섹션 기대값 포함.

---

## Rationale (reviewer-only)

### 왜 preview → full injection 이 아닌가

증상 (2-3 턴 전 assistant 응답의 뒷부분 유실) 을 해결하는 가장 쉬운 경로는 ContextPack `context` 섹션에 preview 대신 full content 를 넣거나 recent_turns 의 2000자 cap 을 제거하는 것이다. 그러나:

1. **같은 정보를 두 곳에서 보관** — claude WS session buffer + tunaFlow context 섹션. 매 턴마다 ~3000자 × N 턴이 재전송되어 입력 토큰이 턴당 수만 토큰 폭증.
2. **claude 가 이미 기억하는 내용을 다시 보내면** 혼동 (같은 turn 이 두 번 보이는 것처럼 대화 흐름이 꼬임).
3. **근본 문제는 "기억이 없는 것" 이 아니라 "기억이 있는데 tunaFlow 가 false negative 로 판단하는 것"** — 판정 로직 자체를 고치는 것이 옳은 계층.

사용자 방침 ("체감 개선으로 토큰 낭비 금지") 은 이를 명시적으로 배제.

### 대안 고려

| 대안 | 판정 | 사유 |
|---|---|---|
| preview → full 주입 확대 | 기각 | 사용자 방침 명시 + 토큰 폭증 |
| `recent_turns` cap 확대 (2000 → 8000) | 기각 | 재주입의 규모만 키움. 원인 치료 아님 |
| anchor N turn 확대 (2 → 5) | 기각 | 동일. 재주입 자체가 본질적 낭비 |
| `current_session_key` 정의 수정 (채택) | ✅ | 근본 수정. 판정 정확화 → 재주입 자연 감소 |
| 매 send 마다 `--resume` 성공 여부 확인 응답 대기 | 고려함 → 기각 | latency 추가. `parsed.session_id` 대조로 비동기 감지 가능 (INV-6) |
| `LAST_DELIVERED_KEY` 를 DB persist | 기각 (현 단계) | INV-7 수용. 복잡도 대비 이득 작음 |

### Open questions (Developer 결정 필요)

1. **Q-1**: `bootstrap_resume_id_from_db` 는 `get_or_create_session` 이 `AppState` 접근권을 갖도록 시그니처를 바꿔야 한다. 현재 `claude_sdk_session::get_or_create_session(conv_id, project_path, model)` 는 DB 접근이 없는 순수 함수. Tauri `State<'_, AppState>` 를 파라미터로 추가할지, 또는 `lazy_static` 전역 DB handle 에서 읽을지 — 호출 site (claude.rs 의 run) 가 이미 AppState 를 받고 있으면 전자 권장.

2. **Q-2**: codex app-server (`codex_app_server::current_thread_key`) 에도 동일 문제가 있을 수 있다. 본 plan 은 claude 에 한정. codex 경로는 별도 plan 으로 분리 (Subtask 에 포함하지 않음 — scope creep 방지).

3. **Q-3**: Roundtable 경로는 `LAST_DELIVERED_KEY` 를 공유할지 각 참여자별로 분리할지. 현재 코드는 conv_id 기준이므로 RT 내 여러 참여자가 같은 conv 의 key 를 덮어쓸 수 있음. 검증 시 RT 시나리오도 테스트에 포함할지 판단.

4. **Q-4**: `process_alive=false` 상태의 session 에 대해 `current_session_key` 가 `None` 을 반환 (위 §1). 그러나 monitor_task 는 `SESSIONS.lock().remove` 도 하므로 대개 entry 자체가 사라진다. 이 double-check 가 필요한지 (방어적) vs 중복인지 판단.

---

## Subtask 구조

| # | 파일 | 범위 | 의존 |
|---|---|---|---|
| 01 | [-task-01.md](./sessionContinuityFixPlan-task-01.md) | `current_session_key` 재정의 + `promote_pending_to_delivered` 수정 + unit tests (§1+§2) | — |
| 02 | [-task-02.md](./sessionContinuityFixPlan-task-02.md) | `RESUME_IDS` bootstrap from DB `resume_token` (§3) + 시그니처 조정 | 01 |
| 03 | [-task-03.md](./sessionContinuityFixPlan-task-03.md) | 신규 session_id 반환 시 auto-invalidate (§4) + 측정 지표 E2E (§5) | 01 |

3 subtask 로 구성. 02 는 앱 재시작 후 `--resume` 연속성 회복 (가장 큰 scope 변경) 이지만 01 과 독립 동작 가능.

---

## 관련 문서

- Developer 진단: `docs/prompts/architectFollowup_sessionPersistence_2026-04-22.md` (PR #130)
- 이전 Architect 핸드오프: `docs/prompts/architectHandoff_2026-04-22.md`
- RT 규약: `docs/plans/harnessVerificationGapPlan.md` §5 proposer 2-track output
- Phase 4 `is_session_continuation` 최초 도입 세션: `project_session_2026-04-19_s38.md` (anchor 2turn / recent_turns tool-request 도입 맥락)
