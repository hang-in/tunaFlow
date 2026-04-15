---
title: Engine Server Mode — PTY 대체 아키텍처
status: planned
created_at: 2026-04-15
priority: P1
related: cicdReleasePlan.md, betaReleaseReadinessPlan.md
---

# Engine Server Mode — PTY 대체 아키텍처

> PTY subprocess + raw text 파싱 대신 각 엔진의 구조화된 IPC 인터페이스를 활용해
> ANSI 파싱 복잡도를 제거하고 세션 제어(interrupt, resume)를 안정화한다.

---

## 1. 배경 및 동기

### 현재 PTY 방식의 문제점

| 문제 | 상세 |
|------|------|
| ANSI 파싱 복잡도 | raw text + escape code 파싱, 엔진마다 다른 출력 형식 |
| 완료 감지 불안정 | JSONL 빠른 완료 감지 실패 간헐적 발생 (P1 알려진 이슈) |
| 권한 요청 처리 | 파싱으로 감지 → 오탐 가능성 |
| 세션 지속 | PTY 프로세스 생존에 의존, 네트워크 끊김 = 세션 소멸 |
| 엔진별 파서 | Codex/Gemini 파서 별도 구현 필요 |

### 발견 계기

Claude Code 소스코드(`_research/_util/claude-code/src/server/`) 분석에서
`claude server` 내장 HTTP + WebSocket 서버 모드 확인.
약관 적합성: OAuth 토큰은 `claude` 프로세스 내부에만 존재,
tunaFlow는 localhost 통신만 → `-p` 모드와 동일한 법적 근거로 허용.

---

## 2. 엔진별 서버/IPC 인터페이스 현황

### 2.1 Claude

**방식**: `claude server` 내장 HTTP + WebSocket 서버

```bash
claude server --port 0 --auth-token <token>
# → HTTP: POST /sessions → { session_id, ws_url, work_dir }
# → WS: ws_url로 연결 → SDKMessage JSONL 양방향
```

**프로토콜**:
- 세션 생성: `POST /sessions` → `{ session_id, ws_url, work_dir }`
- 메시지 전송: WS로 `{ type: "user", message: { role, content }, ... }`
- 응답 수신: `SDKMessage` 이벤트 스트리밍 (`assistant`, `tool_use`, `tool_result`, `result` 등)
- 권한 요청: `{ type: "control_request", request: { subtype: "can_use_tool" } }`
- interrupt: `{ type: "control_request", request: { subtype: "interrupt" } }`

**장점**: 세션 재연결 가능, 구조화된 이벤트, 공식 Desktop 앱이 이 방식 사용

**주요 파일** (claude-code 소스):
- `src/server/server.ts` — HTTP 서버
- `src/server/createDirectConnectSession.ts` — 세션 생성
- `src/server/directConnectManager.ts` — WebSocket 관리
- `src/server/types.ts` — 메시지 스키마

---

### 2.2 Codex

**방식**: `codex app-server` (stdio JSONL 기본, WS 실험적)

```bash
codex app-server              # stdio JSONL (JSON-RPC 2.0 lite)
codex app-server --listen ws://127.0.0.1:4500  # WS (실험적)
```

**단기 현실적 경로**: `codex exec --json "prompt"` — JSONL 이벤트 스트리밍

```
{ "type": "thread.started", ... }
{ "type": "turn.started", ... }
{ "type": "item.text.delta", "delta": "..." }
{ "type": "turn.completed", ... }
```

**상태**: stdio 안정, WebSocket 실험적 (프로덕션 미권장)

---

### 2.3 Gemini

**방식**: 서버 모드 없음 (daemon 기능 요청 중 — issue #15338)

**현실적 경로**: `-p --output-format stream-json` subprocess pipe

```bash
gemini -p "prompt" --output-format stream-json
# → JSONL: { "type": "init"|"message"|"tool_use"|"result", ... }
```

**상태**: stateless (세션 지속 불가), 각 호출 독립

---

### 2.4 요약

| 엔진 | 서버 모드 | 단기 경로 | 세션 지속 |
|------|-----------|-----------|-----------|
| Claude | ✅ HTTP+WS | `claude server` | ✅ WS 재연결 |
| Codex | ✅ stdio/WS | `codex exec --json` | ⚠️ WS 실험적 |
| Gemini | ❌ 없음 | `-p stream-json` | ❌ stateless |
| OpenCode | 미조사 | — | — |

---

## 3. 목표 아키텍처

```
┌─────────────────────────────────────────────┐
│  tunaFlow Rust backend (src-tauri)           │
│                                              │
│  EngineConnection trait                      │
│    ├── ClaudeServerConnection   (WS)         │
│    ├── CodexServerConnection    (stdio JSONL)│
│    └── GeminiPipeConnection     (pipe JSON)  │
│                                              │
│  → 공통 이벤트: AgentEvent enum             │
│      TextDelta, ToolUse, ToolResult,         │
│      PermissionRequest, Completed, Error     │
└─────────────────────────────────────────────┘
```

### 공통 이벤트 추상화

```rust
pub enum AgentEvent {
    TextDelta { text: String },
    ToolUse { id: String, name: String, input: serde_json::Value },
    ToolResult { id: String, content: String },
    PermissionRequest { id: String, tool: String },
    Completed { cost_usd: f64, input_tokens: i64, output_tokens: i64 },
    Error { message: String },
}
```

현재 PTY 파싱 결과와 동일한 이벤트 타입으로 매핑 → 프론트엔드 변경 최소화.

---

## 4. 구현 단계

### Phase 1 — Claude 서버 모드 (P1)

**목표**: 현재 Claude PTY를 `claude server` + WebSocket으로 교체

**작업**:
- [ ] `claude server` 프로세스 시작/종료 관리 (Rust)
- [ ] lockfile 기반 기존 서버 감지 및 재사용
- [ ] `POST /sessions` → session 생성
- [ ] WebSocket 연결 + SDKMessage 파싱
- [ ] `AgentEvent`로 변환 → 기존 Tauri 이벤트 emit
- [ ] 권한 요청 UI 연동 (`control_request` → 프론트엔드 팝업)
- [ ] interrupt 전송
- [ ] 기존 Claude PTY 경로 feature-flag로 유지 (롤백 대비)

**검증**: 기존 PTY 테스트 케이스 동일 통과 확인

---

### Phase 2 — Codex stdio JSONL (P2)

**목표**: Codex PTY를 `codex exec --json` pipe로 교체

**작업**:
- [ ] `codex exec --json` subprocess 실행
- [ ] JSONL 이벤트 파싱 → `AgentEvent` 변환
- [ ] 세션 지속: `--conversation-id` 플래그 확인 후 적용
- [ ] 기존 Codex PTY 경로 유지 (롤백 대비)

---

### Phase 3 — Gemini stream-json (P2)

**목표**: Gemini PTY를 `-p --output-format stream-json`으로 교체

**작업**:
- [ ] Gemini stream-json 이벤트 스키마 확정
- [ ] `AgentEvent` 변환
- [ ] stateless 특성 반영 (세션 재개 방식 재설계 필요)

---

### Phase 4 — OpenCode 조사 (P3)

- [ ] OpenCode 서버/IPC 모드 존재 여부 조사
- [ ] `app-server` 또는 유사 기능 확인

---

## 5. 기존 PTY와 비교

| 항목 | 현재 PTY | 서버 모드 |
|------|----------|-----------|
| ANSI 파싱 | 필요 (복잡) | 불필요 |
| 완료 감지 | 불안정 (P1 이슈) | `message_stop` 이벤트로 명확 |
| 권한 처리 | 파싱 기반 (오탐 가능) | `control_request` 이벤트 |
| 세션 재연결 | 불가 | Claude WS 가능 |
| 다중 세션 | 프로세스별 PTY | 서버 1개 + 세션 N개 (Claude) |
| interrupt | PTY Ctrl+C | WS control_request |
| 엔진 파서 수 | 엔진별 별도 | 공통 AgentEvent 추상화 |

---

## 6. 관련 파일 (현재)

| 파일 | 역할 |
|------|------|
| `src-tauri/src/commands/pty/` | 현재 PTY 구현 |
| `src-tauri/src/agents/claude.rs` | Claude subprocess 관리 |
| `src-tauri/src/agents/rawq.rs` | rawq sidecar (참고: triple 처리 패턴) |
| `_research/_util/claude-code/src/server/` | Claude server 소스 (참조용) |

---

## 7. 결정 사항

- **약관 적합성**: OAuth 토큰은 각 엔진 프로세스 내부에만 존재, tunaFlow는 localhost 통신만 → `-p` 모드와 동일 근거로 허용
- **롤백 전략**: 기존 PTY 경로를 feature-flag로 유지하다 Phase 1 안정화 후 제거
- **우선순위**: Claude(가장 성숙) → Codex → Gemini 순서
