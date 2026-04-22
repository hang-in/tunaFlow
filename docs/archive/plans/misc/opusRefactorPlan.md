# OPUS Refactor Plan for tunaFlow

## 0. Purpose
이 문서는 `tunaFlow` 코드베이스가 최근 기능 추가로 커진 상태에서, 구조를 깨지 않고 유지보수성을 높이기 위한 **리팩토링 실행 문서**다.

대상 프로젝트:
- `D:\privateProject\tunaFlow`

이 문서의 목적은 두 가지다.

1. Opus가 어디부터 리팩토링해야 하는지 우선순위를 명확히 알게 한다.
2. Opus가 실제로 파일을 어떻게 분리해야 하는지 설계 기준을 제공한다.

이 문서는 **대규모 재설계 문서가 아니다.**
기존 Tauri / Rust / command layer / DB 구조를 유지한 채, 커진 파일을 **작고 분명한 책임 단위로 나누는 것**이 목표다.

---

## 1. Core Principles

### 유지해야 하는 것
- Tauri 기반 구조
- Rust command layer
- 현재 DB 스키마와 데이터 모델의 기본 방향
- 현재 UI의 3패널 구조
- 기존 기능 동작

### 하지 말아야 하는 것
- 프레임워크 교체
- service/repository/usecase 식 과한 계층화
- 한 번에 전체 폴더 구조 갈아엎기
- DB 대수술
- UI 재설계
- “미래에 좋을 것 같아서” 미리 만드는 추상화

### 리팩토링의 기준
- command entry는 얇게
- 조립(build) / 실행(run) / 저장(persist)을 분리
- 한 파일은 한 종류의 책임만 가지게
- 기존 공개 command 이름과 흐름은 최대한 유지
- 한 단계마다 검증 후 멈추기

---

## 2. Refactor Goals

현재 tunaFlow에서 리팩토링이 필요한 핵심 문제는 아래다.

1. command 파일 하나에 너무 많은 책임이 섞여 있음
2. 문자열 조립과 엔진 실행, DB 기록이 한 함수에 같이 있음
3. 프론트에서 패널 하나가 너무 많은 UI와 invoke를 직접 다룸
4. 향후 planning / MCP / evaluation 추가 시 파일이 더 커질 위험이 큼

리팩토링의 결과는 아래여야 한다.

- 파일이 짧아진다
- 각 모듈 책임이 명확해진다
- 이후 기능 추가 시 수정 범위가 줄어든다
- 기존 기능은 그대로 동작한다

---

## 3. Refactor Priority

아래 순서로 진행하라. 한 번에 여러 축을 동시에 리팩토링하지 마라.

### Priority 1. Frontend Context Panel 분리
대상:
- `src/components/tunaflow/ContextPanel.tsx`

이유:
- 현재 가장 빨리 비대해질 가능성이 높음
- plan/artifact/memo/skill이 계속 이 패널에 붙는 구조
- 프론트에서 가장 효과가 빨리 나는 분리 포인트

목표:
- ContextPanel을 유지하되 내부 섹션 컴포넌트로 분리

### Priority 2. Backend Agents Command 분리
대상:
- `src-tauri/src/commands/agents.rs`

이유:
- ContextPack 조립, compression, trace logging, engine dispatch, DB update가 섞여 있음
- 이후 기능 추가가 가장 많이 몰릴 파일

목표:
- `tauri::command` entry는 유지
- 내부 로직만 helper/module로 분리

### Priority 3. Roundtable 분리
대상:
- `src-tauri/src/commands/roundtable.rs`

이유:
- 이미 최소 리팩터링은 했지만, 여전히 prompt / execution / persist가 한 파일에 집중될 수 있음
- fanout / aggregator / followup 확장 전에 책임을 더 분리하는 게 좋음

### Priority 4. Frontend Invoke API 정리
대상:
- `src/lib`
- `src/components/tunaflow/*`

이유:
- 컴포넌트가 직접 `invoke(...)`를 많이 들기 시작하면 유지보수가 급격히 나빠짐

### Priority 5. Plans / Memos / Artifacts 공통 정리
대상:
- `src-tauri/src/commands/plans.rs`
- `src-tauri/src/commands/memos.rs`
- `src-tauri/src/commands/artifacts.rs`
- `src/types/index.ts`

이유:
- 모두 conversation / branch 주변 context domain
- 지금 당장 구조 통일 효과가 큼

### Priority 6. 나머지 장기 정리
- MCP layer
- evaluation harness
- OTel exporter
- HITL 강화

이건 현재 리팩토링 1차 범위 밖이다.

---

## 4. File Split Design

## 4.1 Frontend

### Current Risk
`ContextPanel.tsx`가 계속 기능을 흡수하는 구조다.

### Target Split
아래처럼 파일을 나누되, 상위 엔트리는 유지하라.

```text
src/components/tunaflow/
  ContextPanel.tsx
  context-panel/
    PlansPanel.tsx
    ArtifactsPanel.tsx
    MemosPanel.tsx
    SkillsPanel.tsx
    BranchesPanel.tsx
    PlanCard.tsx
    SubtaskRow.tsx
```

### Rules
- `ContextPanel.tsx`는 탭/세그먼트 선택과 상위 orchestration만 담당
- 실제 렌더링과 invoke는 하위 panel로 이동
- plan-specific status mapping은 plan panel로 이동
- artifact/memo/skill 관련 세부 UI도 각자 분리

### Minimal Acceptable Version
최소한 아래까지는 분리하라.

- `PlansPanel.tsx`
- `ArtifactsPanel.tsx`
- `MemosPanel.tsx`

---

## 4.2 Frontend API Layer

### Target Split

```text
src/lib/api/
  agents.ts
  plans.ts
  artifacts.ts
  memos.ts
  skills.ts
  roundtable.ts
```

### Rules
- 컴포넌트 안에서 직접 `invoke("...")` 하지 말고 API 함수 호출
- command 문자열은 API layer에만 존재
- 컴포넌트는 데이터 흐름과 렌더링만 담당

### Minimal Acceptable Version
- `plans.ts`
- `artifacts.ts`
- `memos.ts`

---

## 4.3 Backend Agents Command

### Current Risk
`commands/agents.rs`에 아래가 같이 들어 있다.

- skill section 조립
- cross-session section 조립
- rawq section 조립
- context summary 조립
- compression
- trace log write
- resume token 처리
- engine별 dispatch
- DB update

### Target Split

```text
src-tauri/src/commands/
  agents.rs
  agents/
    mod.rs
    context_pack.rs
    compression.rs
    trace_log.rs
    dispatch.rs
    db_updates.rs
```

### Responsibility Rules

#### `commands/agents.rs`
- `#[tauri::command]` entry만 둔다
- input 파싱
- high-level orchestration 호출

#### `agents/context_pack.rs`
- `build_skills_section`
- `build_cross_session_section`
- `build_rawq_section`
- `build_context_summary`
- `combine_prompt_parts`

즉, 문자열/section 조립만 둔다.

#### `agents/compression.rs`
- `compress_context_with_claude`
- `maybe_compress_section`

즉, 압축 시도와 fallback 처리만 둔다.

#### `agents/trace_log.rs`
- `insert_trace_log`

즉, trace_log write helper만 둔다.

#### `agents/dispatch.rs`
- 엔진별 실행 분기
- `claude / codex / gemini / opencode` dispatch helper

#### `agents/db_updates.rs`
- conversations usage update
- resume_token 저장
- 관련 공통 DB helper

### Minimal Acceptable Version
최소한 아래 3개는 분리하라.

- `context_pack.rs`
- `compression.rs`
- `trace_log.rs`

---

## 4.4 Backend Roundtable

### Current Risk
`roundtable.rs`는 이미 전략 분리가 일부 됐지만, 앞으로 prompt building / execution / persistence가 더 커질 수 있다.

### Target Split

```text
src-tauri/src/commands/
  roundtable.rs
  roundtable/
    mod.rs
    prompt.rs
    executor.rs
    persist.rs
    strategy.rs
```

### Responsibility Rules

#### `roundtable.rs`
- command entry
- high-level flow만 유지

#### `roundtable/prompt.rs`
- `build_round_prompt`
- `build_prompt_sources`
- truncate helper if roundtable-specific

#### `roundtable/executor.rs`
- `run_participant`
- `run_round`
- `run_round_sequential`
- future `run_round_fanout`

#### `roundtable/persist.rs`
- `persist_round`
- transcript archive helper if 분리 가치 있으면 이동

#### `roundtable/strategy.rs`
- `RoundStrategy`
- strategy-specific docs/comments

### Minimal Acceptable Version
- `prompt.rs`
- `executor.rs`
- `persist.rs`

---

## 4.5 Context Domain Alignment

### Goal
Plans / Memos / Artifacts / Skills를 “conversation/branch context” 관점에서 정리하라.

### Backend
기존 파일은 유지하되, 공통 쿼리 helper를 도입할 수 있다.

예시:

```text
src-tauri/src/commands/context_queries.rs
```

가능한 내용:
- conversation label 로딩
- branch parent resolution
- conversation recent messages 로딩
- cross-session summary용 공통 helper

### Frontend
types를 맞춰라.

예시:
- `Plan`
- `PlanSubtask`
- `Artifact`
- `Memo`

가능하면 naming을 Rust struct와 TS interface에서 최대한 맞춰라.

---

## 5. Step-by-Step Refactor Order

## Step 1. Split `ContextPanel.tsx`
목표:
- UI 동작 유지
- 하위 panel 컴포넌트로 분리

완료 조건:
- `ContextPanel.tsx`가 orchestration 역할만 남음
- 최소 3개 하위 panel로 분리
- `tsc --noEmit` 통과

---

## Step 2. Add frontend API wrappers
목표:
- plan/artifact/memo invoke를 `src/lib/api/*` 로 이동

완료 조건:
- 컴포넌트에서 직접 invoke 감소
- 최소 `plans.ts`, `artifacts.ts`, `memos.ts` 존재

---

## Step 3. Split `commands/agents.rs`
목표:
- context_pack / compression / trace_log 분리

완료 조건:
- `agents.rs`의 `tauri::command` entry 유지
- 핵심 helper 이동
- `cargo check` 통과

---

## Step 4. Split `roundtable.rs`
목표:
- prompt / executor / persist 분리

완료 조건:
- `roundtable.rs`는 얇은 orchestration만 남음
- 기존 동작 유지

---

## Step 5. Context domain cleanup
목표:
- plans / memos / artifacts / types naming과 공통 helper 정리

완료 조건:
- 타입/필드 naming 혼란 감소
- 공통 helper 최소 도입

---

## 6. Progress Reporting Process
각 step 후 반드시 멈추고 아래 형식으로 보고하라.

```md
## Refactor Step Report
- Step: [예: Step 3. Split commands/agents.rs]
- Status: completed | partial | blocked
- Done:
  - [완료한 것]
- Remaining:
  - [남은 것]
- Impact:
  - [무슨 파일이 얼마나 단순해졌는지]
- Verification:
  - [cargo check / tsc --noEmit 등]
- Next:
  - [다음 step]
```

매 2단계마다 아래 요약도 같이 갱신하라.

```md
## Refactor Master Progress
- Completed:
  - [완료된 steps]
- Current:
  - [현재 step]
- Remaining:
  - [남은 steps]
- Health:
  - on_track | caution | blocked
```

---

## 7. Validation Rules

각 단계 후 최소한 아래를 지켜라.

### Frontend step
- `tsc --noEmit`
- 필요한 경우 렌더 위치 설명

### Backend step
- `cargo check`
- 기존 command/flow 보존 이유 설명

### Mixed step
- 둘 다 가능한 범위에서 수행

검증 실패 시 다음 단계로 넘어가지 마라.

---

## 8. Stop Conditions
아래 경우는 반드시 멈추고 보고하라.

- 파일 분리가 DB/command 계약 변경으로 번질 때
- 공개 command 시그니처 변경이 필요할 때
- 프론트 패널 구조가 사실상 재설계로 넘어갈 때
- 리팩토링보다 기능 수정이 더 많이 섞이기 시작할 때

---

## 9. Success Criteria
이번 리팩토링이 성공했다는 뜻은 아래와 같다.

- `ContextPanel.tsx`가 읽기 쉬운 조립 파일이 된다
- `commands/agents.rs`가 과도한 만능 파일이 아니게 된다
- `roundtable.rs`가 prompt/execution/persist로 나뉜다
- invoke 문자열이 프론트 전역에 흩어지지 않는다
- 기존 기능은 그대로 작동한다

---

## 10. Immediate Instruction
지금 즉시 시작할 단계는 아래다.

### Execute First
`Step 1. Split ContextPanel.tsx`

이유:
- 프론트 비대화가 가장 빠르게 진행 중
- 이미 plan/artifact/memo/skill가 한 패널에 몰려 있음
- 기능 변경 없이도 분리 효과가 크다

작업 후 반드시 `Refactor Step Report`를 작성하고 멈춰라.

