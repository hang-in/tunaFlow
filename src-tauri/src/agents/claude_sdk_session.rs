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

lazy_static::lazy_static! {
    static ref SESSIONS: SessionRegistry = Arc::new(PlMutex::new(HashMap::new()));
    static ref RESUME_IDS: ResumeRegistry = Arc::new(PlMutex::new(HashMap::new()));
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

    // 기존 세션 확인 (sync lock, await 없음)
    let existing = {
        let sessions = SESSIONS.lock();
        sessions.get(conv_id).map(|s| Arc::clone(s))
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
        eprintln!("[sdk-session] model changed via control_request: {} → {} for conv: {}",
            current_model, effective_model, conv_id);
        return Ok(session);
    }

    // 기존 세션 없음 — 새로 스폰. RESUME_IDS의 prior session_id로 --resume 가능.
    let resume_id = RESUME_IDS.lock().get(conv_id).cloned();
    let session = spawn_session(conv_id, project_path, effective_model, resume_id.as_deref()).await?;
    SESSIONS.lock().insert(conv_id.to_string(), Arc::clone(&session));
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
    SESSIONS.lock().remove(conv_id);
    if !keep_resume {
        RESUME_IDS.lock().remove(conv_id);
    }
    // ContextPack freshness: 세션이 죽으면 LAST_DELIVERED도 무효화 — 다음 send는 full로 강제
    crate::commands::agents_helpers::send_common::session_freshness::clear_delivered_key(conv_id);
    // SdkSession Drop → _shutdown_tx 전송 → axum 서버 종료 → _monitor_abort 취소
}

/// 해당 conversation에 활성 SDK 세션이 있는지 확인한다.
/// UI send-guard용 (WS 연결 전 send 시도 차단).
pub fn has_active_session(conv_id: &str) -> bool {
    SESSIONS.lock().contains_key(conv_id)
}

/// ContextPack freshness 판정용 — 현재 활성 세션의 식별 키.
/// 매 spawn마다 새로운 session_id가 생기므로, 같은 키 = 같은 에이전트 프로세스.
/// 세션이 없으면 None 반환 (=> ContextPack은 full로 보내야 함).
pub fn current_session_key(conv_id: &str) -> Option<String> {
    SESSIONS.lock().get(conv_id).map(|s| format!("claude-ws:{}", s.session_id))
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
    // 이미 세션이 있으면 스킵
    let exists = SESSIONS.lock().contains_key(conv_id);
    if exists { return; }

    match get_or_create_session(conv_id, project_path, model).await {
        Ok(_) => eprintln!("[sdk-session] prewarmed conv={}", conv_id),
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
    cmd.arg("--print")
        .arg("--sdk-url").arg(&sdk_url)
        .arg("--session-id").arg(&session_id)
        .arg("--model").arg(model)
        .arg("--input-format").arg("stream-json")
        .arg("--output-format").arg("stream-json")
        .arg("--replay-user-messages")
        .arg("--permission-mode").arg("bypassPermissions")
        .env("CLAUDE_CODE_ENVIRONMENT_KIND", "bridge")
        .env("CLAUDE_CODE_SESSION_ACCESS_TOKEN", &auth_token)
        // HybridTransport: 이벤트를 HTTP POST로 전송 (Desktop 앱 동일 패턴)
        .env("CLAUDE_CODE_POST_FOR_SESSION_INGRESS_V2", "1")
        .env_remove("CLAUDE_CODE_OAUTH_TOKEN")
        .current_dir(&cwd)
        // stdin/stdout piped — stdin: 메시지 전달, stdout: 이벤트 수신(HTTP POST 병행)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);

    // 이전 대화 이력이 있으면 --resume으로 이어받기
    if let Some(resume_id) = resume_session_id {
        cmd.arg("--resume").arg(resume_id);
        eprintln!("[sdk-session] resuming with session_id={} model={}", resume_id, model);
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
                if let Some(sid) = &parsed.session_id {
                    RESUME_IDS.lock().insert(conv_id_owned.clone(), sid.clone());
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

                return Ok(RunOutput {
                    content: parsed.result.unwrap_or_default(),
                    cost_usd: parsed.total_cost_usd.or(parsed.cost_usd).unwrap_or(0.0),
                    input_tokens: final_input,
                    output_tokens: final_output,
                    session_id: parsed.session_id,
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
    SESSIONS.lock().clear();
    RESUME_IDS.lock().clear();
}
