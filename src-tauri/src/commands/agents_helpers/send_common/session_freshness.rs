//! ContextPack 세션 신선도(freshness) 판정.
//!
//! 핵심 규칙: 같은 (engine, session_id) 튜플로 연속 send → 에이전트가 이미
//! `--replay-user-messages`/codex thread 내부에 히스토리를 가지고 있으므로
//! ContextPack은 minimal(user prompt 위주)로 충분.
//!
//! 다른 튜플 → 새 에이전트 프로세스(엔진 전환, 세션 crash, 첫 send) → full ContextPack.
//!
//! 적용 대상:
//! - claude `--sdk-url` (claude_sdk_session::SESSIONS)
//! - codex app-server (codex_app_server::CONV_THREADS)
//!
//! 적용 제외 (항상 full):
//! - claude `-p` CLI 모드 (start_claude_stream의 비-sdk 경로)
//! - codex CLI exec fallback
//! - gemini, opencode (one-shot)
//! - Roundtable participants
//! - Branch shadow conversation의 첫 send (LAST_DELIVERED 비어있어 자동으로 full)

use std::collections::HashMap;
use parking_lot::Mutex as PlMutex;

lazy_static::lazy_static! {
    /// conv_id → 마지막으로 full ContextPack을 전달한 세션 키.
    /// 같은 키가 다시 등장하면 minimal로 보낼 수 있다.
    static ref LAST_DELIVERED_KEY: PlMutex<HashMap<String, String>> = PlMutex::new(HashMap::new());

    /// msg_id → prepare 시점에 capture한 세션 키 (in-flight send용).
    /// finalize 성공 시 LAST_DELIVERED_KEY로 promote 후 제거. 실패/타임아웃 시 그냥 제거.
    /// prepare 시점의 키를 보존해 prepare↔finalize 사이의 SESSIONS 변경(monitor task crash 등)에 영향받지 않게 함.
    static ref PENDING_DELIVERY: PlMutex<HashMap<String, String>> = PlMutex::new(HashMap::new());
}

/// prepare 시점에 호출 — capture된 키를 msg_id 기준으로 in-flight 보관.
pub fn stash_pending(msg_id: &str, key: &str) {
    PENDING_DELIVERY.lock().insert(msg_id.to_string(), key.to_string());
}

/// finalize 성공 시 호출 — 현재 세션 키를 LAST_DELIVERED 로 승격.
///
/// **순서 (sessionContinuityFixPlan.md INV-5)**: finalize 시점 `current_session_key`
/// 가 진짜 session identity (claude 가 응답에서 돌려준 session_id, RESUME_IDS 에
/// 갱신된 값) 이므로 **live 우선**. stashed 값은 live 가 None 인 race 케이스의
/// fallback.
///
/// 이전 순서 (`stashed.or(live)`) 는 prepare 시점에 router UUID 가 stash 되어
/// 첫 send 부터 LAST_DELIVERED 가 router UUID 로 고정 → 다음 send 에서 키 포맷
/// 불일치 → `is_session_continuation=false` 반복. 순서 반전으로 해결.
///
/// 둘 다 None 이면 기록하지 않음 (예: -p one-shot 모드).
pub fn promote_pending_to_delivered(msg_id: &str, conv_id: &str, engine: &str) {
    let live = current_session_key(conv_id, engine);
    let stashed = PENDING_DELIVERY.lock().remove(msg_id);
    let key = live.or(stashed);
    if let Some(k) = key {
        LAST_DELIVERED_KEY.lock().insert(conv_id.to_string(), k);
    }
}

/// finalize 실패 시 호출 — pending 키만 정리. LAST_DELIVERED는 건드리지 않음.
pub fn discard_pending(msg_id: &str) {
    PENDING_DELIVERY.lock().remove(msg_id);
}

/// 현재 conversation에 대해 활성화된 세션 키를 반환.
/// WS/app-server 지속 세션을 사용하지 않는 엔진은 None을 반환하며,
/// 이 경우 호출자는 항상 full ContextPack 경로를 사용해야 한다.
pub fn current_session_key(conv_id: &str, engine: &str) -> Option<String> {
    match engine {
        "claude" | "claude-code" => crate::agents::claude_sdk_session::current_session_key(conv_id),
        "codex" => {
            // codex는 OpenAI SDK > app-server > CLI 순으로 fallback.
            // app-server 모드일 때만 thread key 존재.
            crate::agents::codex_app_server::current_thread_key(conv_id)
        }
        _ => None,
    }
}

/// 마지막으로 기록된 전달 키.
pub fn last_delivered_key(conv_id: &str) -> Option<String> {
    LAST_DELIVERED_KEY.lock().get(conv_id).cloned()
}

/// send 완료 시 현재 세션 키를 기록.
/// 다음 send에서 같은 키면 minimal로 흐른다.
pub fn record_delivered_key(conv_id: &str, key: &str) {
    LAST_DELIVERED_KEY.lock().insert(conv_id.to_string(), key.to_string());
}

/// 같은 세션이 연속되는 상황인지 판정.
/// true → minimal ContextPack 가능, false → full 필수.
pub fn is_session_continuation(conv_id: &str, engine: &str) -> bool {
    let current = current_session_key(conv_id, engine);
    let last = last_delivered_key(conv_id);
    matches!((current.as_deref(), last.as_deref()), (Some(c), Some(l)) if c == l)
}

/// 세션이 명시적으로 종료되거나 conversation이 reset될 때 호출.
/// 다음 send는 full로 강제된다.
/// 호출처: `branches::open_branch_stream` (브랜치 재open 시), `claude_sdk_session::kill_session_*`,
/// 향후 conversation reset/clear 시.
pub fn clear_delivered_key(conv_id: &str) {
    LAST_DELIVERED_KEY.lock().remove(conv_id);
}

#[cfg(test)]
mod tests {
    use super::*;

    // 테스트는 글로벌 lazy_static 레지스트리를 공유하므로 각 테스트는 고유 conv_id를
    // 사용해야 한다. UUID 같은 고유 prefix로 격리.
    fn unique_conv(prefix: &str) -> String {
        format!("{}-{}", prefix, uuid::Uuid::new_v4())
    }

    #[test]
    fn record_then_compare_same_key_returns_true_via_helper() {
        let conv = unique_conv("rec-eq");
        record_delivered_key(&conv, "claude-ws:abc");
        assert_eq!(last_delivered_key(&conv).as_deref(), Some("claude-ws:abc"));
    }

    #[test]
    fn discard_pending_does_not_promote() {
        let conv = unique_conv("disc");
        let msg = "msg-disc-1";
        stash_pending(msg, "claude-ws:xyz");
        discard_pending(msg);
        // promote 호출해도 pending이 비었고 current_session_key도 None(모듈 외 의존성 없음)
        // → LAST_DELIVERED_KEY는 갱신되지 않아야 함.
        promote_pending_to_delivered(msg, &conv, "unknown-engine");
        assert!(last_delivered_key(&conv).is_none(),
            "discard 후 promote는 LAST_DELIVERED를 변경하지 않아야 함");
    }

    #[test]
    fn promote_falls_back_to_stashed_when_live_is_none() {
        // sessionContinuityFixPlan INV-5 적용 후 동작: live 가 None 일 때만 stashed
        // fallback 으로 쓰인다. 이 경로는 (a) -p CLI 모드처럼 WS 세션 없음 (b) A2
        // race 로 SESSIONS/RESUME_IDS 가 동시에 비어있는 짧은 window 를 커버한다.
        let conv = unique_conv("promote-fallback");
        let msg = "msg-promote-fallback";
        stash_pending(msg, "claude-ws:captured-at-prepare");
        // engine="nonexistent" → current_session_key는 None
        promote_pending_to_delivered(msg, &conv, "nonexistent");
        assert_eq!(
            last_delivered_key(&conv).as_deref(),
            Some("claude-ws:captured-at-prepare"),
            "live 가 None 이면 stashed 를 fallback 으로 사용해야 함"
        );
    }

    #[test]
    fn clear_delivered_forces_full_next_time() {
        let conv = unique_conv("clear");
        record_delivered_key(&conv, "claude-ws:abc");
        clear_delivered_key(&conv);
        // is_session_continuation은 last가 비어있으면 false → 다음 send는 full
        // (current_session_key가 Some이라도 last가 None이면 매치 안 됨)
        assert!(last_delivered_key(&conv).is_none());
    }

    #[test]
    fn is_session_continuation_requires_both_present_and_equal() {
        let conv = unique_conv("cont");
        // last가 없으면 false
        assert!(!is_session_continuation(&conv, "claude"));
        // last 기록 후에도 current_session_key가 None이면 false (-p engine 같은 경우)
        record_delivered_key(&conv, "claude-ws:xyz");
        assert!(!is_session_continuation(&conv, "unknown-engine"),
            "current_session_key가 None인 엔진은 continuation 판정 false여야 함");
    }
}
