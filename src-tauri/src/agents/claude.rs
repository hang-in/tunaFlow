use std::io::{BufRead, BufReader, Read, Write as IoWrite};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use serde::Deserialize;
use crate::errors::AppError;
use crate::no_console::NoConsole;

/// Resolve working directory for CLI agent execution.
/// If a project path is provided, the agent runs inside that project (read+write).
/// Otherwise falls back to temp_dir (no project context).
pub fn resolve_cwd(project_path: Option<&str>) -> PathBuf {
    if let Some(p) = project_path {
        let path = PathBuf::from(p);
        if path.is_dir() {
            return path;
        }
    }
    std::env::temp_dir()
}

// ─── Streaming JSON types (--output-format stream-json) ───────────────────

/// One JSON line emitted by `claude --output-format stream-json`
#[derive(Deserialize)]
struct StreamLine {
    #[serde(rename = "type")]
    line_type: String,
    // assistant event
    message: Option<StreamAssistantMsg>,
    // result event
    result: Option<String>,
    is_error: Option<bool>,
    cost_usd: Option<f64>,
    total_cost_usd: Option<f64>,
    total_input_tokens: Option<i64>,
    total_output_tokens: Option<i64>,
    /// claude CLI 최신 스키마 — usage 가 nested. top-level total_*_tokens 는 일부
    /// 버전에서 부재하므로 `.or_else()` fallback 으로 양쪽 모두 지원.
    /// insightStabilityPlan Subtask 03 (INV-3).
    usage: Option<StreamUsage>,
    session_id: Option<String>,
    // rate_limit_event fields — claude CLI 2.1.x stream-json 일부 응답에 포함.
    // (claudeTransportFlipHardeningPlan T1) optional, 미존재 시 graceful 무시.
    status: Option<String>,
    resets_at: Option<String>,
    rate_limit_type: Option<String>,
    overage_status: Option<String>,
    overage_disabled_reason: Option<String>,
    is_using_overage: Option<bool>,
}

/// claude CLI 의 `rate_limit_event` line 에서 추출된 정보.
///
/// SSOT: `docs/plans/claudeTransportFlipHardeningPlan_2026-04-29.md` Task 01.
/// frontend RuntimeStatusBar 의 indicator + 사용자 가시 안내 (overage rejected,
/// reset 카운트다운) 의 데이터 소스. 미전송 버전 / line 미수신 시 None.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitInfo {
    /// "ok" / "approaching_limit" / "limit_reached" / etc — Anthropic 정의 그대로
    pub status: Option<String>,
    /// reset 시점 RFC3339 ISO 문자열
    pub resets_at: Option<String>,
    /// "5_hour" / "weekly" / etc
    pub rate_limit_type: Option<String>,
    /// "enabled" / "disabled" / "available"
    pub overage_status: Option<String>,
    /// "org_level_disabled" 등 disable 사유
    pub overage_disabled_reason: Option<String>,
    /// 현재 send 가 overage 사용 중인지
    pub is_using_overage: Option<bool>,
}

#[derive(Deserialize)]
struct StreamUsage {
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    #[allow(dead_code)]
    cache_creation_input_tokens: Option<i64>,
    #[allow(dead_code)]
    cache_read_input_tokens: Option<i64>,
}

#[derive(Deserialize)]
struct StreamAssistantMsg {
    content: Option<Vec<StreamContentBlock>>,
}

#[derive(Deserialize)]
struct StreamContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: Option<String>,
    thinking: Option<String>,
    // tool_use fields
    name: Option<String>,
    input: Option<serde_json::Value>,
}

/// Extract thinking content from assistant message (for progress display).
fn extract_thinking(msg: &StreamAssistantMsg) -> Option<String> {
    msg.content
        .as_ref()
        .and_then(|blocks| {
            blocks.iter()
                .filter(|b| b.block_type == "thinking")
                .filter_map(|b| b.thinking.as_deref())
                .next()
                .map(|s| s.to_string())
        })
}

fn extract_text(msg: &StreamAssistantMsg) -> String {
    msg.content
        .as_ref()
        .and_then(|blocks| {
            blocks
                .iter()
                .filter(|b| b.block_type == "text")
                .filter_map(|b| b.text.as_deref())
                .next()
        })
        .unwrap_or("")
        .to_string()
}

/// Shape of `claude -p --output-format json` stdout
#[derive(Debug, Deserialize)]
pub struct ClaudeJsonOutput {
    pub result: Option<String>,
    pub is_error: Option<bool>,
    pub cost_usd: Option<f64>,
    pub total_input_tokens: Option<i64>,
    pub total_output_tokens: Option<i64>,
    /// 최신 스키마 — usage nested. StreamLine 와 동일 fallback (INV-3).
    pub usage: Option<ClaudeUsage>,
    pub session_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ClaudeUsage {
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    #[allow(dead_code)]
    pub cache_creation_input_tokens: Option<i64>,
    #[allow(dead_code)]
    pub cache_read_input_tokens: Option<i64>,
}

pub struct RunInput {
    pub prompt: String,
    pub model: Option<String>,
    pub system_prompt: Option<String>,
    /// Session resume token from the previous CompletedEvent (session_id).
    /// None = new session. Some(token) = continue existing session via --resume.
    pub resume_token: Option<String>,
    /// Project directory — agent runs here (read+write own project).
    pub project_path: Option<String>,
    /// Absolute paths to image attachments. Codex 만 실제로 argv 로 전달
    /// (`-i <path>` 반복). Claude / Gemini 는 prompt 안에 경로 참조로 처리.
    pub image_paths: Vec<String>,
}

pub struct RunOutput {
    pub content: String,
    pub cost_usd: f64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub session_id: Option<String>,
    /// 가장 최근에 관측된 `rate_limit_event` payload. Anthropic 측에서 미전송이거나
    /// 구버전 CLI 면 None. claudeTransportFlipHardeningPlan T1.
    pub last_rate_limit: Option<RateLimitInfo>,
    /// `true` = stale resume_token detect → `--resume` 제거 후 1회 retry 로 fresh
    /// session 으로 응답을 받았다. 호출자 (start_claude_stream / finalize_engine_run)
    /// 가 이 flag 보고 (a) DB resume_token 갱신 (이미 새 session_id 가 들어옴), (b)
    /// `session_freshness::clear_delivered_key()` 호출 → 다음 send 부터
    /// `is_session_continuation=false` → full mode + anchor 2 turns ContextPack
    /// revival 자동 발동, (c) frontend 에 `claude:fresh_fallback` event emit.
    /// claudeTransportFlipHardeningPlan T2 + T3.
    pub fresh_fallback: bool,
}

/// claude API 에러를 사용자 친화 카테고리로 분류 (T7).
///
/// claudeTransportFlipHardeningPlan Task 07 — backend 의 raw "out of extra
/// usage" / "401 Unauthorized" 등을 frontend 가 용도별로 다르게 표시할 수
/// 있도록 4 종 + Unknown 으로 분류. UI 모달은 후속 PR (frontend) — 본 commit
/// 은 분류 함수 + serde label 만 제공.
///
/// 우선순위 (위에서 아래):
/// 1. StaleResumeToken — `looks_like_stale_resume_error` 도 true 인 경우. 이미
///    T2 가 자동 retry 처리하므로 본 분류는 retry 도 fail 시 escalate 라벨.
/// 2. AuthFailure — 401, invalid api key, authentication
/// 3. RateLimited — 429, rate_limit_exceeded, rate limit (Anthropic 측 한도)
/// 4. QuotaExceeded — usage limit / quota / weekly limit (Pro/Max plan 한도)
/// 5. ModelUnavailable — model not found, model deprecated
/// 6. Unknown — 위 매칭 X (raw fallback)
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiErrorKind {
    StaleResumeToken,
    AuthFailure,
    RateLimited,
    QuotaExceeded,
    ModelUnavailable,
    Unknown,
}

/// 에러 메시지 → ApiErrorKind 분류. claude.rs 안의 helper — 다른 엔진과 무관.
pub fn classify_claude_error(error_msg: &str) -> ApiErrorKind {
    let lower = error_msg.to_ascii_lowercase();

    if looks_like_stale_resume_error(error_msg) {
        return ApiErrorKind::StaleResumeToken;
    }
    if lower.contains("401")
        || lower.contains("invalid api key")
        || lower.contains("authentication")
    {
        return ApiErrorKind::AuthFailure;
    }
    if lower.contains("429")
        || lower.contains("rate_limit_exceeded")
        || lower.contains("rate limit")
    {
        return ApiErrorKind::RateLimited;
    }
    if lower.contains("quota")
        || lower.contains("usage limit")
        || lower.contains("weekly limit")
        || lower.contains("monthly limit")
    {
        return ApiErrorKind::QuotaExceeded;
    }
    if lower.contains("model not found")
        || lower.contains("model_not_found")
        || lower.contains("model deprecated")
        || lower.contains("model_deprecated")
    {
        return ApiErrorKind::ModelUnavailable;
    }
    ApiErrorKind::Unknown
}

/// claude CLI 의 result.is_error 메시지가 stale resume_token 을 의미하는지 판정.
///
/// claudeTransportFlipHardeningPlan T2 — `--resume <id>` 동반 send 가 다음 패턴
/// 으로 거부되면 stale resume_token 으로 보고 `--resume` 제거 후 retry 1회.
///
/// 정확한 keyword 조합으로 false positive 차단:
/// - 정상 인증 실패 (401, invalid api key) → match X
/// - 정상 한도 초과 (5h rolling) → match X (Anthropic 측 다른 status code/메시지)
/// - 일시적 네트워크 에러 → match X
///
/// 매칭 keyword (사용자 보고 + Anthropic 정의):
/// - "out of extra usage" — 사용자 보고 패턴 (resume_token 무효화 시점)
/// - "session not found" / "404" — Anthropic 측 session_id 만료
/// - "invalid_request_error" + "session" — invalid session id reject
///
/// **caller 책임**: 본 함수가 true 라도 retry 는 stream_run 내부에서 1회만.
/// 두 번째 stale → caller 가 raw error 그대로 사용자에게 가시화 (escalate).
fn looks_like_stale_resume_error(error_msg: &str) -> bool {
    let lower = error_msg.to_ascii_lowercase();
    lower.contains("out of extra usage")
        || (lower.contains("session not found") || (lower.contains("404") && lower.contains("session")))
        || (lower.contains("invalid_request_error") && lower.contains("session"))
}

/// Execute `claude -p` with `--output-format stream-json`.
///
/// Two callbacks:
/// - `on_progress`: called for thinking/tool events (progress log, not final answer)
/// - `on_chunk`: called when assistant text content arrives (final answer streaming)
///
/// Returns the final `RunOutput` when the `result` line arrives.
/// Caller must NOT hold the DbState lock while calling this function.
///
/// claudeTransportFlipHardeningPlan T2 — `--resume <id>` 가 stale 로 거부되면
/// 자동으로 `--resume` 없이 1회 retry. retry 성공 시 RunOutput.fresh_fallback=true
/// 로 표기. retry 도 fail 이면 raw error 그대로 반환 (다른 원인). 무한 loop 차단.
pub fn stream_run<F, G, C>(input: RunInput, mut on_progress: G, mut on_chunk: F, is_cancelled: C) -> Result<RunOutput, AppError>
where
    F: FnMut(String),
    G: FnMut(String),
    C: Fn() -> bool,
{
    // 1회차 시도 — 사용자의 resume_token 그대로 사용.
    let had_resume = input.resume_token.is_some();
    let first_input = RunInput {
        prompt: input.prompt.clone(),
        model: input.model.clone(),
        system_prompt: input.system_prompt.clone(),
        resume_token: input.resume_token.clone(),
        project_path: input.project_path.clone(),
        image_paths: input.image_paths.clone(),
    };
    let first_result = stream_run_once(first_input, &mut on_progress, &mut on_chunk, &is_cancelled);

    match first_result {
        Ok(out) => Ok(out),
        Err(AppError::Agent(msg)) if had_resume && looks_like_stale_resume_error(&msg) => {
            // Stale resume_token detect — `--resume` 제거 후 1회 retry.
            // false positive 차단: had_resume 가 true 인 경우만 (resume 동반 send).
            // 무한 loop 차단: stream_run_once 직접 호출 (재귀 없음).
            eprintln!(
                "[claude-stale-resume] detected stale resume_token, retrying without --resume (msg: {})",
                msg.chars().take(120).collect::<String>()
            );
            let retry_input = RunInput {
                prompt: input.prompt,
                model: input.model,
                system_prompt: input.system_prompt,
                // resume_token 제거 — fresh session 으로 시작.
                resume_token: None,
                project_path: input.project_path,
                image_paths: input.image_paths,
            };
            match stream_run_once(retry_input, &mut on_progress, &mut on_chunk, &is_cancelled) {
                Ok(mut out) => {
                    out.fresh_fallback = true;
                    Ok(out)
                }
                Err(e) => Err(e),
            }
        }
        Err(e) => Err(e),
    }
}

/// `stream_run` 의 inner 구현 — `--resume` 가 있는 그대로 1회 시도.
/// retry 는 stream_run wrapper 가 담당 (T2 stale detect path).
fn stream_run_once<F, G, C>(
    input: RunInput,
    on_progress: &mut G,
    on_chunk: &mut F,
    is_cancelled: &C,
) -> Result<RunOutput, AppError>
where
    F: FnMut(String),
    G: FnMut(String),
    C: Fn() -> bool,
{
    let mut cmd = Command::new("claude");
    cmd.no_console();
    cmd.arg("-p")
        .arg(&input.prompt)
        .arg("--output-format")
        .arg("stream-json")
        .arg("--verbose")
        .arg("--dangerously-skip-permissions")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .current_dir(resolve_cwd(input.project_path.as_deref()));

    if let Some(model) = &input.model {
        cmd.arg("--model").arg(model);
    }

    // Write system prompt to temp file to avoid Windows 32K command line limit.
    // --append-system-prompt-file reads from file instead of inline arg.
    let _prompt_file = if let Some(system_prompt) = &input.system_prompt {
        let mut tmp = tempfile::NamedTempFile::new()
            .map_err(|e| AppError::Agent(format!("Failed to create temp file: {}", e)))?;
        tmp.write_all(system_prompt.as_bytes())
            .map_err(|e| AppError::Agent(format!("Failed to write system prompt: {}", e)))?;
        tmp.flush()
            .map_err(|e| AppError::Agent(format!("Failed to flush system prompt: {}", e)))?;
        cmd.arg("--append-system-prompt-file").arg(tmp.path());
        Some(tmp) // keep alive until child exits
    } else {
        None
    };

    if let Some(token) = &input.resume_token {
        cmd.arg("--resume").arg(token);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| AppError::Agent(format!("Failed to spawn claude: {}", e)))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::Agent("Failed to capture stdout".into()))?;

    // Drain stderr in a background thread to prevent pipe-buffer deadlock
    let mut stderr_pipe = child
        .stderr
        .take()
        .ok_or_else(|| AppError::Agent("Failed to capture stderr".into()))?;
    let stderr_handle = thread::spawn(move || {
        let mut buf = String::new();
        let _ = stderr_pipe.read_to_string(&mut buf);
        buf
    });

    let reader = BufReader::new(stdout);
    let mut final_output: Option<RunOutput> = None;
    let mut unparsed_lines: Vec<String> = Vec::new();
    // 마지막으로 관측된 rate_limit_event — result event 시 RunOutput.last_rate_limit
    // 으로 전달. claudeTransportFlipHardeningPlan T1.
    let mut last_rate_limit: Option<RateLimitInfo> = None;

    // Idle timeout: kill process if no output for 10 minutes.
    // insightStabilityPlan Subtask 04 (INV-4): watchdog 가 `timed_out` 플래그를 set
    // 하면 reader 루프 exit 후 distinct 에러 반환 → 상위 (run_insight_analysis) 가
    // insight_sessions.status = 'failed' 로 전이할 때 원인 구분 가능.
    //
    // 2026-04-29: trailing kill 차단 — reader 가 정상 종료한 뒤에도 watchdog 의
    // 30s sleep 이 누적되어 이미 reap 된 PID 에 `kill -9` 가 송출되던 race 를
    // 차단. `watchdog_done` AtomicBool + RAII `WatchdogGuard` 로 함수 scope 가
    // 끝날 때 (정상/에러/cancel/panic 모두) flag 가 set 되어 watchdog loop 가
    // 다음 깨어남 즉시 종료. 기존 `timed_out` 플래그 / reader 루프 / 정상 종료
    // 분기는 그대로 유지.
    let idle_timeout = std::time::Duration::from_secs(600);
    let last_activity = std::sync::Arc::new(parking_lot::Mutex::new(std::time::Instant::now()));
    let timed_out = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let watchdog_done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let child_id = child.id();
    {
        let last_act = std::sync::Arc::clone(&last_activity);
        let timed_out_flag = std::sync::Arc::clone(&timed_out);
        let done_flag = std::sync::Arc::clone(&watchdog_done);
        thread::spawn(move || {
            loop {
                thread::sleep(std::time::Duration::from_secs(30));
                if done_flag.load(std::sync::atomic::Ordering::SeqCst) {
                    break;
                }
                let elapsed = last_act.lock().elapsed();
                if elapsed > idle_timeout {
                    eprintln!(
                        "[agent-timeout] No output for {}s, killing pid {}",
                        elapsed.as_secs(),
                        child_id
                    );
                    timed_out_flag.store(true, std::sync::atomic::Ordering::SeqCst);
                    // Best-effort kill via OS-native command. `kill -9` is Unix-only
                    // (`kill` doesn't exist on Windows); use `taskkill /F /PID <pid>`
                    // there. Without this branch, on Windows the watchdog could
                    // never reap a hung child claude.exe — the RAII guard still
                    // breaks the watchdog loop, but the subprocess would leak.
                    #[cfg(unix)]
                    let _ = std::process::Command::new("kill")
                        .no_console()
                        .arg("-9")
                        .arg(child_id.to_string())
                        .output();
                    #[cfg(windows)]
                    let _ = std::process::Command::new("taskkill")
                        .no_console()
                        .arg("/F")
                        .arg("/PID")
                        .arg(child_id.to_string())
                        .output();
                    break;
                }
            }
        });
    }

    // RAII guard: 함수 scope 가 끝날 때 watchdog 에 종료 신호 전달.
    // Drop 은 panic 시에도 호출되므로 모든 종료 경로 (정상 / 에러 / cancel /
    // unwind) 에서 trailing kill 을 차단한다.
    struct WatchdogGuard(std::sync::Arc<std::sync::atomic::AtomicBool>);
    impl Drop for WatchdogGuard {
        fn drop(&mut self) {
            self.0.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }
    let _watchdog_guard = WatchdogGuard(std::sync::Arc::clone(&watchdog_done));

    for raw in reader.lines() {
        // Reset idle timer on each line
        *last_activity.lock() = std::time::Instant::now();

        // Check cancel between each line of streaming output
        if is_cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(AppError::Agent("cancelled by user".into()));
        }
        let line = raw?;
        if line.trim().is_empty() {
            continue;
        }
        let parsed: StreamLine = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => {
                // Collect unparseable stdout lines (may contain plain-text errors)
                unparsed_lines.push(line);
                continue;
            }
        };

        match parsed.line_type.as_str() {
            "system" => {
                on_progress("Agent initializing...".into());
            }
            // claude CLI 가 stream-json 안에 rate_limit_event line 을 별도 emit 하는
            // 케이스. T1: 마지막 관측치를 RunOutput.last_rate_limit 으로 전달.
            // 구버전 CLI 가 미전송이면 last_rate_limit 은 None 으로 유지된다.
            "rate_limit_event" => {
                last_rate_limit = Some(RateLimitInfo {
                    status: parsed.status.clone(),
                    resets_at: parsed.resets_at.clone(),
                    rate_limit_type: parsed.rate_limit_type.clone(),
                    overage_status: parsed.overage_status.clone(),
                    overage_disabled_reason: parsed.overage_disabled_reason.clone(),
                    is_using_overage: parsed.is_using_overage,
                });
            }
            "assistant" => {
                if let Some(msg) = &parsed.message {
                    // Thinking → structured step
                    if let Some(thinking) = extract_thinking(msg) {
                        let last_line = thinking.lines().filter(|l| !l.trim().is_empty()).last().unwrap_or("").trim();
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
                    // Tool use → structured step
                    if let Some(blocks) = &msg.content {
                        for block in blocks.iter().filter(|b| b.block_type == "tool_use") {
                            if let Some(name) = &block.name {
                                let input_summary = block.input.as_ref().map(|v| {
                                    let s = v.to_string();
                                    if s.len() > 120 {
                                        let mut end = 120;
                                        while end > 0 && !s.is_char_boundary(end) { end -= 1; }
                                        format!("{}…", &s[..end])
                                    } else { s }
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
                    }
                    // Text → final answer chunk
                    let text = extract_text(msg);
                    if !text.is_empty() {
                        on_chunk(text);
                    }
                }
            }
            "result" => {
                if parsed.is_error.unwrap_or(false) {
                    let _ = child.wait();
                    return Err(AppError::Agent(format!(
                        "claude reported error: {}",
                        parsed.result.as_deref().unwrap_or("unknown")
                    )));
                }
                final_output = Some(RunOutput {
                    content: parsed.result.unwrap_or_default(),
                    cost_usd: parsed.total_cost_usd.or(parsed.cost_usd).unwrap_or(0.0),
                    // INV-3: top-level total_*_tokens 우선, 최신 스키마의 nested
                    // usage.*_tokens 로 fallback. 양쪽 None 이면 0.
                    input_tokens: parsed
                        .total_input_tokens
                        .or_else(|| parsed.usage.as_ref().and_then(|u| u.input_tokens))
                        .unwrap_or(0),
                    output_tokens: parsed
                        .total_output_tokens
                        .or_else(|| parsed.usage.as_ref().and_then(|u| u.output_tokens))
                        .unwrap_or(0),
                    session_id: parsed.session_id,
                    last_rate_limit: last_rate_limit.take(),
                    // T2: stream_run wrapper 가 retry 후 true 로 set. 1회 시도 자체는 false.
                    fresh_fallback: false,
                });
            }
            _ => {}
        }
    }

    child.wait()?;
    let stderr_content = stderr_handle.join().unwrap_or_default();

    // Watchdog 가 kill 했으면 원인 구분된 에러로 반환 (INV-4).
    if timed_out.load(std::sync::atomic::Ordering::SeqCst) {
        return Err(AppError::Agent(format!(
            "agent timeout after {}s: claude subprocess killed by watchdog (no output within idle window)",
            idle_timeout.as_secs()
        )));
    }

    final_output.ok_or_else(|| {
        // Build a diagnostic message using stderr, then unparsed stdout lines as fallback
        let detail = if !stderr_content.trim().is_empty() {
            stderr_content.trim().to_string()
        } else if !unparsed_lines.is_empty() {
            unparsed_lines.join(" | ")
        } else {
            "no output received".to_string()
        };
        AppError::Agent(format!("claude stream failed: {}", detail))
    })
}

/// Execute `claude -p` as a one-shot subprocess and return the result.
///
/// Caller must NOT hold the DbState lock while calling this function,
/// since the subprocess can take an arbitrarily long time.
pub fn run(input: RunInput) -> Result<RunOutput, AppError> {
    let mut cmd = Command::new("claude");
    cmd.no_console();
    cmd.arg("-p")
        .arg(&input.prompt)
        .arg("--output-format")
        .arg("json")
        .arg("--dangerously-skip-permissions")
        .current_dir(resolve_cwd(input.project_path.as_deref()));

    if let Some(model) = &input.model {
        cmd.arg("--model").arg(model);
    }

    let _prompt_file = if let Some(system_prompt) = &input.system_prompt {
        let mut tmp = tempfile::NamedTempFile::new()
            .map_err(|e| AppError::Agent(format!("Failed to create temp file: {}", e)))?;
        tmp.write_all(system_prompt.as_bytes())
            .map_err(|e| AppError::Agent(format!("Failed to write system prompt: {}", e)))?;
        tmp.flush()
            .map_err(|e| AppError::Agent(format!("Failed to flush system prompt: {}", e)))?;
        cmd.arg("--append-system-prompt-file").arg(tmp.path());
        Some(tmp)
    } else {
        None
    };

    if let Some(token) = &input.resume_token {
        cmd.arg("--resume").arg(token);
    }

    let output = cmd.output().map_err(|e| {
        AppError::Agent(format!("Failed to spawn claude: {}", e))
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::Agent(format!(
            "claude exited {:?}: {}",
            output.status.code(),
            stderr.trim()
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: ClaudeJsonOutput = serde_json::from_str(stdout.trim()).map_err(|e| {
        AppError::Agent(format!(
            "Failed to parse claude output: {} | raw: {}",
            e,
            stdout.trim()
        ))
    })?;

    if parsed.is_error.unwrap_or(false) {
        return Err(AppError::Agent(format!(
            "claude reported error: {}",
            parsed.result.as_deref().unwrap_or("unknown")
        )));
    }

    Ok(RunOutput {
        content: parsed.result.unwrap_or_default(),
        cost_usd: parsed.cost_usd.unwrap_or(0.0),
        // INV-3: nested usage.*_tokens fallback (최신 schema 지원)
        input_tokens: parsed
            .total_input_tokens
            .or_else(|| parsed.usage.as_ref().and_then(|u| u.input_tokens))
            .unwrap_or(0),
        output_tokens: parsed
            .total_output_tokens
            .or_else(|| parsed.usage.as_ref().and_then(|u| u.output_tokens))
            .unwrap_or(0),
        session_id: parsed.session_id,
        // one-shot json 모드는 rate_limit_event line 을 별도 stream 으로 받지
        // 못한다 (stream-json 전용). 항상 None.
        last_rate_limit: None,
        fresh_fallback: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// insightStabilityPlan Subtask 03 — INV-3 검증.
    /// old schema (top-level total_*_tokens) 와 new schema (nested usage.*_tokens)
    /// 양쪽 모두 parse 하여 non-zero tokens 반환.
    #[test]
    fn claude_json_output_parses_top_level_tokens_old_schema() {
        let json = r#"{
            "type": "result",
            "result": "hi",
            "is_error": false,
            "cost_usd": 0.01,
            "total_input_tokens": 100,
            "total_output_tokens": 50,
            "session_id": "s1"
        }"#;
        let parsed: ClaudeJsonOutput = serde_json::from_str(json).unwrap();
        let input = parsed
            .total_input_tokens
            .or_else(|| parsed.usage.as_ref().and_then(|u| u.input_tokens))
            .unwrap_or(0);
        let output = parsed
            .total_output_tokens
            .or_else(|| parsed.usage.as_ref().and_then(|u| u.output_tokens))
            .unwrap_or(0);
        assert_eq!(input, 100);
        assert_eq!(output, 50);
    }

    #[test]
    fn claude_json_output_parses_nested_usage_new_schema() {
        // 실제 claude CLI 2.1.x `result` 이벤트 구조 재현
        let json = r#"{
            "type": "result",
            "result": "hi",
            "is_error": false,
            "total_cost_usd": 0.132644,
            "usage": {
                "input_tokens": 6,
                "output_tokens": 12,
                "cache_creation_input_tokens": 19878,
                "cache_read_input_tokens": 16153
            },
            "session_id": "s1"
        }"#;
        let parsed: ClaudeJsonOutput = serde_json::from_str(json).unwrap();
        // top-level 없음
        assert!(parsed.total_input_tokens.is_none());
        assert!(parsed.total_output_tokens.is_none());
        // fallback 체인이 usage 에서 찾음
        let input = parsed
            .total_input_tokens
            .or_else(|| parsed.usage.as_ref().and_then(|u| u.input_tokens))
            .unwrap_or(0);
        let output = parsed
            .total_output_tokens
            .or_else(|| parsed.usage.as_ref().and_then(|u| u.output_tokens))
            .unwrap_or(0);
        assert_eq!(input, 6);
        assert_eq!(output, 12);
    }

    #[test]
    fn stream_line_parses_nested_usage_new_schema() {
        let json = r#"{
            "type": "result",
            "result": "hi",
            "is_error": false,
            "total_cost_usd": 0.13,
            "usage": {
                "input_tokens": 6,
                "output_tokens": 12,
                "cache_creation_input_tokens": 100,
                "cache_read_input_tokens": 50
            },
            "session_id": "s1"
        }"#;
        let parsed: StreamLine = serde_json::from_str(json).unwrap();
        let input = parsed
            .total_input_tokens
            .or_else(|| parsed.usage.as_ref().and_then(|u| u.input_tokens))
            .unwrap_or(0);
        let output = parsed
            .total_output_tokens
            .or_else(|| parsed.usage.as_ref().and_then(|u| u.output_tokens))
            .unwrap_or(0);
        assert_eq!(input, 6);
        assert_eq!(output, 12);
    }

    /// claudeTransportFlipHardeningPlan T1 — `rate_limit_event` line 의 fields 가
    /// `StreamLine` 으로 정상 deserialize 되는지 확인. CLI 가 아직 미전송이면
    /// 본 코드 경로를 안 타지만, deserialize 는 unknown field 무시 정책이라
    /// graceful.
    #[test]
    fn stream_line_parses_rate_limit_event() {
        let json = r#"{
            "type": "rate_limit_event",
            "status": "approaching_limit",
            "resets_at": "2026-04-29T12:00:00Z",
            "rate_limit_type": "5_hour",
            "overage_status": "available",
            "overage_disabled_reason": null,
            "is_using_overage": false
        }"#;
        let parsed: StreamLine = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.line_type, "rate_limit_event");
        assert_eq!(parsed.status.as_deref(), Some("approaching_limit"));
        assert_eq!(parsed.resets_at.as_deref(), Some("2026-04-29T12:00:00Z"));
        assert_eq!(parsed.rate_limit_type.as_deref(), Some("5_hour"));
        assert_eq!(parsed.overage_status.as_deref(), Some("available"));
        assert!(parsed.overage_disabled_reason.is_none());
        assert_eq!(parsed.is_using_overage, Some(false));
    }

    #[test]
    fn stream_line_rate_limit_with_overage_disabled() {
        let json = r#"{
            "type": "rate_limit_event",
            "status": "limit_reached",
            "resets_at": "2026-04-29T17:00:00Z",
            "rate_limit_type": "5_hour",
            "overage_status": "disabled",
            "overage_disabled_reason": "org_level_disabled",
            "is_using_overage": false
        }"#;
        let parsed: StreamLine = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.line_type, "rate_limit_event");
        assert_eq!(parsed.status.as_deref(), Some("limit_reached"));
        assert_eq!(parsed.overage_status.as_deref(), Some("disabled"));
        assert_eq!(parsed.overage_disabled_reason.as_deref(), Some("org_level_disabled"));
    }

    /// claudeTransportFlipHardeningPlan T2 — stale resume detect keyword.
    /// false positive 차단 검증 (정상 인증 실패 / 한도 초과 / 네트워크 에러는
    /// match X). retry trigger 정확성이 사용자 회복 핵심.
    #[test]
    fn looks_like_stale_resume_matches_user_reported_pattern() {
        // 사용자 보고 — "out of extra usage"
        assert!(looks_like_stale_resume_error("Anthropic API: out of extra usage"));
        // case insensitive
        assert!(looks_like_stale_resume_error("Out Of Extra Usage"));
    }

    #[test]
    fn looks_like_stale_resume_matches_session_404() {
        assert!(looks_like_stale_resume_error("404 session not found"));
        assert!(looks_like_stale_resume_error("Session not found"));
    }

    #[test]
    fn looks_like_stale_resume_matches_invalid_session_request() {
        assert!(looks_like_stale_resume_error("invalid_request_error: invalid session id"));
    }

    #[test]
    fn looks_like_stale_resume_does_not_match_auth_failure() {
        // 401 / invalid api key 는 retry 트리거하지 않음 (사용자가 재로그인 필요)
        assert!(!looks_like_stale_resume_error("401 Unauthorized"));
        assert!(!looks_like_stale_resume_error("invalid api key"));
        assert!(!looks_like_stale_resume_error("authentication failed"));
    }

    #[test]
    fn looks_like_stale_resume_does_not_match_rate_limit() {
        // 429 / true rate limit 은 retry 무의미 — Anthropic 측 한도 초과
        assert!(!looks_like_stale_resume_error("429 Too Many Requests"));
        assert!(!looks_like_stale_resume_error("rate_limit_exceeded"));
    }

    #[test]
    fn looks_like_stale_resume_does_not_match_network_error() {
        assert!(!looks_like_stale_resume_error("connection timed out"));
        assert!(!looks_like_stale_resume_error("dns resolution failed"));
    }

    /// claudeTransportFlipHardeningPlan T7 — error kind 분류 정확성.
    #[test]
    fn classify_claude_error_routes_kinds() {
        assert_eq!(
            classify_claude_error("Anthropic API: out of extra usage"),
            ApiErrorKind::StaleResumeToken
        );
        assert_eq!(classify_claude_error("401 Unauthorized"), ApiErrorKind::AuthFailure);
        assert_eq!(classify_claude_error("Invalid API key"), ApiErrorKind::AuthFailure);
        assert_eq!(classify_claude_error("429 Too Many Requests"), ApiErrorKind::RateLimited);
        assert_eq!(classify_claude_error("rate_limit_exceeded"), ApiErrorKind::RateLimited);
        assert_eq!(classify_claude_error("monthly quota exceeded"), ApiErrorKind::QuotaExceeded);
        assert_eq!(classify_claude_error("usage limit reached"), ApiErrorKind::QuotaExceeded);
        assert_eq!(classify_claude_error("Model not found: claude-x"), ApiErrorKind::ModelUnavailable);
        assert_eq!(classify_claude_error("model deprecated"), ApiErrorKind::ModelUnavailable);
        assert_eq!(classify_claude_error("connection timed out"), ApiErrorKind::Unknown);
        assert_eq!(classify_claude_error("dns resolution failed"), ApiErrorKind::Unknown);
    }

    #[test]
    fn classify_serializes_snake_case() {
        let kind = ApiErrorKind::StaleResumeToken;
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(json, "\"stale_resume_token\"");
        let kind = ApiErrorKind::QuotaExceeded;
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(json, "\"quota_exceeded\"");
    }

    #[test]
    fn rate_limit_info_serializes_camelcase() {
        // frontend (RuntimeStatusBar) 가 받는 직렬화 — camelCase 필드명 검증.
        let info = RateLimitInfo {
            status: Some("approaching_limit".into()),
            resets_at: Some("2026-04-29T12:00:00Z".into()),
            rate_limit_type: Some("5_hour".into()),
            overage_status: Some("available".into()),
            overage_disabled_reason: None,
            is_using_overage: Some(false),
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"resetsAt\""));
        assert!(json.contains("\"rateLimitType\""));
        assert!(json.contains("\"overageStatus\""));
        assert!(json.contains("\"isUsingOverage\""));
        assert!(json.contains("\"status\""));
    }

    #[test]
    fn both_schemas_absent_returns_zero() {
        let json = r#"{"type":"result","result":"hi","is_error":false,"cost_usd":0.01,"session_id":"s1"}"#;
        let parsed: ClaudeJsonOutput = serde_json::from_str(json).unwrap();
        let input = parsed
            .total_input_tokens
            .or_else(|| parsed.usage.as_ref().and_then(|u| u.input_tokens))
            .unwrap_or(0);
        assert_eq!(input, 0);
    }
}
