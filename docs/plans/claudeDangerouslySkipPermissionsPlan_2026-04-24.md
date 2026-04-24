---
title: Claude headless permission flag 교체 — bypassPermissions → dangerously-skip-permissions (Issue #178 hotfix)
status: ready-to-implement
priority: P0 (기능 차단 — fs 도구 호출 시 무한 행)
created_at: 2026-04-24
related:
  - https://github.com/dghong/tunaFlow/issues/178
  - docs/plans/sdkUrlSessionModePlan.md
canonical: true
owners:
  - architect (본 문서 작성)
  - developer (구현)
---

# 배경

공개 첫날 커뮤니티 사용자 `batmania52` 가 제보 (#178):

> Claude 가 프로젝트 디렉터리 **밖** 의 파일 접근이 필요한 작업 (예: `ln -sf ~/.claude/skills/x .claude/skills/x` 심볼릭 링크) 실행 시 macOS 승인 알림 표시 → tunaFlow 스피너 무한 → **10 분 후 sdk-session timeout**.

원인: tunaFlow 가 Claude CLI 에 `--permission-mode bypassPermissions` 를 넘기지만, 이 모드가 **놓치는 케이스는 터미널 raw prompt 로 출력**됨 (`stream-json` 이벤트 아님). tunaFlow 는 stdin 을 메시지 전송용으로 점유하고 있어 prompt 에 `y/n` 응답 불가 → 무한 hang.

사용자가 이미 조사한 결과 `stream-json` 프로토콜에 **`permission_request` 이벤트 자체가 존재하지 않음** → 제대로 된 승인 UI 는 Anthropic upstream 에 기능 요청 필요 → 현재는 **작업 우회 불가**.

**해결 방안**: `--permission-mode bypassPermissions` 를 `--dangerously-skip-permissions` 로 교체. CLI 쪽 "sandboxed 환경" 전용 플래그로, 실제로 모든 prompt 를 완전히 억제한다.

# 현재 상태 (사실 확인)

이슈 본문은 2 곳을 지적했지만 실제 전수조사 결과 **3 곳** 에서 같은 플래그가 사용됨:

## (A) `src-tauri/src/agents/claude.rs:162` — `stream_run`

```rust
// line 156-162
let mut cmd = Command::new("claude");
cmd.arg("-p").arg(&input.prompt)
    .arg("--output-format").arg("stream-json")
    .arg("--verbose")
    .arg("--permission-mode").arg("bypassPermissions")  // ← 수정 대상
```

Streaming 기반 Claude 호출 경로. 현재 주 경로.

## (B) `src-tauri/src/agents/claude.rs:380` — `run`

```rust
// line 374-380
pub fn run(input: RunInput) -> Result<RunOutput, AppError> {
    let mut cmd = Command::new("claude");
    cmd.arg("-p").arg(&input.prompt)
        .arg("--output-format").arg("json")
        .arg("--permission-mode").arg("bypassPermissions")  // ← 수정 대상
```

One-shot JSON 호출 경로. 현재 주로 사용되지 않지만 남아 있음.

## (C) `src-tauri/src/agents/claude_sdk_session.rs:381` — SDK session spawn ⚠️ **이슈 본문에 없음**

```rust
// line 374-381
let mut cmd = TokioCommand::new("claude");
cmd.arg("--print")
    .arg("--sdk-url").arg(&sdk_url)
    .arg("--model").arg(model)
    .arg("--input-format").arg("stream-json")
    .arg("--output-format").arg("stream-json")
    .arg("--replay-user-messages")
    .arg("--permission-mode").arg("bypassPermissions")  // ← 수정 대상
```

s36~s37 에서 도입된 sdk-url WS 경로. **현재 새 기능이 기본적으로 타는 경로** (project_pty_value 메모리 참조). 이슈 본문이 이걸 빠뜨린 건 단순 누락 — 셋 다 교체해야 완결.

## (D) 플래그 동작 차이 (사용자 조사 요약)

- `--permission-mode bypassPermissions`: **일부** prompt 를 건너뛰지만, 디렉터리 외부 접근 / OS 보안 prompt 는 여전히 터미널 raw 로 출력
- `--dangerously-skip-permissions`: **모든** prompt 완전 억제. CLI 가 "sandboxed container" 전제로 설계된 플래그 (이름이 `dangerously` 인 이유)

# 설계 (MVP — hotfix 1일)

## (1) 세 call site 모두 교체

**동일한 치환 3회**:

```rust
// 변경 전
.arg("--permission-mode").arg("bypassPermissions")

// 변경 후
.arg("--dangerously-skip-permissions")
```

파일:
- `src-tauri/src/agents/claude.rs:162`
- `src-tauri/src/agents/claude.rs:380`
- `src-tauri/src/agents/claude_sdk_session.rs:381`

**일괄 검증**: `rg -n 'bypassPermissions' src-tauri/` 결과 0 건이어야 함.

## (2) 사용자 대상 문서 갱신 (README + 보안 섹션)

플래그 이름이 `dangerously` 이므로 사용자가 README 에서 발견했을 때 놀라지 않도록 **사전 설명** 필요.

- **README.md** 보안 섹션 (없으면 신규):

```markdown
## 보안 & 권한

tunaFlow 는 Claude CLI 를 `--dangerously-skip-permissions` 플래그로 실행합니다.
이는 CLI 가 디렉터리 외부 파일 접근, 시스템 명령 등에 대한 승인 prompt 를 건너뛴다는
의미입니다.

**사용자 책임**:
- 에이전트가 작업하는 프로젝트 디렉터리를 신중히 선택
- 신뢰할 수 없는 프롬프트 / 도구 / MCP 서버를 활성화하지 말 것
- 에이전트가 수행한 작업을 정기적으로 검토

이 설정을 바꾸는 건 현재 제공되지 않습니다 (`stream-json` 에 permission event 가
없어 UI 승인 흐름 구현 불가 — Anthropic upstream 개선 대기 중).
```

- **CHANGELOG**: "security: Claude headless 권한 플래그 dangerously-skip-permissions 로 변경 — 디렉터리 외부 접근 시 무한 hang 해결 (#178)"

## (3) 이슈 대응 — upstream 기능 요청 이슈 제기 (선택)

Anthropic (claude-code) 에 `permission_request` 이벤트를 `stream-json` 프로토콜에 추가하는 기능 요청. 채택되면 tunaFlow 쪽 플래그를 되돌리고 per-tool 승인 UI 구현 가능.

- 본 plan 스코프 밖 (별도 outreach)
- `postBetaBacklogPlan` 에 항목 추가 제안

## (4) 테스트

### Rust 유닛 테스트 — 실현 불가

Claude CLI 호출은 네트워크 + 실제 바이너리 의존. mock 하기 어렵다.

### 정적 검증

- `rg -n 'bypassPermissions' src-tauri/` 결과 **0 건** (invariant 체크)
- `rg -n 'dangerously-skip-permissions' src-tauri/` 결과 **3 건** (세 call site 확인)

### 수동 검증 시나리오 (PR 검증용)

1. 프로젝트 열고 Claude 에이전트 선택
2. 프롬프트: "프로젝트 루트에 `~/.claude/skills/test.md` 로 심볼릭 링크 만들어주세요"
3. **Before (bypassPermissions)**: macOS 알림 + 스피너 무한 + 10 분 후 timeout
4. **After (dangerously-skip-permissions)**: 알림 없음 + 작업 즉시 완료
5. 일반 in-project 작업 (`npx tsc --noEmit` 등) 도 정상 작동 확인 (regression 체크)

# Invariants

- **[INV-1]** `bypassPermissions` 문자열은 코드베이스 어디에도 남아 있지 않다. 검증: `rg 'bypassPermissions' src-tauri/ src/` 결과 0 건.
- **[INV-2]** Claude CLI 를 subprocess 로 띄우는 모든 call site 는 `--dangerously-skip-permissions` 를 포함한다. 검증: 세 call site grep.
- **[INV-3]** README / 보안 섹션에 이 플래그 사용 사실과 사용자 책임이 명시돼 있다. 검증: README grep.
- **[INV-4]** 신규 Claude 호출 경로가 추가되면 **반드시 동일 플래그 포함**. 리뷰 체크리스트에 추가.

# Rationale

## 왜 플래그 교체가 "안전" 한가 (사용자 논거 검증)

- tunaFlow 는 **로컬 데스크탑 앱**. 에이전트가 돌아가는 디렉터리는 **사용자가 명시적으로 선택** (프로젝트 오픈).
- Claude CLI `--dangerously-skip-permissions` 의 의도된 용도는 **sandboxed environment** (Docker, CI, 1회성 실행). tunaFlow 의 실행 환경은 이에 해당하지 않지만:
  - **사용자가 이미 Claude Code 전체 권한 가진 상태** (tunaFlow 가 없어도 Claude CLI 직접 실행 시 동일 권한)
  - tunaFlow 는 그 실행을 프로그램적으로 자동화할 뿐 권한 확장 없음
- 결과적으로 **보안 경계는 "사용자가 에이전트를 어떤 디렉터리에 풀어놓는가"** 로 이동한다. 이 점은 README 보안 섹션에서 명시.

## 왜 UI 승인 흐름을 만들지 않는가

- `stream-json` 프로토콜에 `permission_request` 이벤트 없음 → **기술적 구현 경로 부재**
- Claude CLI 는 prompt 를 터미널에 직접 써버리므로 stdin 으로 응답하려면 PTY 필요
- tunaFlow 는 s36~ PTY → WS 전환 중 (`project_pty_value` 메모리 참조). PTY 복원은 큰 구조 변경
- 따라서 **upstream 변경 (Anthropic) 대기** 가 현실적 경로. 본 plan 은 그동안의 interim fix.

## 왜 이슈 본문에 없는 `claude_sdk_session.rs` 도 같이 고치는가

- 이슈 본문은 L162, L380 만 언급 → 제보자가 해당 경로 실행 시 히트
- 전수조사 결과 `claude_sdk_session.rs:381` 에 같은 패턴 존재
- 현재 tunaFlow 는 s36~s37 에서 sdk-url WS 경로가 **새 기본값** → 여기 안 고치면 **이슈는 여전히 재현**
- 셋 다 교체가 완결적 hotfix

## 왜 3줄 치환에 plan 문서를 쓰는가

- 교체는 3줄이지만, **왜 안전한지 / 왜 UI 승인 UI 안 만드는지** 는 보안 감사 대상
- 미래에 "왜 `dangerously-` 플래그를 쓰지" 라고 누가 물었을 때 이 문서가 답
- `postBetaBacklog` 의 upstream outreach 항목과 연결돼 장기 계획 정리

# Developer 핸드오프 프롬프트

```
[작업] Claude headless permission 플래그 교체 — bypassPermissions → dangerously-skip-permissions (Issue #178 hotfix)

[SSOT] /Users/d9ng/privateProject/tunaFlow/docs/plans/claudeDangerouslySkipPermissionsPlan_2026-04-24.md 먼저 읽기.

[배경 3줄]
- bypassPermissions 가 외부 디렉터리 접근 prompt 를 억제하지 못해 터미널 raw 출력 → 무한 hang
- stream-json 에 permission_request 이벤트 없어 UI 승인 UI 구현 경로 부재
- dangerously-skip-permissions 로 교체해 interim fix, upstream 개선은 별도 outreach

[수정 범위]

1) 수정: src-tauri/src/agents/claude.rs
   - L162 (stream_run): .arg("--permission-mode").arg("bypassPermissions")
                        → .arg("--dangerously-skip-permissions")
   - L380 (run): 동일 치환

2) 수정: src-tauri/src/agents/claude_sdk_session.rs
   - L381 (SDK session spawn): 동일 치환 ⚠️ 이슈 본문에 없지만 필수

3) 검증 명령
   - rg -n 'bypassPermissions' src-tauri/   → 0 건이어야 함
   - rg -n 'dangerously-skip-permissions' src-tauri/   → 3 건이어야 함

4) 수정: README.md
   - 보안 섹션 (없으면 신규) 추가
   - 내용은 SSOT §(2) 참조 — 4 문단 (플래그 설명 / 사용자 책임 / 현 상태 / upstream 상황)

5) 수정: CHANGELOG.md (해당 파일 있으면)
   - "security: Claude headless 권한 플래그 dangerously-skip-permissions 로 변경
     — 디렉터리 외부 접근 시 무한 hang 해결 (#178)"
   - 파일 없으면 CHANGELOG.md 신규 생성 여부는 사용자에게 확인 (기존 운영 패턴 따름)

6) 수정: docs/plans/postBetaBacklogPlan_2026-04-24.md
   - 신규 항목 추가: B-20 "Anthropic upstream 에 stream-json permission_request 이벤트 요청"
   - P2, outreach 카테고리

[검증]
- cd src-tauri && cargo check --all-targets: 0 에러
- rg 결과 invariant 확인 (INV-1 / INV-2)
- 수동 smoke:
    1. 프로젝트 열고 Claude 에이전트 선택
    2. 프롬프트: "프로젝트 루트에 ~/.claude/skills/test.md 로 심볼릭 링크 만들어주세요"
    3. macOS 알림 없이 즉시 완료 확인
    4. 일반 in-project 작업 (tsc --noEmit 등) regression 없는지 확인

[커밋]
- security(claude): replace bypassPermissions with dangerously-skip-permissions (#178)
- docs(readme): security section on permission flag + user responsibility
- docs(backlog): B-20 upstream permission_request event request

각 커밋 trailer 에 Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>

[PR 제목]
security(claude): fix infinite hang on fs permission prompts (#178)

PR 본문에는 before/after 시연 (심링크 작업 or 외부 디렉터리 접근) 필수.

[주의]
- git stash drop/clear 금지
- 세 call site 중 하나라도 누락되면 이슈 재현 — INV-2 grep 으로 반드시 확인
- README 보안 섹션은 **플래그 이름 그대로** 노출 (dangerously 라는 단어 숨기지 않기). 투명성이 신뢰.
- CHANGELOG 가 기존에 없으면 단독 신설하지 말고 사용자에게 확인 요청 — 운영 패턴 벗어나는 결정
```

# 관련 기록

- Issue #178 (`batmania52`, 2026-04-24) — 원 제보. 이슈 본문의 기술 분석이 탄탄해 그대로 반영.
- `project_pty_value.md` (메모리) — PTY → sdk-url WS 경로 전환 맥락. 이 전환 중이라 call site 3 개 공존.
- 후속 plan 후보 (B-20): Anthropic `claude-code` repo 에 `permission_request` 이벤트 기능 요청 이슈 제기. 채택 시 본 plan 의 플래그 되돌리고 UI 승인 흐름 설계.
