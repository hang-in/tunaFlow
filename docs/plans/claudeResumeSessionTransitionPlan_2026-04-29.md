---
title: Claude resume-session transition — sdk-session 모드 deprecate, `-p --session-id`/`--resume` 으로 전환
status: ready-to-implement
priority: P0 (claude 2.1.121 의 --sdk-url 정책으로 sdk-session 영구 차단 — 사용자 환경 broken)
created_at: 2026-04-29
related:
  - src-tauri/src/agents/claude_sdk_session.rs              # 현재 차단된 transport
  - src-tauri/src/agents/claude.rs                          # 기존 -p single-shot, 본 plan 의 base
  - src-tauri/src/commands/agents.rs                        # send 경로 dispatch
  - docs/plans/sdkUrlSessionModePlan.md                     # 직전 transport (PTY → sdk-url WS)
canonical: true
owners:
  - architect (본 plan 작성 + 사용자 직접 처리)
---

# 배경

## 정책 변경 (2026-04-28)

claude CLI 2.1.121 (4월 28일 자동 update) 가 `--sdk-url` flag 를 *Anthropic 공식 endpoint 만* 허용하도록 정책 변경:

```
Error: --sdk-url rejected: host "127.0.0.1" is not an approved Anthropic endpoint.
This flag is reserved for Remote Control worker processes connecting to Anthropic's backend.
```

**검증된 whitelist** (binary minified JS 분석):
```js
CD3 = new Set([
  "api.anthropic.com",
  "api-staging.anthropic.com",
  "beacon.claude-ai.staging.ant.dev",
  "claude.fedstart.com",
  "claude-staging.fedstart.com"
])  // hardcoded — config/env override 없음
```

= **tunaFlow 의 sdk-session 모드 (localhost WS) 영구 차단**.

## 우회 path 분석 (모두 부적합)

| Path | 결과 |
|---|---|
| `/etc/hosts` 매핑 + self-signed TLS | 기술적 가능 ✓ (manual 검증), production 부적합 — system-wide 침범 + cert 신뢰 강요 |
| binary patch (CD3 추가 / IDK 우회) | 매 update 재실행 + Anthropic ToS 회색 지대 + fragile |
| Anthropic 공식 RC worker 등록 | 가능성 낮음 (tunaFlow 인정 받기 어려움) |
| desktop app → claude-code 빈틈 | desktop app 이 *Anthropic cloud* 사용, child claude spawn 흔적 없음 |
| PTY 회귀 | 사용자 거부 (parsing 불안 — sdk-session 으로 옮긴 이유) |

## 채택 path — `-p --resume` (사용자 제안)

claude CLI 의 *공식 사용자 path*: `--session-id` 신규 + `--resume` 이어가기. **manual 검증 완료 (2026-04-29)**:

```
Step 1: claude -p --session-id <uuid> --output-format stream-json ... <<< "remember 42"
        → "OK"  (session=<uuid>, 2486ms)

Step 2: claude -p --resume <same-uuid> ... <<< "what number?"
        → "42"  (same session, 2491ms, cache hit 36942 tokens)
```

= **stateful multi-turn conversation 정상**. claude internal session store 가 history 보관.

# 평가

| axis | sdk-session (현재 broken) | resume-session (제안) |
|---|---|---|
| Anthropic 정책 | ❌ 차단 | ✓ 공식 path |
| claude self-update 영향 | ❌ 매 update fragile | ✓ version 무관 |
| Multi-turn 대화 | ✓ | ✓ (claude internal store) |
| Streaming 응답 | ✓ WS | ✓ `--output-format stream-json` |
| Tool use (file/bash) | ✓ | ✓ |
| 매 message overhead | 0 (persistent process) | ~2.5s spawn cost |
| `/command` (slash) | ✓ | ❌ 사용자 명시 *"안 쓰니까 OK"* |
| Cache hit | ✓ | ✓ (cache_read_input_tokens 36942 확인) |

= production-ready. spawn cost 외 기능 동등.

# Invariants

- **[INV-1]** 본 transition 은 macOS / Windows 양쪽 영향. Anthropic 정책이 OS 무관이라 모든 platform 에 동일 fix 필요.
- **[INV-2]** 기존 `claude_sdk_session.rs` 코드는 *deprecate* 하되 코드 자체는 유지 — env / setting toggle 로 실험 가능. 사용자가 정책 우회 path 발견 시 재사용 가능.
- **[INV-3]** `RESUME_IDS` 메모리 + DB `resume_token` mechanism 그대로 재사용. transition 은 *spawn 방식만* 변경.
- **[INV-4]** stream-json output 형식 그대로. tunaFlow 의 기존 parser (`claude.rs` 의 stream parsing) 재사용.
- **[INV-5]** `rate_limit_event` 같은 신규 stream chunk type 은 line filter 로 무시 (또는 status badge 로 표시). 기존 parser breaking change 0.
- **[INV-6]** *Engine 변경 / model 변경* 시는 새 session_id 발급 (현재 sdk-session 의 `restart_sdk_session` 와 동일 의미). DB resume_token clear path 그대로.
- **[INV-7]** Windows 환경에서도 동일 동작 — `claude.exe -p --session-id ...` Windows 빌드 검증 필요 (사용자 Windows 작업 시).

# 구현 단계

## Step 1: 신규 module — `src-tauri/src/agents/claude_resume_session.rs`

기존 `claude.rs` 의 `run_streaming` 패턴 base + `--session-id`/`--resume` 분기:

```rust
pub struct ResumeInput {
    pub conv_id: String,
    pub prompt: String,
    pub project_path: Option<String>,
    pub model: String,
    pub resume_session_id: Option<String>,  // None 이면 신규
}

pub async fn run_streaming<F, G, C>(
    input: ResumeInput,
    on_text: F,        // assistant text chunk
    on_event: G,       // tool_use, rate_limit_event 등 메타
    cancel: C,         // stream abort flag
) -> Result<RunOutput, AppError>
where
    F: FnMut(String),
    G: FnMut(String),
    C: Fn() -> bool,
{
    let mut cmd = Command::new("claude");
    cmd.no_console();
    cmd.arg("-p")
       .arg("--output-format").arg("stream-json")
       .arg("--input-format").arg("text")
       .arg("--verbose")
       .arg("--model").arg(&input.model)
       .arg("--dangerously-skip-permissions");

    // session 분기
    let session_id = match input.resume_session_id {
        Some(id) => {
            cmd.arg("--resume").arg(&id);
            id
        }
        None => {
            let new_id = Uuid::new_v4().to_string();
            cmd.arg("--session-id").arg(&new_id);
            new_id
        }
    };

    cmd.current_dir(resolve_cwd(input.project_path.as_deref()));
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());

    let mut child = cmd.spawn()?;
    let mut stdin = child.stdin.take().unwrap();
    write!(stdin, "{}", input.prompt)?;
    drop(stdin);  // EOF — claude 가 single-shot 으로 응답

    // stdout stream-json line-by-line parse
    let stdout = child.stdout.take().unwrap();
    let reader = BufReader::new(stdout);
    let mut output = RunOutput { session_id: session_id.clone(), ..Default::default() };

    for line in reader.lines() {
        if cancel() {
            child.kill()?;
            return Err(AppError::Agent("cancelled".into()));
        }
        let line = line?;
        if line.is_empty() { continue; }
        // JSON parse + dispatch
        let v: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => { on_text(line); continue; }
        };
        match v.get("type").and_then(|t| t.as_str()) {
            Some("assistant") => {
                // text chunk extraction
                if let Some(content) = v.pointer("/message/content/0/text").and_then(|t| t.as_str()) {
                    on_text(content.to_string());
                }
            }
            Some("rate_limit_event") => {
                // ignore or surface as status (별 axis)
            }
            Some("result") => {
                // final session_id capture (이전 turn 과 동일해야)
                if let Some(sid) = v.get("session_id").and_then(|s| s.as_str()) {
                    output.session_id = sid.to_string();
                }
                if let Some(cost) = v.get("total_cost_usd").and_then(|c| c.as_f64()) {
                    output.cost_usd = cost;
                }
                // usage 추출 etc.
            }
            _ => on_event(line),
        }
    }

    let status = child.wait()?;
    if !status.success() {
        return Err(AppError::Agent(format!("claude exit {:?}", status.code())));
    }
    Ok(output)
}
```

## Step 2: send 경로 dispatch — `src-tauri/src/commands/agents.rs`

기존 `send_claude_*` 함수가 sdk-session 호출 → resume-session 호출로 변경:

```rust
// 기존 (sdk-session, 차단됨):
// claude_sdk_session::send_message(...)

// 신규:
let resume_id = load_resume_token_from_db(&conv_id, "claude-code")?;
let result = claude_resume_session::run_streaming(
    ResumeInput { conv_id, prompt, project_path, model, resume_session_id: resume_id },
    on_text, on_event, cancel,
).await?;
// 응답 후 DB resume_token 갱신
update_resume_token_in_db(&conv_id, "claude-code", &result.session_id)?;
```

## Step 3: `claude_sdk_session.rs` deprecate

- 모듈 삭제 X — 코드 유지
- `lib.rs` 의 command 등록 (`restart_sdk_session`, `kill_session_*`) 도 유지
- Default 경로에서 제외 (agents.rs dispatch 가 resume-session 으로)
- 환경변수 `TUNAFLOW_USE_SDK_SESSION=1` 설정 시 sdk-session 사용 — 정책 우회 path 발견 시 즉시 활성화

## Step 4: stream parser 재사용

`claude.rs` 의 기존 stream-json parsing 코드 *그대로* 사용. resume-session 도 동일 format 반환. parser 변경 없음. 단:
- 첫 line 이 `rate_limit_event` 로 시작 가능 — line filter 추가 또는 status badge 로 surface (선택, 별 axis)

## Step 5: cancel mechanism

기존 stream abort token (`CANCEL_FLAGS` registry) 그대로 재사용. resume-session 의 spawn loop 에서 `cancel()` poll → child kill.

# 검증

## Manual smoke (사용자 dev 모드)

1. **새 conversation** — claude 엔진 선택 → 첫 메시지 → 응답 OK
2. **multi-turn** — 같은 conv 에서 두번째 메시지 → prior 답 reference (e.g., "방금 말한 숫자 뭐였지?" → 정답 도출)
3. **streaming** — 긴 응답 시 chunk 별 frontend 도착 visible
4. **tool use** — claude 가 file read / bash 시도 → 정상 동작
5. **engine 변경** — sonnet → opus → 새 session 시작 (resume_token clear)
6. **cancel** — 응답 중 cancel 클릭 → stream abort + claude process kill
7. **rate limit** — `rate_limit_event` chunk 가 frontend 에 도달해도 깨짐 없음 (parser line filter)

## 자동

- `cargo check` / `cargo test --lib` (Rust)
- `npx tsc --noEmit` / `npx vitest run` (Frontend, 본 변경은 backend-only 라 회귀 가능성 낮음)

## CI

- macOS dmg 빌드 통과
- Windows nsis 빌드 통과 (사용자 Windows 환경 검증 시)

# 후속 / Sibling

- `sdkUrlSessionModePlan` (직전 transport, 차단됨) — superseded by 본 plan
- (보류) **claude `/command` slash 지원 검토** — 현재 사용 X 라 본 plan scope 외. 필요 시 별 plan
- (보류) **rate_limit_event status badge** — frontend 에 사용자 가시 표시. 별 axis
- (P2) **spawn overhead 최적화** — 매 message ~2.5s 가 사용자 가시. claude 의 *partial cache reuse* 또는 *stream batching* 으로 최적화 가능 시 별 plan
- **windowsBetaHardeningPlan_2026-04-26** — Windows 환경 작업 시 본 transition 도 같이 검증

# Architect / Developer 핸드오프 프롬프트

```
[작업] Claude transport 전환 — sdk-session → resume-session (-p --session-id/--resume)

[SSOT] docs/plans/claudeResumeSessionTransitionPlan_2026-04-29.md

[배경 3줄]
- claude CLI 2.1.121 (2026-04-28 update) 가 --sdk-url localhost reject 정책 도입 → tunaFlow sdk-session 영구 차단
- 우회 path (binary patch / hosts 매핑 / desktop app 빈틈 / Anthropic contact) 모두 부적합 또는 dead-end
- 채택: claude CLI 의 공식 -p --session-id/--resume path. manual 검증 완료 (cache hit + multi-turn + stateful)

[수정 범위]
1) src-tauri/src/agents/claude_resume_session.rs (신규) — plan §Step 1 구현 그대로
2) src-tauri/src/commands/agents.rs — claude send dispatch 를 sdk-session → resume-session 으로 변경
3) src-tauri/src/agents/claude_sdk_session.rs — deprecate (코드 유지, env TUNAFLOW_USE_SDK_SESSION=1 시만 활성화)
4) DB resume_token mechanism 그대로 재사용 (변경 없음)

[검증]
- cargo check / cargo test --lib
- npm run tauri dev → smoke (plan §검증 §Manual 1~7 모두)
- 사용자 dev 모드에서 한 conversation 에 multi-turn 5+ 시도 → 정상

[커밋 분리]
- feat(claude): resume-session transport (replace --sdk-url after 2.1.121 policy block)
- chore(sdk-session): deprecate behind TUNAFLOW_USE_SDK_SESSION env
- docs(plans): register claudeResumeSessionTransitionPlan + sdkUrlSessionModePlan superseded mark

trailer: Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>

[CI 정책]
PR 생성 직후 macOS / Windows 빌드 둘 다 ✓ 확인 후 admin merge.
본 변경은 cross-platform 영향이라 self-trust admin merge 의 *예외* — CI watch 필수.

[PR 제목]
feat(claude): resume-session transport — replace blocked --sdk-url path
```
