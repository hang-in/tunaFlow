//! Codex `app-server` 지속 세션 모드
//!
//! `codex app-server --listen ws://127.0.0.1:PORT` 를 글로벌 프로세스로 스폰하고
//! WebSocket 클라이언트로 연결해 멀티턴 대화를 유지한다.
//!
//! 프로토콜: JSON-RPC 2.0 over WebSocket
//! 소스 참조: `_util/codex/codex-rs/app-server-protocol/src/`
//!
//! # 구조
//! - **글로벌 AppServer**: 하나의 codex 프로세스 + 하나의 WS 연결
//! - **스레드 레지스트리**: conv_id → thread_id 매핑 (conversation당 하나의 codex thread)
//! - **RPC 호출**: ID 기반 요청/응답 매칭 (broadcast 채널 필터링)
//! - **스트리밍**: `agentMessageDelta` 알림으로 실시간 텍스트 스트리밍

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use futures_util::{SinkExt, StreamExt};
use parking_lot::Mutex as PlMutex;
use serde_json::{Value, json};
use tokio::sync::{broadcast, mpsc};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

use crate::agents::claude::{RunInput, RunOutput, resolve_cwd};
use crate::agents::resolve::{NpmCliConfig, resolve_npm_cli};
use crate::errors::AppError;

// ─────────────────────────── Types ────────────────────────────────────────────

struct AppServer {
    /// codex app-server가 리스닝하는 포트 (디버깅용)
    #[allow(dead_code)]
    port: u16,
    /// tunaFlow → codex: JSON 문자열 전송
    ws_tx: mpsc::UnboundedSender<String>,
    /// codex → tunaFlow: 모든 서버 메시지 브로드캐스트
    from_server_tx: broadcast::Sender<String>,
    /// 다음 JSON-RPC 요청 ID (자동 증가)
    next_id: AtomicU64,
    /// codex 프로세스 모니터 태스크 핸들 (Drop 시 abort → 프로세스 종료)
    _process_abort: tokio::task::AbortHandle,
}

struct ConvThread {
    thread_id: String,
    model: String,
}

// ─────────────────────────── Global State ─────────────────────────────────────

lazy_static::lazy_static! {
    /// 글로벌 codex app-server (하나의 프로세스, 여러 conversation 공유)
    static ref SERVER: PlMutex<Option<Arc<AppServer>>> = PlMutex::new(None);
    /// conv_id → ConvThread (thread_id, model)
    static ref CONV_THREADS: PlMutex<HashMap<String, ConvThread>> = PlMutex::new(HashMap::new());
    /// 서버 시작 중복 방지 뮤텍스
    static ref SERVER_INIT: tokio::sync::Mutex<()> = tokio::sync::Mutex::new(());
}

// ─────────────────────────── Public API ───────────────────────────────────────

/// codex 바이너리가 설치되어 있으면 app-server 모드를 사용할 수 있다.
pub fn is_available() -> bool {
    let resolved = resolve_npm_cli(&NpmCliConfig {
        bin_name: "codex",
        npm_package: "@openai/codex",
        npm_entry: "bin/codex.js",
    });
    // 절대 경로면 파일 존재 여부 확인, 아니면 PATH에서 탐색
    let cmd = &resolved.command;
    if std::path::Path::new(cmd).is_absolute() {
        std::path::Path::new(cmd).exists()
    } else {
        // PATH에서 바이너리 탐색
        std::env::var("PATH")
            .unwrap_or_default()
            .split(':')
            .any(|dir| std::path::Path::new(dir).join(cmd).exists())
    }
}

/// ContextPack freshness 판정용 — 현재 활성 thread의 식별 키.
/// 같은 키 = 같은 codex thread (replay 히스토리 보유). 없으면 None (=> full ContextPack 필요).
pub fn current_thread_key(conv_id: &str) -> Option<String> {
    CONV_THREADS.lock().get(conv_id).map(|t| format!("codex-app:{}", t.thread_id))
}

/// conversation이 제거될 때 해당 thread를 레지스트리에서 제거한다.
/// 서버 자체는 계속 실행된다.
#[allow(dead_code)]
pub fn kill_thread(conv_id: &str) {
    CONV_THREADS.lock().remove(conv_id);
}

/// app-server 세션을 통해 메시지를 전송하고 스트리밍 응답을 수집한다.
///
/// Claude의 `claude_sdk_session::stream_run_sdk`와 동일한 인터페이스.
pub async fn stream_run_app_server<F, G, C>(
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
    // 빈 문자열 model 가드 — Some("")이 들어오면 unwrap_or가 발동하지 않아
    // codex API에 빈 model 필드가 전달되어 RPC 에러가 발생한다 (claude와 동일 패턴).
    //
    // default = "gpt-5-codex": ChatGPT account/Codex 사용자 모두 호환되는 안전한 default.
    // 이전 default였던 "o4-mini"는 OpenAI Platform API 전용 — ChatGPT account에선
    // 400 "model is not supported" 에러 발생 (실측 확인됨).
    let model = input.model
        .as_deref()
        .filter(|m| !m.is_empty())
        .unwrap_or("gpt-5-codex")
        .to_string();

    let server = get_or_start_server().await?;
    let thread_id =
        get_or_create_thread(&server, conv_id, &model, input.project_path.as_deref()).await?;

    // turn/start 전에 구독 → 초기 이벤트 누락 방지
    let mut event_rx = server.from_server_tx.subscribe();

    // turn/start RPC 호출 → turn_id 획득
    let turn_result = rpc_call(&server, "turn/start", json!({
        "threadId": thread_id,
        "input": [{ "type": "text", "text": input.prompt }]
    }))
    .await?;

    let turn_id = turn_result
        .get("turn")
        .and_then(|t| t.get("id"))
        .and_then(|id| id.as_str())
        .ok_or_else(|| AppError::Agent("codex app-server: turn/start returned no turn.id".into()))?
        .to_string();

    on_progress("Agent thinking...".into());

    let mut accumulated_text = String::new();
    let input_tokens: i64 = 0;
    let output_tokens: i64 = 0;
    let total_cost: f64 = 0.0;

    // 스트리밍 이벤트 수신 루프
    loop {
        let line_result = tokio::select! {
            r = tokio::time::timeout(
                std::time::Duration::from_secs(600),
                event_rx.recv()
            ) => r,
            _ = async {
                loop {
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    if is_cancelled() { break; }
                }
            } => {
                // 취소 요청 → turn/interrupt 전송
                let id = server.next_id.fetch_add(1, Ordering::Relaxed).to_string();
                let _ = server.ws_tx.send(json!({
                    "id": id,
                    "method": "turn/interrupt",
                    "params": { "threadId": thread_id }
                }).to_string());
                return Err(AppError::Agent("cancelled by user".into()));
            }
        };

        let line = match line_result {
            Ok(Ok(line)) => line,
            Ok(Err(broadcast::error::RecvError::Lagged(n))) => {
                eprintln!("[codex-app-server] receiver lagged by {} messages", n);
                continue;
            }
            Ok(Err(broadcast::error::RecvError::Closed)) => {
                return Err(AppError::Agent("codex app-server: event channel closed".into()));
            }
            Err(_) => {
                return Err(AppError::Agent(
                    "codex app-server: timeout waiting for response (10min)".into(),
                ));
            }
        };

        let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };

        // RPC 응답(id 있음)은 건너뜀 — rpc_call 내부에서 처리
        if v.get("id").is_some() {
            continue;
        }

        let method = v.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let params = v.get("params").unwrap_or(&Value::Null);

        // 현재 thread_id 필터링
        let msg_thread_id = params.get("threadId").and_then(|t| t.as_str()).unwrap_or("");
        if !thread_id.is_empty() && msg_thread_id != thread_id {
            continue;
        }

        match method {
            "item/agentMessage/delta" => {
                let delta = params.get("delta").and_then(|d| d.as_str()).unwrap_or("");
                if !delta.is_empty() {
                    accumulated_text.push_str(delta);
                    on_chunk(accumulated_text.clone());
                }
            }
            "item/started" => {
                if let Some(item) = params.get("item") {
                    let item_type = item.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    match item_type {
                        "command_execution" => {
                            let cmd = item
                                .get("command")
                                .and_then(|v| v.as_str())
                                .unwrap_or("bash");
                            let truncated = &cmd[..cmd.len().min(120)];
                            let step = json!({
                                "type": "command",
                                "name": "Bash",
                                "input": truncated,
                                "status": "running"
                            });
                            on_progress(format!("__STEP__:{}", step));
                        }
                        "file_change" => {
                            let file = item
                                .get("file")
                                .and_then(|v| v.as_str())
                                .unwrap_or("file");
                            let step = json!({
                                "type": "file_change",
                                "name": "Edit",
                                "input": file,
                                "status": "running"
                            });
                            on_progress(format!("__STEP__:{}", step));
                        }
                        "reasoning" => {
                            let step = json!({
                                "type": "thinking",
                                "name": "Reasoning",
                                "input": "",
                                "status": "running"
                            });
                            on_progress(format!("__STEP__:{}", step));
                        }
                        _ => {}
                    }
                }
            }
            "item/completed" => {
                if let Some(item) = params.get("item") {
                    let item_type = item.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    match item_type {
                        "command_execution" => {
                            let cmd = item
                                .get("command")
                                .and_then(|v| v.as_str())
                                .unwrap_or("bash");
                            let truncated = &cmd[..cmd.len().min(120)];
                            let status = item
                                .get("status")
                                .and_then(|v| v.as_str())
                                .unwrap_or("done");
                            let step = json!({
                                "type": "command",
                                "name": "Bash",
                                "input": truncated,
                                "status": if status == "failed" { "error" } else { "done" }
                            });
                            on_progress(format!("__STEP__:{}", step));
                        }
                        "file_change" => {
                            let file = item
                                .get("file")
                                .and_then(|v| v.as_str())
                                .unwrap_or("file");
                            let step = json!({
                                "type": "file_change",
                                "name": "Edit",
                                "input": file,
                                "status": "done"
                            });
                            on_progress(format!("__STEP__:{}", step));
                        }
                        _ => {}
                    }
                }
            }
            "turn/completed" => {
                let turn = params.get("turn").unwrap_or(&Value::Null);
                let completed_turn_id = turn
                    .get("id")
                    .and_then(|id| id.as_str())
                    .unwrap_or("");
                // 다른 turn의 완료 이벤트는 건너뜀
                if completed_turn_id != turn_id {
                    continue;
                }

                let content = accumulated_text.trim().to_string();
                return Ok(RunOutput {
                    content,
                    cost_usd: total_cost,
                    input_tokens,
                    output_tokens,
                    session_id: Some(thread_id),
                });
            }
            // wire format: "error" (NOT "errorNotification"; codex v2 protocol common.rs:978)
            // payload: { error: { message, codexErrorInfo, additionalDetails }, willRetry, threadId, turnId }
            "error" => {
                let error_msg = params
                    .get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(|m| m.as_str())
                    .unwrap_or("unknown error");
                let will_retry = params.get("willRetry").and_then(|v| v.as_bool()).unwrap_or(false);
                eprintln!("[codex-app-server] error: {} (willRetry={})", error_msg, will_retry);
                // willRetry=false이면 turn이 interrupt되어 turn/completed가 오지 않으므로
                // 즉시 Err로 끊어 finalize에서 ok=false 표시. willRetry=true면 turn 계속.
                if !will_retry {
                    return Err(AppError::Agent(format!(
                        "codex app-server error: {}",
                        error_msg
                    )));
                }
            }
            _ => {
                // threadStarted/turnStarted/planDelta/hookStarted/command-exec-outputDelta/
                // reasoning-textDelta 등 — UI에 표시할 step은 아니지만 watchdog 유지를 위한
                // heartbeat은 보낸다. __HEARTBEAT__ 접두는 frontend에서 timer reset만 하고
                // tool-step 파싱은 스킵한다.
                if !method.is_empty() {
                    on_progress(format!("__HEARTBEAT__:{}", method));
                    eprintln!("[codex-app-server] notification: {}", method);
                }
            }
        }
    }
}

// ─────────────────────────── Server Management ────────────────────────────────

async fn get_or_start_server() -> Result<Arc<AppServer>, AppError> {
    // 빠른 경로: 서버가 이미 실행 중
    if let Some(s) = SERVER.lock().clone() {
        return Ok(s);
    }

    // 중복 시작 방지 락 획득
    let _guard = SERVER_INIT.lock().await;

    // 락 획득 후 재확인 (double-checked locking)
    if let Some(s) = SERVER.lock().clone() {
        return Ok(s);
    }

    let server = start_server().await?;
    *SERVER.lock() = Some(Arc::clone(&server));
    Ok(server)
}

async fn start_server() -> Result<Arc<AppServer>, AppError> {
    // 여유 포트 탐색 (잠시 바인딩 후 해제 — codex가 해당 포트를 사용)
    let port = find_free_port().await?;

    let resolved = resolve_npm_cli(&NpmCliConfig {
        bin_name: "codex",
        npm_package: "@openai/codex",
        npm_entry: "bin/codex.js",
    });

    let ws_url = format!("ws://127.0.0.1:{}", port);

    let mut cmd = tokio::process::Command::new(&resolved.command);
    if let Some(ref script) = resolved.script_arg {
        cmd.arg(script);
    }
    cmd.arg("app-server")
        .arg("--listen")
        .arg(&ws_url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true);

    let mut child = cmd.spawn().map_err(|e| {
        AppError::Agent(format!(
            "codex app-server: spawn failed ({}): {}",
            resolved.command, e
        ))
    })?;

    // codex가 바인딩할 때까지 재시도 (최대 10초)
    let ws_connect_url = format!("ws://127.0.0.1:{}", port);
    let ws_stream = retry_connect(&ws_connect_url, 20, 500).await.map_err(|e| {
        AppError::Agent(format!("codex app-server: WS connect failed: {}", e))
    })?;

    let (mut ws_sink, mut ws_source) = ws_stream.split();
    let (ws_tx, mut ws_rx) = mpsc::unbounded_channel::<String>();
    let (from_server_tx, _) = broadcast::channel::<String>(512);
    let from_server_tx_clone = from_server_tx.clone();

    // WS 전송 태스크: ws_rx → ws_sink
    tokio::spawn(async move {
        while let Some(msg) = ws_rx.recv().await {
            if ws_sink.send(Message::Text(msg.into())).await.is_err() {
                eprintln!("[codex-app-server] WS send error, sender loop exiting");
                break;
            }
        }
    });

    // WS 수신 태스크: ws_source → from_server_tx
    tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_source.next().await {
            if let Message::Text(text) = msg {
                let _ = from_server_tx_clone.send(text.to_string());
            }
        }
        eprintln!("[codex-app-server] WS connection closed — clearing server");
        *SERVER.lock() = None;
    });

    // 프로세스 모니터 태스크 (종료 시 레지스트리 초기화)
    let process_task = tokio::spawn(async move {
        let _ = child.wait().await;
        eprintln!("[codex-app-server] process exited — clearing server & threads");
        *SERVER.lock() = None;
        CONV_THREADS.lock().clear();
    });

    let server = Arc::new(AppServer {
        port,
        ws_tx,
        from_server_tx,
        next_id: AtomicU64::new(1),
        _process_abort: process_task.abort_handle(),
    });

    // initialize RPC 호출
    rpc_call(
        &server,
        "initialize",
        json!({
            "clientInfo": { "name": "tunaflow", "version": "0.1.0" },
            "capabilities": { "experimentalApi": false }
        }),
    )
    .await?;

    eprintln!("[codex-app-server] started on port {}", port);
    Ok(server)
}

// ─────────────────────────── Thread Management ────────────────────────────────

async fn get_or_create_thread(
    server: &AppServer,
    conv_id: &str,
    model: &str,
    project_path: Option<&str>,
) -> Result<String, AppError> {
    // 기존 스레드 확인
    let existing = CONV_THREADS
        .lock()
        .get(conv_id)
        .map(|t| (t.thread_id.clone(), t.model.clone()));

    if let Some((thread_id, current_model)) = existing {
        if current_model == model {
            return Ok(thread_id);
        }
        // 모델 변경 — 기존 스레드 폐기, 새 스레드 시작
        eprintln!(
            "[codex-app-server] model changed ({} → {}), starting new thread for conv={}",
            current_model, model, conv_id
        );
        CONV_THREADS.lock().remove(conv_id);
    }

    let cwd = resolve_cwd(project_path);
    // sandbox=danger-full-access는 claude의 --permission-mode bypassPermissions와 등가.
    // 이게 없으면 codex는 default sandbox(workspace-write, network 차단)에서 동작하여
    // gh/curl 등 네트워크 도구가 모두 실패한다.
    // persistExtendedHistory/experimentalRawEvents는 experimentalApi capability가 있어야
    // 허용되므로 필드 자체를 생략.
    let result = rpc_call(
        server,
        "thread/start",
        json!({
            "model": model,
            "cwd": cwd.to_string_lossy(),
            "approvalPolicy": "never",
            "sandbox": "danger-full-access"
        }),
    )
    .await?;

    let thread_id = result
        .get("thread")
        .and_then(|t| t.get("id"))
        .and_then(|id| id.as_str())
        .ok_or_else(|| {
            AppError::Agent("codex app-server: thread/start returned no thread.id".into())
        })?
        .to_string();

    CONV_THREADS.lock().insert(
        conv_id.to_string(),
        ConvThread {
            thread_id: thread_id.clone(),
            model: model.to_string(),
        },
    );

    eprintln!(
        "[codex-app-server] created thread {} for conv={}",
        thread_id, conv_id
    );
    Ok(thread_id)
}

// ─────────────────────────── RPC Helper ───────────────────────────────────────

/// JSON-RPC 2.0 요청을 전송하고 매칭 응답을 기다린다.
///
/// 전송 전에 broadcast를 구독하므로 응답을 놓치지 않는다.
/// 알림(notifications)은 id가 없어서 자동으로 필터링된다.
async fn rpc_call(server: &AppServer, method: &str, params: Value) -> Result<Value, AppError> {
    let id = server.next_id.fetch_add(1, Ordering::Relaxed).to_string();
    let request = json!({ "id": id, "method": method, "params": params });

    // 전송 전 구독
    let mut rx = server.from_server_tx.subscribe();

    server
        .ws_tx
        .send(request.to_string())
        .map_err(|_| AppError::Agent("codex app-server: WS send channel closed".into()))?;

    let id_str = id.clone();
    let method_str = method.to_string();

    tokio::time::timeout(
        std::time::Duration::from_secs(30),
        async move {
            loop {
                match rx.recv().await {
                    Ok(line) => {
                        let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
                        // ID 매칭: 문자열 또는 정수 ID 모두 지원
                        let v_id = v.get("id").and_then(|i| match i {
                            Value::String(s) => Some(s.clone()),
                            Value::Number(n) => n.as_i64().map(|n| n.to_string()),
                            _ => None,
                        });
                        if v_id.as_deref() != Some(&id_str) {
                            continue;
                        }
                        if let Some(error) = v.get("error") {
                            return Err(AppError::Agent(format!(
                                "codex RPC error ({}): {}",
                                method_str, error
                            )));
                        }
                        return Ok(v.get("result").cloned().unwrap_or(Value::Null));
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        eprintln!("[codex-app-server] rpc_call lagged by {} messages", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        return Err(AppError::Agent(
                            "codex app-server: broadcast closed during RPC".into(),
                        ));
                    }
                }
            }
        },
    )
    .await
    .map_err(|_| {
        AppError::Agent(format!(
            "codex app-server: RPC timeout for method={}",
            method
        ))
    })?
}

// ─────────────────────────── Utilities ────────────────────────────────────────

/// 임시 바인딩으로 여유 포트를 찾는다.
async fn find_free_port() -> Result<u16, AppError> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| AppError::Agent(format!("codex app-server: find_free_port: {}", e)))?;
    let port = listener
        .local_addr()
        .map_err(|e| AppError::Agent(format!("codex app-server: local_addr: {}", e)))?
        .port();
    drop(listener); // codex가 사용할 포트를 해제
    Ok(port)
}

/// WS 연결을 재시도한다. codex 프로세스가 바인딩할 때까지 폴링.
async fn retry_connect(
    url: &str,
    max_attempts: u32,
    delay_ms: u64,
) -> Result<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    String,
> {
    let mut last_err = String::from("no attempts made");
    for attempt in 1..=max_attempts {
        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
        match connect_async(url).await {
            Ok((stream, _)) => {
                eprintln!(
                    "[codex-app-server] WS connected after {} attempt(s)",
                    attempt
                );
                return Ok(stream);
            }
            Err(e) => {
                last_err = e.to_string();
                eprintln!(
                    "[codex-app-server] WS connect attempt {}/{} failed: {}",
                    attempt, max_attempts, e
                );
            }
        }
    }
    Err(last_err)
}
