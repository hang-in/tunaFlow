# PTY 전체 통합 계획 — `-p` → PTY 전환 로드맵

> Status: in_progress
> Created: 2026-04-10
> Branch: `feature/pty-interactive`
> 전제: PTY PoC 동작 확인 완료 (spawn + stdin/stdout + sendWithEngine 분기)

---

## 현재 상태

| 항목 | 상태 |
|------|------|
| portable-pty Rust 모듈 | ✅ `commands/pty.rs` (spawn/write/resize/kill) |
| xterm.js Terminal 탭 | ✅ 디버그/보기 전용 |
| sendWithEngine PTY 분기 | ✅ PoC (ptyStore → pty_write → pty:output → ANSI strip) |
| ANSI strip | ✅ 기본 정규식 |
| 완료 감지 | ✅ "Worked for" 패턴 (1초 debounce) |

---

## Phase 1: sendWithEngine PTY 안정화 (현재 세션)

Chat 탭에서 PTY 모드가 안정적으로 동작하도록.

### 1-1. ANSI strip 정확도 개선
- Claude Code TUI의 출력 패턴 실제 분석 (Terminal 탭 활용)
- 박스 그리기 문자, 프로그레스 바, 색상 코드 등 처리
- 필요 시 strip-ansi-escapes 크레이트로 Rust 쪽 처리

### 1-2. 완료 감지 정확도
- "Worked for Xs" 외 추가 패턴: `❯` 프롬프트, idle timeout
- False positive 방지 (응답 중간에 "Worked for" 텍스트가 포함된 경우)

### 1-3. 결과를 DB에 영속화
- 현재: 메시지가 Zustand store에만 존재 (새로고침 시 사라짐)
- 목표: 완료 시 `invoke("create_message", ...)` → DB 저장
- `agent:completed` 이벤트와의 호환 (워크플로우 자동 감지 등)

---

## Phase 2: ContextPack → 파일 기반 전환

PTY 세션에서는 프롬프트에 ContextPack을 합칠 수 없음 (이미 실행 중인 프로세스). 대신 파일로 전달.

### 2-1. CLAUDE.md 동적 섹션 갱신
```
프로젝트의 CLAUDE.md에 ## tunaFlow Context 섹션 동적 갱신:
  - Tier 0: project path, identity
  - Tier 1: active plan, findings (조건부)
  - Claude Code가 자동으로 읽음
```

### 2-2. 갱신 타이밍
- 메시지 전송 직전 (sendViaPty에서)
- Plan 상태 변경 시
- 프로젝트 전환 시

### 2-3. 다른 엔진용
```
Claude → CLAUDE.md
Codex  → AGENTS.md (또는 codex.md)
Gemini → GEMINI.md
```
각 CLI가 자동으로 읽는 설정 파일에 동일한 컨텍스트 갱신.

---

## Phase 3: 워크플로우 통합

### 3-1. Architect/Developer/Reviewer PTY 세션
```
현재:  워크플로우 각 단계마다 -p로 새 프로세스 spawn
목표:  PTY 세션에서 역할별 지시 → multi-step 작업 → 결과 수집

예: Developer phase
  현재: claude -p "이 subtask를 구현해줘" → 텍스트 응답만
  PTY:  PTY에 "이 subtask를 구현해줘" 전달 → 파일 편집 + 테스트 + 결과 보고
        → Claude가 실제로 코드를 작성하고 테스트까지 돌림
```

### 3-2. 마커 감지 (기존 tool-request 시스템 활용)
- PTY 출력에서 `<!-- tunaflow:plan-proposal -->` 등 마커 감지
- 기존 PlanProposalCard, ReviewVerdictCard 등 UI 자동 연동
- ANSI strip 후 마커 파싱

### 3-3. RT는 -p 유지
```
RT: 참가자별 독립 세션 → -p 모드 (stream_participant) 유지
    PTY 상주 세션은 오버킬
```

---

## Phase 4: Engine trait 통합

### 4-1. Rust trait 정의
```rust
trait AgentEngine {
    fn run(&self, input: RunInput) -> Result<RunOutput, AppError>;
    fn stream_run(&self, input: RunInput, callbacks: StreamCallbacks) -> Result<RunOutput, AppError>;
}
```

### 4-2. PTY adapter 추가
```rust
struct PtyEngine {
    session_id: u32,
}

impl AgentEngine for PtyEngine {
    fn run(&self, input: RunInput) -> Result<RunOutput, AppError> {
        // PTY stdin으로 전달 → 완료 대기 → 결과 수집
    }
}
```

### 4-3. start_*_stream commands 통합
```
현재: start_claude_stream, start_gemini_stream, start_codex_run, ... (5개)
목표: start_engine_run(engine, mode: "pty" | "cli" | "sdk")
```

---

## Phase 5: 프로세스 관리 + 안정성

### 5-1. PTY 세션 생명주기
- 앱 시작 시: 선택된 프로젝트에 대해 PTY 자동 시작 (선택사항)
- 프로젝트 전환 시: 이전 PTY kill → 새 PTY spawn
- 앱 종료 시: 모든 PTY cleanup
- PTY 비정상 종료 시: 자동 재시작 + 사용자 알림

### 5-2. 동시 세션 관리
```
메인 채팅: PTY 세션 1개 (Claude)
드로어 Branch: 별도 PTY 세션 (선택사항)
RT: -p 모드 (변경 없음)
```

### 5-3. 에러 복구
- PTY exit → 자동 재시작 옵션
- 네트워크 끊김 → PTY는 로컬이므로 영향 없음
- Claude CLI 업데이트 → PTY 재시작으로 대응

---

## 구현 순서

| # | 항목 | 의존성 | 예상 난이도 |
|---|------|--------|-----------|
| 1 | **ANSI strip 개선 + 완료 감지 안정화** | 없음 | 낮 |
| 2 | **결과 DB 영속화** | #1 | 낮 |
| 3 | **CLAUDE.md 동적 갱신** | 없음 | 중 |
| 4 | **워크플로우 마커 감지** | #1 | 중 |
| 5 | **Engine trait 통합** | #1-4 안정화 후 | 높 |
| 6 | **프로세스 관리 + 안정성** | #5 | 중 |

---

## 사이드이펙트 체크리스트

| 항목 | 영향 | 대응 |
|------|------|------|
| 기존 `-p` 경로 | Phase 5까지 유지 → 이후 RT 전용으로 축소 | 점진적 전환 |
| RT | 변경 없음 (-p + stream_participant 유지) | — |
| 워크플로우 | 마커 감지 방식 변경 (stdout 파싱 → PTY 출력 파싱) | Phase 4 |
| ContextPack | 프롬프트 합치기 → 파일 갱신으로 전환 | Phase 2 |
| DB 영속성 | PTY 결과도 messages 테이블에 저장 필요 | Phase 2 |
| 테스트 | PTY mock 필요 + 기존 streaming-flow 테스트 유지 | Phase별 |
| 고아 프로세스 | PTY 세션 cleanup 필수 | Phase 5 |

---

## 핵심 원칙

> PTY는 **에이전트의 능력을 제한하지 않는** 실행 경로.
> `-p` 모드가 에이전트를 "답변 기계"로 제한했다면,
> PTY는 에이전트를 "작업자"로 풀어주는 것.
> 트레이드오프는 리팩토링 공수 뿐 — 한 번 하면 끝.
