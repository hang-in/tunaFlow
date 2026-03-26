# tunaFlow ContextPack 추적성 계획

- 작성 시각: 2026-03-27
- 상태: 진행 예정

## 문제

현재 에이전트 응답이 어떤 ContextPack으로 생성되었는지 사후 확인할 방법이 없다.

구체적으로:

1. `trace_log`에 `context_mode` (Lite/Standard/Full)가 기록되지 않음
2. 어떤 섹션(plan/findings/artifacts/rawq/skills/cross-session/thread-inheritance)이 포함되었는지 알 수 없음
3. system prompt의 fingerprint/hash가 메시지나 trace에 저장되지 않음
4. 같은 대화에서 연속된 두 메시지가 같은 context를 공유했는지 구분 불가

사용자가 겪을 수 있는 혼란:

- "이 응답은 코드를 참고한 건가, 안 한 건가?" (rawq 포함 여부)
- "Plan이 반영된 응답인가?" (plan section 포함 여부)
- "이전 대화 맥락을 본 건가?" (parent context / cross-session 포함 여부)
- "왜 같은 질문인데 다른 답이 나오지?" (context mode 차이)

## 현재 구조

### 결정 시점
`agents.rs`의 `send_with_claude` / `stream_with_claude`에서 `ctx_mode`를 결정:

```
is_branch || agent_name || system_prompt → Standard
otherwise → Lite
Full → 현재 자동 승격 조건 없음 (skills/rawq/cross-session이 있을 때)
```

### 조립 시점
`context_pack.rs`의 각 `build_*_section` 함수에서 섹션별로 조립.
`combine_prompt_parts`로 합친 뒤 `guardrail::enforce_total_limit`로 잘라냄.

### 기록 시점
`trace_log`에 `input_tokens`, `output_tokens`, `cost_usd`, `duration_ms` 등은 기록됨.
하지만 **어떤 context가 들어갔는지**는 기록되지 않음.

## 제안 구조

### Phase 1: trace_log에 context metadata 추가

`trace_log` 또는 별도 테이블에 아래를 기록:

| 필드 | 설명 |
|------|------|
| `context_mode` | `"lite"` / `"standard"` / `"full"` |
| `sections_included` | JSON 배열: `["plan","findings","rawq","thread_inheritance",...]` |
| `system_prompt_hash` | SHA-256 truncated (8자) — 같은 prompt인지 비교용 |
| `system_prompt_length` | 바이트 수 — prompt 크기 추적 |

기록 위치: `insert_trace_log` 호출 시 추가 파라미터로.

### Phase 2: UI에서 context 확인

메시지별로 "어떤 context로 생성되었는가"를 볼 수 있는 UI:

- 메시지 헤더 또는 hover에 context mode 뱃지
- Trace 패널에서 해당 span의 context metadata 표시
- "이 응답에 포함된 context" 상세 보기

### Phase 3: context 비교

- 연속 메시지의 context 차이 하이라이트
- context mode 변경 시 시각적 표시
- "rawq가 이 응답에 포함되었는가" 필터

## 현재 코드 참조

- `src-tauri/src/commands/agents.rs` — ctx_mode 결정, combine_prompt_parts 호출
- `src-tauri/src/commands/agents_helpers/context_pack.rs` — 각 섹션 빌더
- `src-tauri/src/commands/agents_helpers/trace_log.rs` — trace 기록
- `src-tauri/src/db/schema.rs` — trace_log 스키마
- `src/components/tunaflow/context-panel/TracePanel.tsx` — trace UI

## 우선순위

Phase 1은 DB migration + trace_log 확장으로 비용이 낮다.
Phase 2는 UI 작업이 필요하지만 TracePanel에 이미 span 표시가 있으므로 확장 가능.
Phase 3은 Phase 1+2 완료 후 검토.

## 프로젝트 중심 원칙과의 관계

context 추적도 현재 프로젝트 범위 안에서만 의미가 있다.
크로스 프로젝트 context 비교는 목표가 아니다.
