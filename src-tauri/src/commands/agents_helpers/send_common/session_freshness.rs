//! ContextPack 세션 신선도(freshness) 판정.
//!
//! 핵심 규칙: 같은 (engine, session_id) 튜플로 연속 send → 에이전트가 이미
//! `--replay-user-messages`/`--resume`/codex thread 내부에 히스토리를 가지고 있으므로
//! ContextPack은 minimal(user prompt 위주)로 충분.
//!
//! 다른 튜플 → 새 에이전트 프로세스(엔진 전환, 세션 crash, 첫 send) → full ContextPack.
//!
//! 적용 대상:
//! - claude `--sdk-url` (claude_sdk_session::SESSIONS) — key: `claude-ws:{sid}`
//! - claude `-p` cli `--resume` (claudeTransportFlipHardeningPlan T9, 2026-04-30)
//!   — key: `claude-cli:{sid}` (sdk-url path 와 prefix 분리해 충돌 차단)
//! - codex app-server (codex_app_server::CONV_THREADS)
//!
//! 적용 제외 (항상 full):
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
///
/// claude 분기 (T9, 2026-04-30 — claudeTransportFlipHardeningPlan):
/// - sdk-url mode (`TUNAFLOW_USE_SDK_URL=1`) 또는 anthropic_sdk + branch path
///   → `claude_sdk_session::current_session_key` 위임 (`claude-ws:{sid}` 형식)
/// - cli mode (default since 2026-04-29) → `claude-cli:{sid}` 형식. cli 와 sdk-url
///   양쪽이 같은 conv 에 LAST_DELIVERED 등록할 일은 없지만 (mode 는 conv lifetime 동안
///   고정), prefix 분리로 mode 전환 시 stale key 충돌도 차단.
///
/// **router fallback 처리**: cli/sdk-url 모두 RESUME_IDS 가 비어있는 첫 send 에서는
/// `claude_sdk_session::current_session_key` 가 router UUID fallback 을 반환할 수 있음
/// (`claude-ws:router:{uuid}`). cli mode 에서는 router fallback 의미가 없으므로
/// (cli 는 SESSIONS 자체를 채우지 않음) `None` 으로 정규화 — 첫 send 의 LAST_DELIVERED
/// 미등록을 보장.
pub fn current_session_key(conv_id: &str, engine: &str) -> Option<String> {
    current_session_key_with_mode(conv_id, engine, is_claude_cli_mode(conv_id))
}

/// `current_session_key` 의 inner — mode flag 를 외부 인자로 받는 testable form.
///
/// production code 는 `current_session_key` 만 호출 (flag 자동 detect).
/// test 는 본 함수에 명시 flag 를 주입 → process-wide env var mutation 회피로
/// CI 병렬 실행 race 차단.
fn current_session_key_with_mode(
    conv_id: &str,
    engine: &str,
    cli_mode_active: bool,
) -> Option<String> {
    match engine {
        "claude" | "claude-code" => {
            if cli_mode_active {
                // cli mode: RESUME_IDS 의 sid 를 cli prefix 로 wrapping.
                // sdk_session 의 current_session_key 를 호출하되 결과를 변환.
                let raw = crate::agents::claude_sdk_session::current_session_key(conv_id)?;
                // router fallback (`claude-ws:router:...`) 은 cli 에 무의미 → None.
                // SESSIONS 가 cli path 에서 채워지지 않으므로 router fallback 자체가
                // 도달하기 어렵지만, 방어적으로 차단.
                if raw.starts_with("claude-ws:router:") {
                    return None;
                }
                let sid = raw.strip_prefix("claude-ws:")?;
                Some(format!("claude-cli:{}", sid))
            } else {
                // sdk-url mode (또는 branch + anthropic_sdk path): 기존 동작 그대로.
                crate::agents::claude_sdk_session::current_session_key(conv_id)
            }
        }
        "codex" => {
            // codex는 OpenAI SDK > app-server > CLI 순으로 fallback.
            // app-server 모드일 때만 thread key 존재.
            crate::agents::codex_app_server::current_thread_key(conv_id)
        }
        _ => None,
    }
}

/// claude `-p` cli mode 활성 여부.
///
/// `agents.rs::resolve_claude_mode` 와 동일 로직 (env var `TUNAFLOW_USE_SDK_URL=1`
/// + branch + anthropic_sdk::is_available). 본 함수는 session_freshness 내부에서만
/// 사용해 cli/sdk-url path 의 key prefix 를 분리한다.
///
/// resolve_claude_mode 자체를 pub 으로 노출하면 외부 의존이 늘어나므로, 같은 정책을
/// 좁은 범위로 inline 한다. 정책 변경 시 양쪽 sync 필수 (현재 단일 env var 라 부담 적음).
fn is_claude_cli_mode(conv_id: &str) -> bool {
    let is_branch = conv_id.starts_with("branch:");
    if crate::agents::anthropic_sdk::is_available() && is_branch {
        return false; // sdk path
    }
    if std::env::var("TUNAFLOW_USE_SDK_URL").as_deref() == Ok("1") {
        return false; // sdk-url path
    }
    true // cli (default since 2026-04-29)
}

/// `is_claude_cli_mode` 의 pub wrapper — `persistence::finalize_engine_run` 이 cli mode
/// 에서만 RESUME_IDS 갱신을 trigger 하기 위해 사용. session_freshness 모듈 안에서
/// 정책을 단일 진실로 유지하기 위해 외부 호출은 본 함수로 통일.
pub fn is_claude_cli_mode_external(conv_id: &str) -> bool {
    is_claude_cli_mode(conv_id)
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

    /// T9 (claudeTransportFlipHardeningPlan, 2026-04-30):
    /// cli mode 와 sdk-url mode 의 session key prefix 가 분리되어야 한다.
    /// 같은 RESUME_IDS sid 라도 cli/sdk 양쪽이 같은 conv 에 충돌하지 않도록 검증.
    ///
    /// **CI race 차단**: env var (`TUNAFLOW_USE_SDK_URL`) 를 process-wide 로 mutate
    /// 하면 병렬 테스트 사이 leak 발생 (실제로 첫 PR push CI rust-check 에서 fail).
    /// 따라서 mode flag 는 `current_session_key_with_mode` 에 직접 주입한다.
    #[test]
    fn cli_and_sdk_url_session_key_prefixes_never_collide() {
        let conv_cli = unique_conv("non-branch-cli");
        // RESUME_IDS 에 sid 직접 주입 — claude_sdk_session 의 pub helper 사용.
        crate::agents::claude_sdk_session::register_cli_resume_id(&conv_cli, "sid-abc-123");

        // mode=true (cli) 명시 주입.
        let cli_key = current_session_key_with_mode(&conv_cli, "claude-code", true)
            .expect("cli mode 에서 RESUME_IDS sid 가 있으면 Some");
        assert!(
            cli_key.starts_with("claude-cli:"),
            "cli mode 의 key 는 claude-cli: prefix 를 사용해야 함: {}",
            cli_key
        );
        assert!(
            !cli_key.starts_with("claude-ws:"),
            "cli mode 의 key 는 sdk-url 의 claude-ws: prefix 와 충돌하면 안 됨: {}",
            cli_key
        );

        // 정리 — RESUME_IDS leak 방지 (다른 테스트 영향 차단)
        crate::agents::claude_sdk_session::clear_resume_id_for_test(&conv_cli);
    }

    /// T9: cli mode 의 첫 send (RESUME_IDS 비어있음) 는 None 반환 — LAST_DELIVERED
    /// 미등록 → 다음 send 도 full mode. SESSIONS 가 cli path 에서 채워지지 않으므로
    /// router fallback (`claude-ws:router:`) 이 도달해도 None 으로 정규화.
    #[test]
    fn cli_mode_first_send_returns_none_for_router_fallback() {
        let conv = unique_conv("cli-first-send");
        // RESUME_IDS 미설정, SESSIONS 도 없음 → claude_sdk_session::current_session_key None
        // → cli wrapper 도 None. mode=true (cli) 명시 주입.
        assert!(
            current_session_key_with_mode(&conv, "claude-code", true).is_none(),
            "cli mode 첫 send 는 None 이어야 함 (LAST_DELIVERED 미등록)"
        );
    }

    /// T9: sdk-url mode 일 때 기존 동작 보존 — claude-ws: prefix 그대로.
    #[test]
    fn sdk_url_mode_preserves_existing_claude_ws_prefix() {
        let conv = unique_conv("sdk-url-preserve");
        crate::agents::claude_sdk_session::register_cli_resume_id(&conv, "sid-sdk-url-XYZ");
        // mode=false (sdk-url) 명시 주입.
        let key = current_session_key_with_mode(&conv, "claude-code", false)
            .expect("RESUME_IDS sid 가 있으면 Some");
        assert!(
            key.starts_with("claude-ws:"),
            "sdk-url mode 는 기존 claude-ws: prefix 를 보존해야 함: {}",
            key
        );
        assert!(
            !key.starts_with("claude-cli:"),
            "sdk-url mode 가 cli prefix 를 쓰면 안 됨: {}",
            key
        );
        // 정리
        crate::agents::claude_sdk_session::clear_resume_id_for_test(&conv);
    }
}
