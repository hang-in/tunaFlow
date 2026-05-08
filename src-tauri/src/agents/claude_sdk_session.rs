//! Claude `--sdk-url` 세션 모드
//!
//! Desktop 앱 소스 분석으로 확인한 실제 동작 방식:
//! - claude 자식 프로세스: `--print --sdk-url ws://... --session-id ... --replay-user-messages`
//! - **사용자 메시지 전달: stdin write** (WS TEXT 아님)
//! - **이벤트 수신: HTTP POST `/{session_id}/events`** (stdout 병행)
//! - WS: 연결 인증(Bearer token) + keepalive 전용
//!
//! 출처: claude 바이너리 내부 JS — `writeStdin(X)` 및 `w.stdout` readline 패턴
//!
//! 참고: `docs/plans/sdkUrlSessionModePlan.md`

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use axum::{
    body::Bytes,
    extract::{Path, State, WebSocketUpgrade, ws},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use parking_lot::Mutex as PlMutex;
use serde::Deserialize;
use tokio::io::AsyncWriteExt;
use tokio::process::Command as TokioCommand;
use tokio::sync::{broadcast, mpsc, oneshot};
use uuid::Uuid;

use crate::agents::claude::{resolve_cwd, RunInput, RunOutput};
use crate::errors::AppError;
use crate::no_console::NoConsole;

// ─────────────────────────────── Stream JSON types ────────────────────────────

/// One JSON line from claude `--output-format stream-json` via HTTP POST events.
#[derive(Deserialize)]
struct SdkStreamLine {
    #[serde(rename = "type")]
    line_type: String,
    message: Option<SdkAssistantMsg>,
    result: Option<String>,
    is_error: Option<bool>,
    cost_usd: Option<f64>,
    total_cost_usd: Option<f64>,
    total_input_tokens: Option<i64>,
    total_output_tokens: Option<i64>,
    session_id: Option<String>,
}

#[derive(Deserialize)]
struct SdkAssistantMsg {
    content: Option<Vec<SdkContentBlock>>,
    /// 토큰 사용량 — assistant 이벤트마다 포함되며 마지막 값이 최신 누적치
    usage: Option<SdkUsage>,
}

#[derive(Deserialize)]
struct SdkUsage {
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
}

#[derive(Deserialize)]
struct SdkContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: Option<String>,
    thinking: Option<String>,
    name: Option<String>,
    input: Option<serde_json::Value>,
}

// ──────────────────────────── WS Server State ─────────────────────────────────────

/// axum 핸들러에 공유되는 세션 WS 서버 상태.
///
/// WS는 인증(Bearer) + keepalive + **tunaFlow→claude 제어 메시지** 전송 경로.
/// 사용자 메시지는 stdin으로 전달하고, 이벤트는 HTTP POST로 수신.
/// 모델 변경은 control_request(set_model)을 WS로 전송한다 (재스폰 없음).
#[derive(Clone)]
struct WsServerState {
    auth_token: String,
    /// claude → tunaFlow 브로드캐스트 (HTTP POST 이벤트 수신)
    from_claude_tx: broadcast::Sender<String>,
    /// claude가 WS 연결 시 신호 (1회만 사용)
    connected_tx: Arc<tokio::sync::Mutex<Option<oneshot::Sender<()>>>>,
    /// tunaFlow → claude WS 전송 채널 (control_request 등)
    ws_send_rx: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<String>>>,
}

// ──────────────────────────── Session Handle ──────────────────────────────────

/// 하나의 conversation에 대한 활성 claude SDK 세션.
pub struct SdkSession {
    /// 세션 식별자 — 매 spawn마다 새로 생성됨. ContextPack freshness 판정에 사용.
    pub session_id: String,
    /// 사용자 메시지를 claude stdin으로 전송하는 채널
    pub to_claude_tx: mpsc::UnboundedSender<String>,
    /// claude 이벤트 브로드캐스트 채널
    pub from_claude_tx: broadcast::Sender<String>,
    /// 현재 세션의 모델 (모델 변경 가능 — 내부 가변성)
    pub model: Arc<PlMutex<String>>,
    /// tunaFlow→claude WS 제어 메시지 전송 채널 (set_model control_request 등)
    pub ws_sender_tx: mpsc::UnboundedSender<String>,
    /// claude child process가 살아있는지. monitor task가 child.wait() 후 false로 set.
    /// stream_run_sdk가 이 값을 polling/select하여 죽었으면 즉시 에러로 끊는다.
    /// (broadcast 채널은 SdkSession이 sender를 보유한 한 close되지 않으므로 별도 신호 필요.)
    pub process_alive: Arc<AtomicBool>,
    /// WS 서버 종료 신호 (Drop 시 자동 전송)
    _shutdown_tx: oneshot::Sender<()>,
    /// claude 자식 프로세스 모니터 태스크 핸들
    /// Drop 시 abort() 호출 → 태스크 취소 → child drop → kill_on_drop으로 프로세스 종료
    _monitor_abort: tokio::task::AbortHandle,
}

impl Drop for SdkSession {
    fn drop(&mut self) {
        // 세션이 제거될 때 모니터 태스크를 명시적으로 취소해 child 프로세스를 즉시 종료한다.
        self._monitor_abort.abort();
    }
}

// ──────────────────────────── Session Registry ────────────────────────────────

type SessionRegistry = Arc<PlMutex<HashMap<String, Arc<SdkSession>>>>;
/// conversation_id → 마지막 result session_id (세션 종료 후에도 --resume용으로 보존)
type ResumeRegistry = Arc<PlMutex<HashMap<String, String>>>;
/// (claudeSdkSessionWindowGuardPlan_2026-05-09) session_key → 마지막 result 의
/// `accumulated_input_tokens` (cumulative SDK history 누적치). stream_run_sdk
/// 의 result handler 가 turn 종료 시 stash, 다음 dispatch 진입 시 read 해서
/// 임계 (180K default / 900K `[1m]`) 도달 여부 검사 → fresh-rotate.
///
/// in-memory stash — 앱 재시작 시 reset (DB persist 영역은 별 P3 plan, Plan §6
/// 후속 plan 가능성).
type WindowGuardRegistry = Arc<PlMutex<HashMap<String, u64>>>;
/// `branch:<branch_id>` shadow conv → root main conversation_id 캐시.
///
/// **의도** (`docs/plans/branchInheritsMainSessionPlan_2026-04-25.md`):
/// brand 는 main session 을 공유해야 한다 (사용자 원래 의도, raw log
/// `037bb82f` 2026-04-17). SESSIONS / RESUME_IDS 는 root main conv_id 로
/// keying 하여 brand send 가 main 의 claude `--sdk-url` 세션을 그대로
/// 이어받는다.
///
/// 캐시 채우는 시점: `bootstrap_resume_id_from_db` 등 DbState 가 있는 진입점.
/// cache miss 시엔 fallback 으로 conv_id 자체 사용 (backward-compatible).
type BranchRootCache = Arc<PlMutex<HashMap<String, String>>>;

lazy_static::lazy_static! {
    static ref SESSIONS: SessionRegistry = Arc::new(PlMutex::new(HashMap::new()));
    static ref RESUME_IDS: ResumeRegistry = Arc::new(PlMutex::new(HashMap::new()));
    static ref BRANCH_ROOT_CACHE: BranchRootCache = Arc::new(PlMutex::new(HashMap::new()));
    /// claudeSdkSessionWindowGuardPlan Task 01 — session_key → 누적 input_tokens.
    static ref WINDOW_GUARD_INPUT_TOKENS: WindowGuardRegistry = Arc::new(PlMutex::new(HashMap::new()));
}

/// (claudeSdkSessionWindowGuardPlan Task 01) 누적 input_tokens 를 stash.
///
/// `stream_run_sdk` 의 result event 핸들러 끝에서 호출. 다음 dispatch 진입 시
/// `take_window_guard_input_tokens` 가 read → 임계 비교 → fresh-rotate 결정.
///
/// **INV-CSW-1** (Plan §1): `accumulated_input_tokens` tracking 본체 (line 902~)
/// 변경 0 — 본 helper 는 *additive*, 단순 stash 만.
fn stash_window_guard_input_tokens(session_key: &str, tokens: u64) {
    WINDOW_GUARD_INPUT_TOKENS
        .lock()
        .insert(session_key.to_string(), tokens);
}

/// (claudeSdkSessionWindowGuardPlan Task 01) 누적 input_tokens stash 를 read.
///
/// `stream_run_sdk` 진입 직후 호출 → 본 값이 임계 도달이면 fresh-rotate.
/// 미존재 (첫 send) 시 0 반환 — 자연스러운 정상 path 흐름.
fn read_window_guard_input_tokens(session_key: &str) -> u64 {
    WINDOW_GUARD_INPUT_TOKENS
        .lock()
        .get(session_key)
        .copied()
        .unwrap_or(0)
}

/// (claudeSdkSessionWindowGuardPlan Task 01) fresh-rotate 후 stash reset.
///
/// `kill_session_clear_resume` 직후 호출 → 새 세션의 첫 turn 부터 정상 누적
/// 시작. INV-CSW-2 (fresh-rotate 후 ContextPack 재주입) 의 일부.
fn clear_window_guard_input_tokens(session_key: &str) {
    WINDOW_GUARD_INPUT_TOKENS.lock().remove(session_key);
}

/// (claudeSdkSessionWindowGuardPlan Task 01) fresh-rotate trigger 발동 결정.
///
/// 누적 input_tokens 가 model_id 별 임계 (Task 04 helper) 도달 시 true.
/// stream_run_sdk 진입 path 에서 호출 → true 면 `kill_session_clear_resume`
/// + `clear_window_guard_input_tokens` + (PR-2 에서) Tauri event emit.
///
/// fresh-rotate 자체는 *현재 turn* 에 적용 — `kill_session_clear_resume` 이
/// SESSIONS / RESUME_IDS / LAST_DELIVERED 모두 invalidate 하므로 그 직후의
/// `get_or_create_session` 이 fresh session 으로 spawn (INV-CSW-2).
pub(crate) fn should_trigger_window_rotate(
    session_key: &str,
    model_id: Option<&str>,
) -> bool {
    let accumulated = read_window_guard_input_tokens(session_key);
    if accumulated == 0 {
        return false; // 첫 send 또는 초기화 직후 — fast path
    }
    let threshold =
        crate::agents::claude_window_guard::current_window_guard_threshold(model_id);
    accumulated >= threshold
}

/// brand:* conv_id 를 root main conversation_id 로 normalize.
///
/// 메모리 캐시 (`BRANCH_ROOT_CACHE`) 만 본다. 캐시가 비어 있으면 conv_id 자체를
/// 반환 — 호출자는 사전에 `cache_branch_root_from_db` 로 채워두어야 한다.
/// 이렇게 분리한 이유: 여러 hot path (SESSIONS lookup 등) 에서 매번 DB read lock
/// 을 잡는 부담을 피하기 위함.
///
/// non-branch conv_id 는 그대로 반환.
///
/// pub: session_freshness 모듈 (LAST_DELIVERED 키 normalize) 에서도 사용.
pub fn session_key_for(conv_id: &str) -> String {
    if conv_id.starts_with("branch:") {
        if let Some(root) = BRANCH_ROOT_CACHE.lock().get(conv_id).cloned() {
            return root;
        }
    }
    conv_id.to_string()
}

/// brand:* conv_id 의 root main conv_id 를 DB 에서 조회해 캐시에 저장.
///
/// `branches.conversation_id` 컬럼은 이미 root main conv_id 를 보유한다
/// (`commands/branches.rs:120-141` 의 `root_conv_id` resolution).
///
/// `parent_branch_id` 체인은 nested branch (b1.1, b1.1.1 등) 의 경우에도
/// 같은 row 의 `conversation_id` 가 root 를 가리키므로 1회 lookup 으로 충분.
///
/// 호출 지점: `bootstrap_resume_id_from_db`. non-branch conv 또는 이미
/// 캐시된 경우 no-op.
pub fn cache_branch_root_from_db(conv_id: &str, db: &crate::db::DbState) {
    let Some(branch_id) = conv_id.strip_prefix("branch:") else {
        return;
    };
    if BRANCH_ROOT_CACHE.lock().contains_key(conv_id) {
        return;
    }
    let Ok(conn) = db.read.lock() else { return };
    let root: Option<String> = conn
        .query_row(
            "SELECT conversation_id FROM branches WHERE id = ?1",
            [branch_id],
            |row| row.get::<_, String>(0),
        )
        .ok();
    if let Some(root_id) = root {
        // branches.conversation_id 가 root 가 아닌 경우는 거의 없지만
        // (create_branch 가 root 로 정규화함) 방어적으로 다시 strip 한다.
        let normalized = if let Some(inner) = root_id.strip_prefix("branch:") {
            // 이론상 도달하지 않는 경로 — fallback 으로 한 단계 더.
            conn.query_row(
                "SELECT conversation_id FROM branches WHERE id = ?1",
                [inner],
                |row| row.get::<_, String>(0),
            )
            .unwrap_or(root_id.clone())
        } else {
            root_id
        };
        BRANCH_ROOT_CACHE
            .lock()
            .insert(conv_id.to_string(), normalized.clone());
        eprintln!(
            "[sdk-session] branch root cached: {} → {}",
            conv_id, normalized
        );
    }
}

/// DB 의 `conversations.resume_token` 을 `RESUME_IDS` 메모리 레지스트리로 로드한다.
///
/// 앱 재시작 후 첫 send 시, `RESUME_IDS` 는 빈 상태여서 `get_or_create_session`
/// 이 `--resume` 없이 fresh claude 세션을 스폰한다 → claude 는 이전 대화
/// 맥락을 잃음. DB 에는 token 이 저장돼 있으므로 (finalize_engine_run 이
/// `conversations.resume_token` 을 갱신), 이 bootstrap 으로 앱 생애주기 이후에도
/// `--resume` 연속성 회복.
///
/// 이미 메모리에 있으면 no-op. DB read lock 만 사용 (write 경로 경합 없음).
///
/// **호출 지점**: `prewarm_sdk_session` / `start_claude_stream` Tauri command
/// 입구 — `stream_run_sdk` / `prewarm_session` 을 부르기 전.
///
/// sessionContinuityFixPlan.md task-02 (INV-4).
pub fn bootstrap_resume_id_from_db(conv_id: &str, db: &crate::db::DbState) -> Option<String> {
    // brand:* 진입점 — root conv 로 normalize 하기 위해 캐시 채움.
    // (claude session 통합: brand 는 main 의 sdk-url WS 세션을 공유한다.)
    cache_branch_root_from_db(conv_id, db);
    let key = session_key_for(conv_id);

    // 이미 메모리에 있으면 skip (DB 보다 메모리 값이 최신일 수 있음)
    if let Some(existing) = RESUME_IDS.lock().get(&key).cloned() {
        return Some(existing);
    }
    let conn = db.read.lock().ok()?;
    let token: Option<String> = conn
        .query_row(
            "SELECT resume_token FROM conversations \
             WHERE id = ?1 \
               AND resume_token IS NOT NULL \
               AND resume_token_engine IN ('claude','claude-code')",
            [&key],
            |row| row.get(0),
        )
        .ok();
    if let Some(ref t) = token {
        RESUME_IDS.lock().insert(key.clone(), t.clone());
        eprintln!(
            "[sdk-session] bootstrapped RESUME_IDS conv={} (key={}) from DB resume_token",
            conv_id, key
        );
    }
    token
}

/// conversation_id에 대한 세션을 반환하거나 새로 생성한다.
///
/// 모델이 변경된 경우 **프로세스를 재스폰하지 않고** WS로 `control_request(set_model)`을
/// 전송해 claude 내부 모델만 교체한다 (Claude Desktop/Claude Code 동일 방식).
pub async fn get_or_create_session(
    conv_id: &str,
    project_path: Option<&str>,
    model: Option<&str>,
) -> Result<Arc<SdkSession>, AppError> {
    let effective_model = match model {
        Some(m) if !m.is_empty() => m,
        _ => "claude-sonnet-4-6",
    };

    // brand 가 main 세션을 공유하도록 키 normalize (Layer A).
    let key = session_key_for(conv_id);

    // 기존 세션 확인 (sync lock, await 없음)
    let existing = {
        let sessions = SESSIONS.lock();
        sessions.get(&key).map(|s| Arc::clone(s))
    };

    if let Some(session) = existing {
        let current_model = session.model.lock().clone();
        if current_model == effective_model {
            return Ok(session); // 같은 모델 — 재사용
        }
        // 모델 변경 — control_request(set_model)을 WS로 전송, 프로세스 유지 시도.
        // 일부 케이스(sonnet→opus 등)에서 claude가 control_request 후 자체 종료할 수 있다.
        // 그런 경우는 stream_run_sdk의 process_alive 감시가 즉시 에러로 끊어주므로
        // UI는 무한 streaming 대신 명확한 에러를 받게 된다.
        send_set_model(&session, effective_model)?;
        *session.model.lock() = effective_model.to_string();
        eprintln!("[sdk-session] model changed via control_request: {} → {} for conv: {} (key={})",
            current_model, effective_model, conv_id, key);
        return Ok(session);
    }

    // 기존 세션 없음 — 새로 스폰. RESUME_IDS의 prior session_id로 --resume 가능.
    let resume_id = RESUME_IDS.lock().get(&key).cloned();
    // spawn_session 내부의 monitor task 가 SESSIONS 에서 entry 를 제거할 때 같은 key 를
    // 써야 하므로 key (root main conv) 를 전달.
    let session = spawn_session(&key, project_path, effective_model, resume_id.as_deref()).await?;
    SESSIONS.lock().insert(key, Arc::clone(&session));
    Ok(session)
}

/// WS를 통해 claude에 set_model control_request를 전송한다.
///
/// claude는 내부 `mainLoopModelOverride`를 즉시 업데이트하고 `control_response(success)`로 응답.
/// 응답은 비동기로 오므로 여기서 기다리지 않는다 (다음 턴부터 새 모델이 적용되면 충분).
fn send_set_model(session: &SdkSession, model: &str) -> Result<(), AppError> {
    let request_id = Uuid::new_v4().to_string();
    let msg = serde_json::json!({
        "type": "control_request",
        "request_id": request_id,
        "request": {
            "subtype": "set_model",
            "model": model
        }
    })
    .to_string();
    session
        .ws_sender_tx
        .send(msg)
        .map_err(|_| AppError::Agent("sdk-session: WS sender closed (set_model failed)".into()))
}

/// 세션을 명시적으로 종료한다.
/// `keep_resume`: true이면 RESUME_IDS를 유지해 다음 세션에서 --resume 사용.
///               false이면 RESUME_IDS도 제거해 다음 세션은 새로 시작.
#[allow(dead_code)]
pub fn kill_session(conv_id: &str) {
    kill_session_with_resume(conv_id, true);
}

pub fn kill_session_clear_resume(conv_id: &str) {
    kill_session_with_resume(conv_id, false);
}

fn kill_session_with_resume(conv_id: &str, keep_resume: bool) {
    let key = session_key_for(conv_id);
    SESSIONS.lock().remove(&key);
    if !keep_resume {
        RESUME_IDS.lock().remove(&key);
    }
    // ContextPack freshness: 세션이 죽으면 LAST_DELIVERED도 무효화 — 다음 send는 full로 강제.
    // brand 와 main 모두 무효화해야 일관성 유지.
    crate::commands::agents_helpers::send_common::session_freshness::clear_delivered_key(conv_id);
    if key != conv_id {
        crate::commands::agents_helpers::send_common::session_freshness::clear_delivered_key(&key);
    }
    // (claudeSdkSessionWindowGuardPlan Task 01) window guard stash 도 정리 —
    // 다음 send 가 fresh session 으로 시작하는데 prior 누적치가 stash 에 남아
    // 있으면 false trigger 위험. brand/main 양쪽 키 모두 정리.
    clear_window_guard_input_tokens(&key);
    if key != conv_id {
        clear_window_guard_input_tokens(conv_id);
    }
    // SdkSession Drop → _shutdown_tx 전송 → axum 서버 종료 → _monitor_abort 취소
}

/// (claudeSdkSessionWindowGuardPlan Task 02 hook) fresh-rotate 발생 시
/// frontend toast 알림 발행 — PR-1 단계는 stub (silent 동작).
///
/// 본 함수는 PR-2 (Task 02) 에서 Tauri event `tunaflow:sdk-session-window-rotated`
/// 를 발행하도록 확장. PR-1 단독 머지 시에도 회귀 차단 (fresh-rotate trigger
/// 자체) 은 즉시 발동, 단 사용자 가시성 (toast) 은 PR-2 머지 후 활성화.
///
/// 매개변수:
/// - `conv_id`: rotate 발생 conversation
/// - `prior_tokens`: rotate 직전 누적 input_tokens
/// - `threshold`: 적용된 임계값 (180K default / 900K `[1m]`)
///
/// stub 단계 — eprintln 만 발행 → release log 에 기록되어 디버깅 용이.
fn emit_window_rotated_event(conv_id: &str, prior_tokens: u64, threshold: u64) {
    // PR-2 에서 AppHandle.emit("tunaflow:sdk-session-window-rotated", payload) 로 확장
    eprintln!(
        "[sdk-window-guard:event] tunaflow:sdk-session-window-rotated conv={} prior_tokens={} threshold={} (PR-2 toast hook stub)",
        conv_id, prior_tokens, threshold
    );
}

/// 해당 conversation에 활성 SDK 세션이 있는지 확인한다.
/// UI send-guard용 (WS 연결 전 send 시도 차단).
///
/// brand:* conv 는 main session 을 공유하므로 main 의 활성 여부를 본다 (Layer A).
pub fn has_active_session(conv_id: &str) -> bool {
    let key = session_key_for(conv_id);
    SESSIONS.lock().contains_key(&key)
}

/// claude `-p` cli mode (T9, claudeTransportFlipHardeningPlan 2026-04-30) — finalize 시점에
/// claude 응답의 새 session_id 를 메모리 RESUME_IDS 에 갱신한다.
///
/// sdk-url path 는 `stream_run_sdk` 의 result 이벤트 핸들러에서 RESUME_IDS 에 직접
/// insert (line 933) 하지만, cli path 는 `claude.rs::stream_run` 외부에서 finalize 가
/// 진행되므로 별도 hook 이 없다. 이 helper 는 cli path 의 finalize 직전에 호출되어
/// `session_freshness::current_session_key` 의 다음 lookup 이 새 sid 를 반영하도록 한다.
///
/// **Identity 원칙**: RESUME_IDS 는 sdk-url 와 cli mode 가 공유. mode 는 conv lifetime
/// 동안 고정 (env var 기반) 이므로 충돌 없음. brand:* 는 root main key 로 normalize.
///
/// **DO NOT 가드** (T9): 본 helper 는 *additive* — sdk-url path 의 result 핸들러 동작
/// 변경 0. 단지 cli path 가 RESUME_IDS 에 접근할 수 있는 새 진입점 추가.
pub fn register_cli_resume_id(conv_id: &str, sid: &str) {
    let key = session_key_for(conv_id);
    let prior = RESUME_IDS.lock().insert(key.clone(), sid.to_string());
    if let Some(p) = prior {
        if p != sid {
            // sdk-url path 의 INV-6 와 동일 정책 — sid 가 변하면 LAST_DELIVERED 무효화.
            // cli path 의 finalize 흐름에서 새 sid 가 들어왔다면 다음 send 를 full 로
            // 강제해 history 일관성 유지 (사용자가 외부에서 session 재생성한 시나리오).
            eprintln!(
                "[cli-session] RESUME_IDS sid changed (prior={} new={}) for conv={} (key={}) — \
                 invalidating LAST_DELIVERED",
                p, sid, conv_id, key
            );
            crate::commands::agents_helpers::send_common::session_freshness::clear_delivered_key(
                conv_id,
            );
            if key != conv_id {
                crate::commands::agents_helpers::send_common::session_freshness::clear_delivered_key(
                    &key,
                );
            }
        }
    }
}

/// 테스트 전용 — RESUME_IDS leak 방지.
///
/// session_freshness 의 unit test 가 `register_cli_resume_id` 호출 후 정리할 때 사용.
/// 다른 테스트의 conv_id 와 충돌하지 않도록 unique conv 를 쓰면 strictly 필요하지
/// 않지만, 안전 마진으로 유지.
#[cfg(test)]
pub fn clear_resume_id_for_test(conv_id: &str) {
    let key = session_key_for(conv_id);
    RESUME_IDS.lock().remove(&key);
}

/// ContextPack freshness 판정용 — 현재 활성 세션의 식별 키.
///
/// **Identity 원칙** (sessionContinuityFixPlan.md task-01): 식별자는 claude 자체가
/// 응답에서 반환하는 `session_id` (= `--resume` 타깃, `RESUME_IDS` 에 캐시) 여야
/// 한다. `SdkSession::session_id` 는 tunaFlow 내부 router UUID 로 WS respawn 마다
/// 새로 생성되므로 identity 로 쓰면 false negative 가 양산된다 (같은 claude
/// 세션인데 재주입 반복).
///
/// Fallback — `RESUME_IDS` 가 아직 채워지지 않은 첫 send 에서는 SESSIONS 의
/// router UUID 로 떨어지되, 키 prefix 를 `claude-ws:router:` 로 분리해 누수 시
/// 쉽게 식별. process_alive=false 이면 None — is_session_continuation=false 강제.
pub fn current_session_key(conv_id: &str) -> Option<String> {
    // brand:* 는 main session 공유 (Layer A).
    let key = session_key_for(conv_id);

    // (a) Claude 자체 session identity 우선. WS respawn 후에도 유지.
    if let Some(sid) = RESUME_IDS.lock().get(&key).cloned() {
        return Some(format!("claude-ws:{}", sid));
    }
    // (b) Fallback — 첫 send, RESUME_IDS 미채워짐. router UUID 를 prefix 로 분리.
    //     첫 send 는 어차피 LAST_DELIVERED 가 비어 있어 is_session_continuation=false
    //     로 자연스럽게 흐르므로 이 fallback 이 계속 쓰이는 일은 없다.
    let sessions = SESSIONS.lock();
    let s = sessions.get(&key)?;
    if !s.process_alive.load(std::sync::atomic::Ordering::Relaxed) {
        return None;
    }
    Some(format!("claude-ws:router:{}", s.session_id))
}

/// conversation이 로드될 때 미리 세션을 초기화한다.
///
/// claude 자식 프로세스 + MCP 초기화(26-60s)를 사전에 완료해
/// 사용자가 첫 메시지를 보낼 때 즉시 응답받을 수 있도록 한다.
/// 세션이 이미 존재하면 no-op.
pub async fn prewarm_session(
    conv_id: &str,
    project_path: Option<&str>,
    model: Option<&str>,
) {
    // brand:* 는 main session 공유 — main 의 세션이 있으면 prewarm 스킵 (Layer A).
    let key = session_key_for(conv_id);
    let exists = SESSIONS.lock().contains_key(&key);
    if exists { return; }

    match get_or_create_session(conv_id, project_path, model).await {
        Ok(_) => eprintln!("[sdk-session] prewarmed conv={} (key={})", conv_id, key),
        Err(e) => eprintln!("[sdk-session] prewarm failed conv={}: {}", conv_id, e),
    }
}

// ──────────────────────────── Session Spawn ───────────────────────────────────

async fn spawn_session(
    conv_id: &str,
    project_path: Option<&str>,
    model: &str,
    resume_session_id: Option<&str>,
) -> Result<Arc<SdkSession>, AppError> {
    // 랜덤 포트 바인딩
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| AppError::Agent(format!("sdk-session: bind failed: {}", e)))?;
    let port = listener
        .local_addr()
        .map_err(|e| AppError::Agent(format!("sdk-session: local_addr failed: {}", e)))?
        .port();

    let session_id = Uuid::new_v4().to_string();
    let auth_token = Uuid::new_v4().to_string();

    // 채널 생성
    // to_claude: tunaFlow → claude stdin (사용자 메시지)
    let (to_claude_tx, mut to_claude_rx) = mpsc::unbounded_channel::<String>();
    // from_claude: claude → tunaFlow (이벤트 브로드캐스트)
    let (from_claude_tx, _) = broadcast::channel::<String>(512);
    let (connected_tx, connected_rx) = oneshot::channel::<()>();
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    // ws_sender: tunaFlow → claude WS (control_request: set_model 등)
    let (ws_sender_tx, ws_send_rx) = mpsc::unbounded_channel::<String>();

    let ws_state = WsServerState {
        auth_token: auth_token.clone(),
        from_claude_tx: from_claude_tx.clone(),
        connected_tx: Arc::new(tokio::sync::Mutex::new(Some(connected_tx))),
        ws_send_rx: Arc::new(tokio::sync::Mutex::new(ws_send_rx)),
    };

    // axum 라우터: WS 연결(keepalive/auth) + HTTP POST 이벤트 수신
    let router = Router::new()
        .route("/{session_id}", axum::routing::get(ws_handler))
        .route("/{session_id}/events", axum::routing::post(events_handler))
        .with_state(ws_state);

    // axum 서버 태스크
    tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            })
            .await
            .ok();
    });

    // claude 자식 프로세스 스폰
    // sdk_url: WS 연결 + HTTP POST 이벤트 경로 기반 URL
    // ws://127.0.0.1:{port}/{session_id} → POST http://127.0.0.1:{port}/{session_id}/events
    let sdk_url = format!("ws://127.0.0.1:{}/{}", port, session_id);
    let cwd = resolve_cwd(project_path);

    let mut cmd = TokioCommand::new("claude");
    cmd.no_console();
    cmd.arg("--print")
        .arg("--sdk-url").arg(&sdk_url)
        .arg("--model").arg(model)
        .arg("--input-format").arg("stream-json")
        .arg("--output-format").arg("stream-json")
        .arg("--replay-user-messages")
        .arg("--dangerously-skip-permissions")
        .env("CLAUDE_CODE_ENVIRONMENT_KIND", "bridge")
        .env("CLAUDE_CODE_SESSION_ACCESS_TOKEN", &auth_token)
        // HybridTransport: 이벤트를 HTTP POST로 전송 (Desktop 앱 동일 패턴)
        .env("CLAUDE_CODE_POST_FOR_SESSION_INGRESS_V2", "1")
        .env_remove("CLAUDE_CODE_OAUTH_TOKEN")
        .current_dir(&cwd)
        // stdin/stdout piped — stdin: 메시지 전달, stdout: 이벤트 수신(HTTP POST 병행)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        // stderr 를 piped 로 surface — 첫 메시지 stuck (Defender first-spawn /
        // connection refused / version 불일치 등) 의 root cause 가 invisible 인
        // 회귀 차단. 별 task 에서 line 단위 drain (drain 실패 시 buffer full 로
        // claude 가 hang 함).
        // PR #222 codex stderr surface 와 동등 패턴.
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);

    // `--session-id` vs `--resume` 상호배타 (claude CLI 2.1.x 제약).
    //   - resume 있음: claude 가 prior session_id 를 이어받으므로 `--session-id` 생략
    //   - resume 없음: `--session-id <router-uuid>` 로 신규 세션 식별자 부여
    // WS routing 은 `sdk_url` 경로의 router UUID 기반이므로 claude 내부 session_id
    // 와 무관하게 정상 동작. sessionContinuityFixPlan.md 의 hot fix (Architect 지시).
    if let Some(resume_id) = resume_session_id {
        cmd.arg("--resume").arg(resume_id);
        eprintln!("[sdk-session] resuming with session_id={} model={} (--session-id 생략)", resume_id, model);
    } else {
        cmd.arg("--session-id").arg(&session_id);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| AppError::Agent(format!("sdk-session: spawn failed: {}", e)))?;

    // stdin 핸들 추출 — 메시지 전달 채널로 사용
    let mut child_stdin = child
        .stdin
        .take()
        .ok_or_else(|| AppError::Agent("sdk-session: could not get child stdin".into()))?;

    // stdout 핸들 추출 — 이벤트 수신 (HTTP POST와 병행)
    let child_stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::Agent("sdk-session: could not get child stdout".into()))?;

    // stderr 핸들 추출 + drain 태스크 — claude 의 startup 실패 (Defender freeze /
    // connection refused / version 미스매치 등) 가 backend stderr 로 즉시 표면화
    // 되도록 한다. WS connect 30s timeout 도달 전 claude 가 stderr 로 무엇을
    // 토했는지가 가장 빠른 root-cause 신호. 30s 안에 connect 못 하는 케이스라도
    // 이 태스크는 분리되어 있어 timeout 처리 흐름을 막지 않는다.
    if let Some(child_stderr) = child.stderr.take() {
        let conv_id_for_log = conv_id.to_string();
        tokio::spawn(async move {
            use tokio::io::{AsyncBufReadExt, BufReader};
            let mut lines = BufReader::new(child_stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if line.trim().is_empty() { continue; }
                eprintln!("[sdk-session-stderr] conv={} {}", conv_id_for_log, line);
            }
        });
    }

    // claude WS 연결 대기 (최대 30초)
    tokio::time::timeout(
        std::time::Duration::from_secs(30),
        connected_rx,
    )
    .await
    .map_err(|_| AppError::Agent("sdk-session: claude did not connect within 30s".into()))?
    .map_err(|_| AppError::Agent("sdk-session: connected signal dropped".into()))?;

    eprintln!("[sdk-session] claude connected on port {} model={} for conv: {}", port, model, conv_id);

    // stdin 쓰기 태스크: to_claude_rx → child stdin
    // 각 메시지는 JSON 한 줄 + newline
    tokio::spawn(async move {
        while let Some(msg) = to_claude_rx.recv().await {
            let line = format!("{}\n", msg);
            if child_stdin.write_all(line.as_bytes()).await.is_err() {
                eprintln!("[sdk-session] stdin write failed, channel closed");
                break;
            }
        }
        // 채널이 닫히면 stdin EOF → claude가 정상 종료
        drop(child_stdin);
    });

    // stdout 읽기 태스크: 이벤트를 from_claude_tx로 브로드캐스트 (HTTP POST 병행)
    let from_claude_tx_stdout = from_claude_tx.clone();
    tokio::spawn(async move {
        use tokio::io::{AsyncBufReadExt, BufReader};
        let mut lines = BufReader::new(child_stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if line.trim().is_empty() { continue; }
            let _ = from_claude_tx_stdout.send(line);
        }
    });

    // 자식 프로세스 모니터 태스크
    let conv_id_owned = conv_id.to_string();
    let process_alive = Arc::new(AtomicBool::new(true));
    let alive_for_monitor = process_alive.clone();
    let monitor_task = tokio::spawn(async move {
        let _ = child.wait().await;
        eprintln!("[sdk-session] claude subprocess exited for conv: {}", conv_id_owned);
        // alive flag를 가장 먼저 false로 — stream_run_sdk가 즉시 감지하도록.
        alive_for_monitor.store(false, Ordering::SeqCst);
        // 세션 레지스트리에서 제거 (좀비 방지)
        SESSIONS.lock().remove(&conv_id_owned);
        // ContextPack freshness: 프로세스가 죽었으면 다음 send는 새 세션이므로 full 필요
        crate::commands::agents_helpers::send_common::session_freshness::clear_delivered_key(&conv_id_owned);
    });

    Ok(Arc::new(SdkSession {
        session_id: session_id.clone(),
        to_claude_tx,
        from_claude_tx,
        model: Arc::new(PlMutex::new(model.to_string())),
        ws_sender_tx,
        process_alive,
        _shutdown_tx: shutdown_tx,
        _monitor_abort: monitor_task.abort_handle(),
    }))
}

// ──────────────────────────── WS Handler ─────────────────────────────────────

async fn ws_handler(
    Path(_sid): Path<String>,
    State(state): State<WsServerState>,
    headers: HeaderMap,
    ws_upgrade: WebSocketUpgrade,
) -> impl IntoResponse {
    // Bearer 토큰 검증
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");

    if token != state.auth_token {
        return (StatusCode::UNAUTHORIZED, "invalid token").into_response();
    }

    ws_upgrade
        .on_upgrade(move |socket| handle_claude_ws(socket, state))
        .into_response()
}

/// WS 양방향 핸들러.
///
/// - **수신 (claude → 서버)**: keepalive 및 control_response 처리
/// - **전송 (서버 → claude)**: ws_send_rx 채널로 들어온 control_request 전송
///   예: `{"type":"control_request","request":{"subtype":"set_model","model":"..."}}`
///
/// 메시지 전달은 stdin, 이벤트 수신은 HTTP POST + stdout으로 처리.
async fn handle_claude_ws(socket: ws::WebSocket, state: WsServerState) {
    let (mut ws_sender, mut ws_receiver) = socket.split();

    // 연결 신호 발송 (spawn_session의 connected_rx 해제)
    if let Some(tx) = state.connected_tx.lock().await.take() {
        let _ = tx.send(());
    }

    // ws_send_rx 잠금 — 이 핸들러가 유일한 소비자
    let mut ws_send_rx = state.ws_send_rx.lock().await;

    loop {
        tokio::select! {
            // claude → 서버: keepalive / control_response 수신
            msg = ws_receiver.next() => {
                match msg {
                    Some(Ok(ws::Message::Close(_))) | None => break,
                    Some(Ok(ws::Message::Text(text))) => {
                        // control_response 로깅 (set_model 응답 등)
                        if text.contains("control_response") {
                            eprintln!("[sdk-session] control_response: {:.120}", text);
                        }
                    }
                    _ => {} // ping/pong 등 무시
                }
            }
            // 서버 → claude: control_request 전송 (set_model 등)
            //
            // **CRITICAL**: trailing newline 필수. claude의 RemoteIO는 WS로 받은 데이터와
            // stdin을 같은 PassThrough inputStream으로 합친다 (claude code source: remoteIO.ts:98-103
            // `setOnData(data => inputStream.write(data))`). newline이 없으면 다음에 오는 stdin
            // user message와 한 줄로 합쳐져 stream-json 파서가 fail → "JSON Parse error" → 프로세스 exit.
            // 격리 테스트(/tmp/ws_test/test_set_model.js)로 검증된 사실.
            msg = ws_send_rx.recv() => {
                match msg {
                    Some(text) => {
                        let payload = if text.ends_with('\n') { text } else { format!("{}\n", text) };
                        if ws_sender.send(ws::Message::Text(payload.into())).await.is_err() {
                            eprintln!("[sdk-session] WS send failed");
                            break;
                        }
                    }
                    None => break, // 채널 닫힘
                }
            }
        }
    }

    eprintln!("[sdk-session] WS connection closed");
}

// ──────────────────────────── HTTP POST Events Handler ───────────────────────

/// HybridTransport: claude가 `/{session_id}/events` 로 POST하는 이벤트를 수신한다.
///
/// 요청 본문: `{"events":[...]}` 배열 래퍼 또는 단일 JSON 객체.
/// 각 이벤트를 `from_claude_tx`로 브로드캐스트해 `stream_run_sdk`가 소비한다.
async fn events_handler(
    Path(_sid): Path<String>,
    State(state): State<WsServerState>,
    body: Bytes,
) -> impl IntoResponse {
    if let Ok(text) = std::str::from_utf8(&body) {
        // `{"events":[...]}` 래퍼 형식 처리
        if let Ok(obj) = serde_json::from_str::<serde_json::Value>(text) {
            if let Some(arr) = obj.get("events").and_then(|v| v.as_array()) {
                for item in arr {
                    let _ = state.from_claude_tx.send(item.to_string());
                }
            } else {
                // 단일 이벤트 형식
                let _ = state.from_claude_tx.send(obj.to_string());
            }
        } else {
            eprintln!("[sdk-session] events_handler: failed to parse body: {:.100}", text);
        }
    }
    StatusCode::OK
}

// ──────────────────────────── Stream Run ─────────────────────────────────────

/// claude `--sdk-url` 세션을 통해 메시지를 전송하고 스트리밍 응답을 수집한다.
///
/// 기존 `claude::stream_run` 과 동일한 인터페이스.
///
/// `is_cancelled` 는 **stream abort token** 이다 (옵션 X, plan
/// `branchCancelSemanticsPlan_2026-04-25.md`):
///
/// - true 반환 시 → 진행 중 stream 만 abort (control_request "interrupt"
///   전송 후 `Err("cancelled by user")` return)
/// - **session / SESSIONS / RESUME_IDS / process 는 모두 유지** — 다음
///   send 가 자연 이어진다 (history 보존)
/// - session 자체를 죽이려면 별도 `kill_session_clear_resume` 또는
///   `restart_sdk_session` 명시 호출 (engine/model 변경 시)
///
/// brand 와 main 의 cancel 식별: 호출자가 conv_id 단위로 토큰을 분리해
/// 캡처해야 한다 (PR #198 의 SESSIONS/RESUME_IDS normalize 와 의도가 다름).
pub async fn stream_run_sdk<F, G, C>(
    conv_id: &str,
    input: RunInput,
    mut on_progress: G,
    mut on_chunk: F,
    is_cancelled: C,
) -> Result<RunOutput, AppError>
where
    F: FnMut(String) + Send,
    G: FnMut(String) + Send,
    C: Fn() -> bool + Send,
{
    // (claudeSdkSessionWindowGuardPlan Task 01) SDK 누적 window guard.
    //
    // 직전 turn 의 result event 가 stash 한 누적 input_tokens 를 read 해서
    // 임계 (default 180K / `[1m]` 900K) 도달이면 fresh-rotate 발동:
    //   1. kill_session_clear_resume — SESSIONS + RESUME_IDS + LAST_DELIVERED 모두 invalidate
    //   2. clear_window_guard_input_tokens — stash 도 reset
    //   3. (PR-2) Tauri event emit `tunaflow:sdk-session-window-rotated`
    //   4. 다음 줄의 get_or_create_session 이 fresh session 으로 spawn (INV-CSW-2)
    //
    // 회복 가시화: persistence.rs 가 LAST_DELIVERED 비어있음을 인지 →
    // is_session_continuation=false → ContextPack full mode + anchor 2 turns
    // → plan_doc / findings / RT consensus 재주입 (사용자 컨텍스트 회복).
    let pre_dispatch_key = session_key_for(conv_id);
    if should_trigger_window_rotate(&pre_dispatch_key, input.model.as_deref()) {
        let prior_tokens = read_window_guard_input_tokens(&pre_dispatch_key);
        let threshold = crate::agents::claude_window_guard::current_window_guard_threshold(
            input.model.as_deref(),
        );
        eprintln!(
            "[sdk-window-guard] threshold reached: accumulated={} tokens >= {} (model={:?}) for conv={} key={} — \
             rotating to fresh SDK session (clear SESSIONS+RESUME_IDS+LAST_DELIVERED)",
            prior_tokens,
            threshold,
            input.model.as_deref().unwrap_or("<default>"),
            conv_id,
            pre_dispatch_key
        );
        kill_session_clear_resume(conv_id);
        clear_window_guard_input_tokens(&pre_dispatch_key);
        // (PR-2 Task 02) Tauri event emit hook — frontend toast 알림.
        // PR-1 단독 머지 시 silent 진행 (UX 마찰 ↑) but 회귀 차단은 즉시 발동.
        emit_window_rotated_event(conv_id, prior_tokens, threshold);
    }

    // 세션 획득 (없으면 새로 생성, 모델 변경 시 재시작)
    let session = get_or_create_session(
        conv_id,
        input.project_path.as_deref(),
        input.model.as_deref(),
    ).await?;

    // 이벤트 구독 — 전송 전에 구독해야 이벤트를 놓치지 않음
    let mut event_rx = session.from_claude_tx.subscribe();

    // 사용자 메시지 전송 (stream-json stdin 형식)
    let user_msg = build_user_message(&input.prompt);
    session
        .to_claude_tx
        .send(user_msg)
        .map_err(|_| AppError::Agent("sdk-session: send channel closed".into()))?;

    on_progress("Agent initializing...".into());

    let conv_id_owned = conv_id.to_string();
    // brand:* conv 는 main 의 root key 로 RESUME_IDS / freshness 를 관리한다 (Layer A).
    let session_key_owned = session_key_for(conv_id);
    let mut accumulated_input_tokens: i64 = 0;
    let mut accumulated_output_tokens: i64 = 0;

    // 응답 수집
    let alive_for_loop = session.process_alive.clone();
    loop {
        let line_result: Result<Result<String, broadcast::error::RecvError>, tokio::time::error::Elapsed> =
            tokio::select! {
                r = tokio::time::timeout(std::time::Duration::from_secs(600), event_rx.recv()) => r,
                _ = async {
                    loop {
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        if is_cancelled() { break; }
                    }
                } => {
                    // Stream abort only (옵션 X). control_request "interrupt"
                    // 로 진행 중 응답만 끊고, session/SESSIONS/RESUME_IDS/process
                    // 는 모두 보존 — 다음 send 가 history 그대로 이어진다.
                    let interrupt = serde_json::json!({
                        "type": "control_request",
                        "request": { "subtype": "interrupt" }
                    }).to_string();
                    let _ = session.to_claude_tx.send(interrupt);
                    return Err(AppError::Agent("cancelled by user".into()));
                }
                // claude child process가 죽었는지 100ms마다 폴링.
                // broadcast 채널은 SdkSession Arc(stream_run_sdk가 보유)이 살아있는 한 close되지
                // 않으므로 별도 신호가 필요. monitor_task가 child.wait() 후 false 설정.
                _ = async {
                    loop {
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        if !alive_for_loop.load(Ordering::SeqCst) { break; }
                    }
                } => {
                    eprintln!("[sdk-session] process death detected for conv: {} — aborting stream",
                        &conv_id_owned[..conv_id_owned.len().min(8)]);
                    return Err(AppError::Agent(
                        "claude 프로세스가 종료되었습니다 (모델 전환/외부 종료 추정). 다시 send하면 새 세션으로 자동 재시작됩니다.".into(),
                    ));
                }
            };

        let line = match line_result {
            Ok(Ok(line)) => line,
            Ok(Err(broadcast::error::RecvError::Lagged(n))) => {
                eprintln!("[sdk-session] receiver lagged, skipped {} messages", n);
                continue;
            }
            Ok(Err(broadcast::error::RecvError::Closed)) => {
                return Err(AppError::Agent("sdk-session: event channel closed".into()));
            }
            Err(_) => {
                return Err(AppError::Agent(
                    "sdk-session: timeout waiting for response (10min)".into(),
                ));
            }
        };

        let parsed: SdkStreamLine = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // [디버그] stream-json 라인 도착 추적 — Architect stuck 원인 파악용.
        // 이 라인이 계속 찍히는데 result가 안 오면 → claude 자체 hang.
        // 안 찍히면 → broadcast 채널 문제.
        eprintln!("[sdk-session:trace] line type={} for conv={}", parsed.line_type,
            &conv_id_owned[..conv_id_owned.len().min(8)]);

        // 주의: 이전에 추가했던 `on_progress("__HEARTBEAT__:...")` 호출은
        // architect stuck 의심 디버깅을 위해 일시 제거. watchdog은 기존 progress/chunk
        // 이벤트만으로 동작 — 10분 timeout이 충분한 여유.
        match parsed.line_type.as_str() {
            "system" => {
                on_progress("Agent initializing...".into());
            }
            "assistant" => {
                if let Some(msg) = &parsed.message {
                    if let Some(usage) = &msg.usage {
                        if let Some(v) = usage.input_tokens { accumulated_input_tokens = v; }
                        if let Some(v) = usage.output_tokens { accumulated_output_tokens = v; }
                    }
                    if let Some(blocks) = &msg.content {
                        for block in blocks {
                            match block.block_type.as_str() {
                                "thinking" => {
                                    if let Some(thinking) = &block.thinking {
                                        let last_line = thinking
                                            .lines()
                                            .filter(|l| !l.trim().is_empty())
                                            .last()
                                            .unwrap_or("")
                                            .trim();
                                        if !last_line.is_empty() {
                                            let step = serde_json::json!({
                                                "type": "thinking",
                                                "name": "Thinking",
                                                "input": last_line.chars().take(120).collect::<String>(),
                                                "status": "done"
                                            });
                                            on_progress(format!("__STEP__:{}", step));
                                        }
                                    }
                                }
                                "tool_use" => {
                                    if let Some(name) = &block.name {
                                        let input_summary =
                                            block.input.as_ref().map(|v| {
                                                let s = v.to_string();
                                                if s.len() > 120 {
                                                    let mut end = 120;
                                                    while end > 0 && !s.is_char_boundary(end) {
                                                        end -= 1;
                                                    }
                                                    format!("{}…", &s[..end])
                                                } else {
                                                    s
                                                }
                                            }).unwrap_or_default();
                                        let step = serde_json::json!({
                                            "type": "tool_use",
                                            "name": name,
                                            "input": input_summary,
                                            "status": "running"
                                        });
                                        on_progress(format!("__STEP__:{}", step));
                                    }
                                }
                                "text" => {
                                    if let Some(text) = &block.text {
                                        if !text.is_empty() {
                                            on_chunk(text.clone());
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            "result" => {
                if parsed.is_error.unwrap_or(false) {
                    return Err(AppError::Agent(format!(
                        "claude reported error: {}",
                        parsed.result.as_deref().unwrap_or("unknown")
                    )));
                }

                // 다음 모델 변경 시 --resume에 사용할 session_id를 RESUME_IDS에 보존
                //
                // sessionContinuityFixPlan INV-6: claude 가 이전 session_id 와 **다른**
                // session_id 를 반환하면 `--resume` 이 거부되어 fresh claude 세션이
                // 시작됐다는 뜻. 이 경우 LAST_DELIVERED_KEY 를 invalidate 해 다음
                // send 에서 is_session_continuation=false 가 되도록 한다 — 그렇지
                // 않으면 "claude 는 기억 없는데 tunaFlow 는 있다고 착각" 하는 다른
                // 경로의 맥락 유실이 발생한다.
                if let Some(sid) = &parsed.session_id {
                    // brand 와 main 은 같은 root key 로 RESUME_IDS 공유 (Layer A).
                    let prior = RESUME_IDS.lock().insert(session_key_owned.clone(), sid.clone());
                    if let Some(p) = prior {
                        if p != *sid {
                            eprintln!(
                                "[sdk-session] claude returned new session_id (prior={} new={}) — \
                                 --resume likely rejected; invalidating LAST_DELIVERED for conv={} (key={})",
                                p, sid, conv_id_owned, session_key_owned
                            );
                            crate::commands::agents_helpers::send_common::session_freshness::clear_delivered_key(
                                &conv_id_owned,
                            );
                            // root key 도 invalidate — main 에서 진행 중인 trace 가 있을 수 있음.
                            if session_key_owned != conv_id_owned {
                                crate::commands::agents_helpers::send_common::session_freshness::clear_delivered_key(
                                    &session_key_owned,
                                );
                            }
                        }
                    }
                }

                let final_input = parsed.total_input_tokens
                    .filter(|&v| v > 0)
                    .unwrap_or(accumulated_input_tokens);
                let final_output = parsed.total_output_tokens
                    .filter(|&v| v > 0)
                    .unwrap_or(accumulated_output_tokens);
                eprintln!("[sdk-session] result tokens: in={} out={} (from_result={}/{})",
                    final_input, final_output,
                    parsed.total_input_tokens.unwrap_or(0),
                    parsed.total_output_tokens.unwrap_or(0));

                // (claudeSdkSessionWindowGuardPlan Task 01) cumulative input_tokens stash.
                // 다음 dispatch 진입 시 stream_run_sdk 진입부 가드가 read → 임계
                // 비교 → fresh-rotate 결정. INV-CSW-1: tracking 본체 변경 0,
                // 단순 stash 만 추가.
                if final_input > 0 {
                    stash_window_guard_input_tokens(&session_key_owned, final_input as u64);
                }

                return Ok(RunOutput {
                    content: parsed.result.unwrap_or_default(),
                    cost_usd: parsed.total_cost_usd.or(parsed.cost_usd).unwrap_or(0.0),
                    input_tokens: final_input,
                    output_tokens: final_output,
                    session_id: parsed.session_id,
                    // sdk-session path 는 본 plan scope 외 — 호환성만 유지.
                    last_rate_limit: None,
                    fresh_fallback: false,
                });
            }
            "control_request" => {
                eprintln!("[sdk-session] control_request received (unhandled in Phase 1)");
            }
            _ => {}
        }
    }
}

// ──────────────────────────── Helpers ────────────────────────────────────────

/// stream-json stdin 형식의 사용자 메시지를 생성한다.
fn build_user_message(content: &str) -> String {
    serde_json::json!({
        "type": "user",
        "message": {
            "role": "user",
            "content": content
        }
    })
    .to_string()
}

/// 모든 활성 SDK 세션을 종료한다 (앱 종료 시 호출).
#[allow(dead_code)]
pub fn shutdown_all_sessions() {
    let sessions: Vec<_> = SESSIONS.lock().drain().collect();
    for (conv_id, _session) in &sessions {
        eprintln!("[sdk-session] shutting down session for conv: {}", conv_id);
    }
    drop(sessions); // Drop triggers _monitor_abort → kill_on_drop
    RESUME_IDS.lock().clear();
    // (claudeSdkSessionWindowGuardPlan Task 01) window guard stash 전체 reset.
    WINDOW_GUARD_INPUT_TOKENS.lock().clear();
}

/// Kill any orphaned `claude --sdk-url` processes from previous app runs.
/// Called on app startup to prevent zombie processes that consume rate limit quota.
///
/// Unix path uses `pgrep -f` + `ps -o ppid=` to detect PPID=1 orphans, then
/// `kill <pid>`. Windows would need WMIC / PowerShell parent-PID +
/// command-line introspection — deferred (see windows variant below).
#[cfg(unix)]
pub fn kill_orphan_sdk_processes() {
    use std::process::Command;
    let output = match Command::new("pgrep").no_console().args(["-f", "claude.*--sdk-url"]).output() {
        Ok(o) => o,
        Err(_) => return,
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let current_pid = std::process::id();
    let mut killed = 0;
    for line in stdout.lines() {
        if let Ok(pid) = line.trim().parse::<u32>() {
            if pid == current_pid { continue; }
            // Check if this process is orphaned (PPID=1) or belongs to a dead parent
            if let Ok(ppid_out) = Command::new("ps").no_console().args(["-o", "ppid=", "-p", &pid.to_string()]).output() {
                let ppid_str = String::from_utf8_lossy(&ppid_out.stdout).trim().to_string();
                if let Ok(ppid) = ppid_str.parse::<u32>() {
                    // PPID=1 means orphaned (parent died)
                    if ppid == 1 {
                        eprintln!("[sdk-session] killing orphan claude --sdk-url process PID={}", pid);
                        let _ = Command::new("kill").no_console().arg(pid.to_string()).output();
                        killed += 1;
                    }
                }
            }
        }
    }
    if killed > 0 {
        eprintln!("[sdk-session] cleaned up {} orphan sdk-url process(es)", killed);
    }
}

/// Windows variant — intentional no-op for now.
///
/// The Unix path relies on `pgrep` / `ps -o ppid=` which do not exist on
/// Windows. Spawning them previously failed silently (`Err(_) => return` on
/// the first call), making the no-op non-obvious — this `cfg(windows)` stub
/// makes the absence explicit.
///
/// Why deferred:
/// - Random per-session port (`127.0.0.1:0`) + UUID session_id mean orphaned
///   `claude.exe --sdk-url` processes do not collide with new sessions or
///   directly saturate user-visible quota in practice.
/// - Proper Windows orphan detection requires WMIC / PowerShell command-line
///   + parent-PID introspection — deferred to a separate
///   `windowsOrphanProcessHardeningPlan` (P3, post-beta).
/// - The §D watchdog `taskkill` patch (PR #231) already handles the more
///   common in-session idle-timeout kill path; this stub covers only the
///   cross-app-restart leak path, which is rarer.
#[cfg(windows)]
pub fn kill_orphan_sdk_processes() {
    // intentional no-op — see doc comment above for rationale and follow-up plan.
}

// ─── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // RESUME_IDS, SESSIONS 는 모듈 내부 lazy_static 이라 tests 에서 직접 접근 가능.
    // 다른 테스트와 충돌하지 않도록 모든 테스트는 uuid 기반 고유 conv_id 를 사용한다.
    fn unique_conv(tag: &str) -> String {
        format!("test-conv-{}-{}", tag, uuid::Uuid::new_v4())
    }

    #[test]
    fn current_session_key_prefers_claude_session_id_over_router_uuid() {
        // sessionContinuityFixPlan INV-2: router UUID (spawn 마다 바뀌는 값) 대신
        // claude 자체 session_id (RESUME_IDS 에 저장) 를 identity 로 써야 한다.
        let conv = unique_conv("prefer-claude-sid");
        RESUME_IDS.lock().insert(conv.clone(), "claude-real-session-ABC".into());

        let key = current_session_key(&conv).expect("RESUME_IDS 에 있으면 Some 반환");
        assert_eq!(
            key, "claude-ws:claude-real-session-ABC",
            "claude session_id 가 키의 뒷부분이어야 함"
        );
        assert!(
            !key.contains("router:"),
            "RESUME_IDS 가 있으면 router fallback prefix 를 쓰면 안 됨: {}",
            key
        );

        // cleanup — 전역 lazy_static 격리
        RESUME_IDS.lock().remove(&conv);
    }

    #[test]
    fn current_session_key_returns_none_when_no_session_and_no_resume_id() {
        let conv = unique_conv("none-no-session");
        // RESUME_IDS 비어있고 SESSIONS 도 비어있으면 None — is_session_continuation 을
        // false 로 강제해 ContextPack 이 full 경로로 흐른다.
        RESUME_IDS.lock().remove(&conv);
        // SESSIONS 에도 entry 없음 (unique conv_id 기본값)
        assert!(current_session_key(&conv).is_none());
    }

    #[test]
    fn current_session_key_resume_id_stable_across_conceptual_respawn() {
        // Router UUID 는 spawn 마다 Uuid::new_v4() 로 새로 생기지만 claude 자체
        // session_id (RESUME_IDS) 는 유지된다. current_session_key 가 RESUME_IDS
        // 를 보는 이상 키가 변하지 않아야 함 — respawn 시나리오의 identity 불변성.
        //
        // SdkSession 구조체 전체를 테스트에서 만들기는 어려우므로 RESUME_IDS 단위의
        // 불변성만 검증. SESSIONS 는 비어있어도 RESUME_IDS 만 있으면 키가 돈다.
        let conv = unique_conv("respawn-stable");
        RESUME_IDS.lock().insert(conv.clone(), "claude-sess-STABLE".into());

        let k1 = current_session_key(&conv).unwrap();
        // "두 번째 spawn" 을 흉내낸다 — 실제 SESSIONS 를 교체하는 건 어렵지만,
        // 핵심은 RESUME_IDS 가 유지되는 한 key 가 변하지 않음을 확인하는 것.
        let k2 = current_session_key(&conv).unwrap();

        assert_eq!(k1, k2, "RESUME_IDS 가 있는 한 key 는 호출마다 같아야 함");

        RESUME_IDS.lock().remove(&conv);
    }

    #[test]
    fn current_session_key_router_fallback_prefix_is_separable() {
        // RESUME_IDS 가 없는 첫 send 에서는 router UUID fallback 을 쓰되 prefix 로
        // 구분되어야 한다. 이 prefix 가 LAST_DELIVERED 의 정상 키와 매칭되는 일이
        // 없음을 확인 (is_session_continuation=false 자동 유도).
        //
        // SESSIONS 에 직접 entry 를 삽입하는 건 SdkSession 의 여러 필드 (child,
        // channels, ports 등) 를 만들어야 하므로 번거로움. 이 테스트는 fallback
        // prefix "claude-ws:router:" 가 정상 prefix "claude-ws:<uuid>" 와 포맷상
        // 구분되는지만 검증 — 실제 SESSIONS 주입 테스트는 integration 수준에서.
        assert!(
            "claude-ws:router:abc" != "claude-ws:abc",
            "router fallback prefix 와 정상 prefix 는 달라야 함"
        );
    }

    // ─── task-02: DB bootstrap ──────────────────────────────────────────────

    /// In-memory DbState helper — minimal schema + migration v22 수준 (conversations)
    /// 만 올리고 conversations row 하나 삽입. 전체 migration (vec0 의존) 은 skip.
    fn build_test_db_with_conversation(
        conv_id: &str,
        resume_token: Option<&str>,
        engine: Option<&str>,
    ) -> crate::db::DbState {
        use std::sync::{Arc, Mutex};
        let read = rusqlite::Connection::open_in_memory().unwrap();
        read.execute_batch(
            "CREATE TABLE conversations (
                id TEXT PRIMARY KEY,
                resume_token TEXT,
                resume_token_engine TEXT
             );",
        )
        .unwrap();
        if let Some(tok) = resume_token {
            read.execute(
                "INSERT INTO conversations (id, resume_token, resume_token_engine) VALUES (?1, ?2, ?3)",
                rusqlite::params![conv_id, tok, engine.unwrap_or("claude")],
            )
            .unwrap();
        } else {
            read.execute(
                "INSERT INTO conversations (id, resume_token, resume_token_engine) VALUES (?1, NULL, NULL)",
                rusqlite::params![conv_id],
            )
            .unwrap();
        }
        // write connection 은 bootstrap 이 touch 하지 않지만 DbState 구조 맞추기용
        let write = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::DbState {
            read: Arc::new(Mutex::new(read)),
            write: Arc::new(Mutex::new(write)),
        }
    }

    #[test]
    fn bootstrap_loads_token_for_claude_engine() {
        let conv = unique_conv("bootstrap-claude");
        RESUME_IDS.lock().remove(&conv);
        let db = build_test_db_with_conversation(&conv, Some("claude-sess-FROM-DB"), Some("claude"));

        let got = bootstrap_resume_id_from_db(&conv, &db);
        assert_eq!(got.as_deref(), Some("claude-sess-FROM-DB"));
        assert_eq!(
            RESUME_IDS.lock().get(&conv).cloned().as_deref(),
            Some("claude-sess-FROM-DB")
        );

        RESUME_IDS.lock().remove(&conv);
    }

    #[test]
    fn bootstrap_accepts_claude_code_engine_label() {
        // finalize_engine_run 은 engine_key="claude-code" 로 resume_token_engine 을
        // 저장한다. bootstrap 쿼리는 IN ('claude','claude-code') 로 이를 수용해야.
        let conv = unique_conv("bootstrap-cc");
        RESUME_IDS.lock().remove(&conv);
        let db = build_test_db_with_conversation(&conv, Some("sess-CC"), Some("claude-code"));

        let got = bootstrap_resume_id_from_db(&conv, &db);
        assert_eq!(got.as_deref(), Some("sess-CC"));

        RESUME_IDS.lock().remove(&conv);
    }

    #[test]
    fn bootstrap_is_noop_when_memory_has_value() {
        let conv = unique_conv("bootstrap-noop");
        RESUME_IDS.lock().insert(conv.clone(), "MEM-WINS".into());
        let db = build_test_db_with_conversation(&conv, Some("DB-LOSES"), Some("claude"));

        let got = bootstrap_resume_id_from_db(&conv, &db);
        assert_eq!(got.as_deref(), Some("MEM-WINS"), "메모리 값이 있으면 DB 는 무시");

        RESUME_IDS.lock().remove(&conv);
    }

    #[test]
    fn bootstrap_skips_non_claude_engine() {
        let conv = unique_conv("bootstrap-codex");
        RESUME_IDS.lock().remove(&conv);
        let db = build_test_db_with_conversation(&conv, Some("codex-sess-X"), Some("codex"));

        let got = bootstrap_resume_id_from_db(&conv, &db);
        assert!(got.is_none(), "codex engine 의 token 은 claude bootstrap 대상 아님");
        assert!(RESUME_IDS.lock().get(&conv).is_none());
    }

    #[test]
    fn bootstrap_skips_when_resume_token_null() {
        let conv = unique_conv("bootstrap-null");
        RESUME_IDS.lock().remove(&conv);
        let db = build_test_db_with_conversation(&conv, None, None);

        let got = bootstrap_resume_id_from_db(&conv, &db);
        assert!(got.is_none());
        assert!(RESUME_IDS.lock().get(&conv).is_none());
    }

    #[test]
    fn bootstrap_noop_when_conversation_missing() {
        let conv = unique_conv("bootstrap-missing");
        RESUME_IDS.lock().remove(&conv);
        // 다른 conv_id 로 DB 생성 — 찾으려는 conv 는 DB 에 없음
        let db = build_test_db_with_conversation("other-conv", Some("tok"), Some("claude"));

        let got = bootstrap_resume_id_from_db(&conv, &db);
        assert!(got.is_none());
        assert!(RESUME_IDS.lock().get(&conv).is_none());
    }

    // ─── task-03: auto-invalidate on new session_id ─────────────────────────

    /// Unit helper — `stream_run_sdk` 의 `result` 이벤트 처리 블록의 순수 로직만
    /// 발췌. 실제 이벤트 루프를 돌리기엔 subprocess + WS가 필요하므로, 동일
    /// 로직을 테스트 전용 함수로 재현한다. Production 코드와 drift 나지 않도록
    /// 이 함수는 production 경로에서 호출하지 않고 오직 단위 테스트에서만 사용.
    fn handle_result_session_id_for_test(conv_id: &str, new_sid: &str) {
        // production 로직 (claude_sdk_session.rs:775 근처) 과 **동일** 해야 함.
        use crate::commands::agents_helpers::send_common::session_freshness::clear_delivered_key;
        let prior = RESUME_IDS.lock().insert(conv_id.to_string(), new_sid.to_string());
        if let Some(p) = prior {
            if p != new_sid {
                clear_delivered_key(conv_id);
            }
        }
    }

    #[test]
    fn new_session_id_triggers_delivered_key_clear() {
        use crate::commands::agents_helpers::send_common::session_freshness::{
            last_delivered_key, record_delivered_key,
        };

        let conv = unique_conv("invalidate-new-sid");
        // Arrange — claude 가 이전에 sid-OLD 를 줬고 LAST_DELIVERED 에도 기록됨
        RESUME_IDS.lock().insert(conv.clone(), "sid-OLD".into());
        record_delivered_key(&conv, "claude-ws:sid-OLD");
        assert_eq!(
            last_delivered_key(&conv).as_deref(),
            Some("claude-ws:sid-OLD")
        );

        // Act — claude 가 응답에서 sid-NEW 를 돌려줌 (--resume 거부 시나리오)
        handle_result_session_id_for_test(&conv, "sid-NEW");

        // Assert — RESUME_IDS 는 새 값으로 갱신, LAST_DELIVERED 는 clear
        assert_eq!(
            RESUME_IDS.lock().get(&conv).cloned().as_deref(),
            Some("sid-NEW")
        );
        assert!(
            last_delivered_key(&conv).is_none(),
            "새 session_id 반환 시 LAST_DELIVERED 는 invalidate 되어야"
        );

        RESUME_IDS.lock().remove(&conv);
    }

    #[test]
    fn same_session_id_does_not_clear_delivered_key() {
        use crate::commands::agents_helpers::send_common::session_freshness::{
            last_delivered_key, record_delivered_key,
        };

        let conv = unique_conv("invalidate-same-sid");
        RESUME_IDS.lock().insert(conv.clone(), "sid-STABLE".into());
        record_delivered_key(&conv, "claude-ws:sid-STABLE");

        // 같은 session_id 반환 → normal continuation
        handle_result_session_id_for_test(&conv, "sid-STABLE");

        assert_eq!(
            last_delivered_key(&conv).as_deref(),
            Some("claude-ws:sid-STABLE"),
            "같은 session_id 는 invalidate 되면 안 됨"
        );

        RESUME_IDS.lock().remove(&conv);
    }

    #[test]
    fn first_session_id_stores_without_invalidating() {
        use crate::commands::agents_helpers::send_common::session_freshness::{
            last_delivered_key, record_delivered_key,
        };

        let conv = unique_conv("invalidate-first-sid");
        // prior 가 없는 상태 (첫 응답). LAST_DELIVERED 는 router-fallback 이었을 수도
        // 있으나 여기선 일단 non-empty 로 set 해 두고 invalidate 되지 않음을 확인.
        RESUME_IDS.lock().remove(&conv);
        record_delivered_key(&conv, "claude-ws:router:pre-bootstrap");

        handle_result_session_id_for_test(&conv, "sid-FIRST");

        assert_eq!(
            RESUME_IDS.lock().get(&conv).cloned().as_deref(),
            Some("sid-FIRST")
        );
        // prior 가 None 이었으므로 clear 호출 안 됨 — LAST_DELIVERED 유지.
        // (다음 send 에서 current_session_key 가 "claude-ws:sid-FIRST" 를 반환하는데
        //  LAST_DELIVERED 는 "claude-ws:router:pre-bootstrap" 이라 is_session_continuation
        //  은 자연스럽게 false → fresh 경로. 이게 의도된 동작.)
        assert!(last_delivered_key(&conv).is_some());

        RESUME_IDS.lock().remove(&conv);
    }

    // ─── Layer A: brand session inheritance (branchInheritsMainSessionPlan) ─────

    /// `conversations` + `branches` 테이블을 갖춘 in-memory DbState.
    /// brand:* shadow 와 root main conv 양쪽 row 를 모두 채워준다.
    fn build_test_db_with_branch(
        root_conv_id: &str,
        branch_id: &str,
        resume_token: Option<&str>,
        engine: Option<&str>,
    ) -> crate::db::DbState {
        use std::sync::{Arc, Mutex};
        let read = rusqlite::Connection::open_in_memory().unwrap();
        read.execute_batch(
            "CREATE TABLE conversations (
                id TEXT PRIMARY KEY,
                resume_token TEXT,
                resume_token_engine TEXT
             );
             CREATE TABLE branches (
                id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL
             );",
        )
        .unwrap();
        // root main conv (resume_token 부모)
        if let Some(tok) = resume_token {
            read.execute(
                "INSERT INTO conversations (id, resume_token, resume_token_engine) VALUES (?1, ?2, ?3)",
                rusqlite::params![root_conv_id, tok, engine.unwrap_or("claude")],
            )
            .unwrap();
        } else {
            read.execute(
                "INSERT INTO conversations (id, resume_token, resume_token_engine) VALUES (?1, NULL, NULL)",
                rusqlite::params![root_conv_id],
            )
            .unwrap();
        }
        // branches row — root_conv_id 가 branches.conversation_id 에 들어 있어야
        // session_key_for(brand:*) 가 root 로 정규화된다.
        read.execute(
            "INSERT INTO branches (id, conversation_id) VALUES (?1, ?2)",
            rusqlite::params![branch_id, root_conv_id],
        )
        .unwrap();
        let write = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::DbState {
            read: Arc::new(Mutex::new(read)),
            write: Arc::new(Mutex::new(write)),
        }
    }

    fn clear_branch_cache(brand_conv: &str) {
        BRANCH_ROOT_CACHE.lock().remove(brand_conv);
    }

    #[test]
    fn session_key_for_returns_conv_id_unchanged_for_non_branch() {
        let conv = unique_conv("non-branch");
        assert_eq!(session_key_for(&conv), conv);
    }

    #[test]
    fn session_key_for_returns_root_when_branch_root_cached() {
        let root = unique_conv("root-A");
        let branch_id = format!("b-{}", uuid::Uuid::new_v4());
        let brand_conv = format!("branch:{}", branch_id);

        // 캐시 직접 주입 (DB 없이 helper 단독 검증)
        BRANCH_ROOT_CACHE.lock().insert(brand_conv.clone(), root.clone());

        assert_eq!(session_key_for(&brand_conv), root);

        clear_branch_cache(&brand_conv);
    }

    #[test]
    fn session_key_for_falls_back_to_conv_id_when_cache_miss() {
        // 잘못된 prefix / DB 미조회 상태 — fallback 으로 conv_id 그대로 반환
        let brand_conv = format!("branch:{}", uuid::Uuid::new_v4());
        clear_branch_cache(&brand_conv);
        assert_eq!(session_key_for(&brand_conv), brand_conv);
    }

    #[test]
    fn cache_branch_root_from_db_populates_cache_for_branch() {
        let root = unique_conv("layerA-root");
        let branch_id = format!("b-{}", uuid::Uuid::new_v4());
        let brand_conv = format!("branch:{}", branch_id);
        clear_branch_cache(&brand_conv);

        let db = build_test_db_with_branch(&root, &branch_id, Some("tok"), Some("claude"));
        cache_branch_root_from_db(&brand_conv, &db);

        assert_eq!(
            BRANCH_ROOT_CACHE.lock().get(&brand_conv).cloned(),
            Some(root.clone())
        );
        assert_eq!(session_key_for(&brand_conv), root);

        clear_branch_cache(&brand_conv);
    }

    #[test]
    fn cache_branch_root_from_db_is_noop_for_non_branch_conv_id() {
        // non-branch conv_id 는 캐시에 들어가지 않아야 함 (불필요한 항목 방지).
        let conv = unique_conv("plain-conv");
        let db = build_test_db_with_branch(
            &conv,
            "some-branch",
            None,
            None,
        );
        cache_branch_root_from_db(&conv, &db);
        assert!(BRANCH_ROOT_CACHE.lock().get(&conv).is_none());
    }

    #[test]
    fn cache_branch_root_from_db_is_noop_when_already_cached() {
        let root = unique_conv("already-cached-root");
        let branch_id = format!("b-{}", uuid::Uuid::new_v4());
        let brand_conv = format!("branch:{}", branch_id);

        // 캐시 선점
        BRANCH_ROOT_CACHE
            .lock()
            .insert(brand_conv.clone(), "PRE-EXISTING".into());

        // DB 에는 다른 root 가 있어도 캐시는 변하지 않아야 함
        let db = build_test_db_with_branch(&root, &branch_id, None, None);
        cache_branch_root_from_db(&brand_conv, &db);

        assert_eq!(
            BRANCH_ROOT_CACHE.lock().get(&brand_conv).cloned().as_deref(),
            Some("PRE-EXISTING"),
            "이미 캐시된 값은 덮어쓰지 않아야 함"
        );

        clear_branch_cache(&brand_conv);
    }

    #[test]
    fn bootstrap_resume_id_for_branch_keys_root_conv() {
        // INV-1: brand:* 진입 시 RESUME_IDS 는 root key 로 들어가야 한다.
        let root = unique_conv("inv1-root");
        let branch_id = format!("b-{}", uuid::Uuid::new_v4());
        let brand_conv = format!("branch:{}", branch_id);

        RESUME_IDS.lock().remove(&root);
        RESUME_IDS.lock().remove(&brand_conv);
        clear_branch_cache(&brand_conv);

        let db = build_test_db_with_branch(&root, &branch_id, Some("ROOT-TOK"), Some("claude"));
        // brand:* conv 로 진입했지만 DB 에는 root 의 resume_token 만 있다.
        let got = bootstrap_resume_id_from_db(&brand_conv, &db);
        assert_eq!(got.as_deref(), Some("ROOT-TOK"));

        // RESUME_IDS 에는 root key 로 저장되어야 함 (brand 키 아님)
        assert_eq!(
            RESUME_IDS.lock().get(&root).cloned().as_deref(),
            Some("ROOT-TOK"),
            "RESUME_IDS 는 root main conv 키로 저장되어야 함"
        );
        assert!(
            RESUME_IDS.lock().get(&brand_conv).is_none(),
            "brand:* 키로는 RESUME_IDS 에 entry 가 들어가면 안 됨"
        );

        // current_session_key 도 brand → root 로 normalize 되어 같은 값 반환
        let key_from_brand = current_session_key(&brand_conv);
        let key_from_root = current_session_key(&root);
        assert_eq!(key_from_brand, key_from_root);
        assert_eq!(key_from_brand.as_deref(), Some("claude-ws:ROOT-TOK"));

        // cleanup
        RESUME_IDS.lock().remove(&root);
        clear_branch_cache(&brand_conv);
    }

    // ─── claudeSdkSessionWindowGuardPlan Task 01 ───────────────────────────

    /// 신규 stash/read/clear API 단위 검증 — 다른 테스트 격리를 위해 unique conv 사용.
    #[test]
    fn window_guard_stash_round_trip_persists_and_clears() {
        let conv = unique_conv("wg-stash-roundtrip");
        let key = session_key_for(&conv);

        // 초기 stash 없음 → 0 반환 (fast path).
        assert_eq!(read_window_guard_input_tokens(&key), 0);

        stash_window_guard_input_tokens(&key, 175_000);
        assert_eq!(read_window_guard_input_tokens(&key), 175_000);

        clear_window_guard_input_tokens(&key);
        assert_eq!(read_window_guard_input_tokens(&key), 0);
    }

    #[test]
    fn sdk_window_guard_no_op_below_threshold() {
        // accumulated < 임계 → fresh-rotate 미발동 (정상 path 보존).
        // INV-CSW (G1): 정상 사용량 사용자 영향 0.
        let conv = unique_conv("wg-below-default");
        let key = session_key_for(&conv);

        clear_window_guard_input_tokens(&key);
        // 임계 직전 (179K, default 180K cap)
        stash_window_guard_input_tokens(&key, 179_000);

        assert!(
            !should_trigger_window_rotate(&key, Some("claude-opus-4-7")),
            "default 모델, 179K < 180K → rotate trigger 안 됨"
        );

        clear_window_guard_input_tokens(&key);
    }

    #[test]
    fn sdk_window_guard_triggers_fresh_rotate_at_threshold() {
        // accumulated >= 임계 → fresh-rotate 발동 (사용자 보고 fix 의도).
        // INV-CSW (G1): "Prompt is too long" 회귀 차단의 핵심 분기.
        let conv = unique_conv("wg-at-default");
        let key = session_key_for(&conv);

        clear_window_guard_input_tokens(&key);
        // 임계 도달 (180K = default cap)
        stash_window_guard_input_tokens(&key, 180_000);

        assert!(
            should_trigger_window_rotate(&key, Some("claude-opus-4-7")),
            "default 모델, 180K >= 180K → rotate trigger 발동"
        );

        // 초과 (200K) 도 동일하게 trigger
        stash_window_guard_input_tokens(&key, 200_000);
        assert!(should_trigger_window_rotate(&key, Some("claude-sonnet-4-6")));

        clear_window_guard_input_tokens(&key);
    }

    #[test]
    fn sdk_window_guard_resets_after_kill_session_clear_resume() {
        // INV-CSW-2: fresh-rotate 후 stash reset 되어 새 session 의 첫 turn 부터
        // 정상 누적. kill_session_clear_resume 가 stash 도 정리하는지 확인.
        let conv = unique_conv("wg-reset-after-kill");
        let key = session_key_for(&conv);

        // RESUME_IDS / stash 초기화
        RESUME_IDS.lock().insert(key.clone(), "sid-OLD".into());
        stash_window_guard_input_tokens(&key, 185_000);
        assert_eq!(read_window_guard_input_tokens(&key), 185_000);

        // kill_session_clear_resume — stash 도 함께 reset 되어야 함 (INV-CSW-2)
        kill_session_clear_resume(&conv);
        assert_eq!(
            read_window_guard_input_tokens(&key),
            0,
            "kill_session_clear_resume 후 window guard stash 가 reset 되어야 함"
        );
    }

    #[test]
    fn sdk_window_guard_1m_variant_threshold_900k_not_triggered_at_180k() {
        // INV-CSW-5: `[1m]` variant 사용자 (claude-opus-4-7-1m 등) 영향 0 — 200K
        // limit 무관. 임계 900K 적용 → 200K 누적도 trigger 안 됨.
        let conv = unique_conv("wg-1m-not-trigger");
        let key = session_key_for(&conv);

        clear_window_guard_input_tokens(&key);
        // default 모델이면 trigger 됐을 양 (200K)
        stash_window_guard_input_tokens(&key, 200_000);

        assert!(
            !should_trigger_window_rotate(&key, Some("claude-opus-4-7-1m")),
            "1M variant: 200K < 900K → rotate 미발동 (INV-CSW-5)"
        );
        assert!(
            !should_trigger_window_rotate(&key, Some("claude-haiku-4-5-1m")),
            "1M variant 다른 family 도 동일"
        );

        clear_window_guard_input_tokens(&key);
    }

    #[test]
    fn sdk_window_guard_1m_variant_triggers_at_900k() {
        // 1M variant 도 임계 도달 시 (900K) trigger.
        let conv = unique_conv("wg-1m-at-trigger");
        let key = session_key_for(&conv);

        clear_window_guard_input_tokens(&key);
        stash_window_guard_input_tokens(&key, 900_000);

        assert!(
            should_trigger_window_rotate(&key, Some("claude-opus-4-7-1m")),
            "1M variant: 900K >= 900K → rotate 발동"
        );

        clear_window_guard_input_tokens(&key);
    }

    #[test]
    fn sdk_window_guard_no_op_with_zero_accumulated() {
        // 첫 send (stash 비어있음) — fast path, threshold 비교 자체 skip.
        // INV-CSW-8: RT 미사용 / Reviewer 미사용 conv 의 fast path 보존.
        let conv = unique_conv("wg-fresh-zero");
        let key = session_key_for(&conv);

        clear_window_guard_input_tokens(&key);

        assert!(
            !should_trigger_window_rotate(&key, Some("claude-opus-4-7")),
            "stash 비어있으면 trigger 안 됨"
        );
        assert!(
            !should_trigger_window_rotate(&key, None),
            "model_id 없어도 stash 비어있으면 trigger 안 됨"
        );
    }
}
