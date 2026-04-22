# OPUS Master Implementation Plan for tunaFlow

## 0. Purpose
이 문서는 `tunaFlow` 로컬 코드베이스에서 AgentScope 분석 결과를 바탕으로, 구조를 깨지 않고 선별 기능을 단계적으로 고도화하기 위한 **총괄 실행 문서**다.

대상 프로젝트:
- `D:\privateProject\tunaFlow`

기준 분석 문서:
- `D:\00. Downloads\claude_ai_usage_widget\AGENTSCOPE_ANALYSIS_FOR_TUNAFLOW.md`

이 문서는 Opus에게 **한 번에 전달하는 단일 실행 문서**이지만, 실제 작업은 반드시 **단계별로 멈추고 보고하는 방식**으로 진행해야 한다.

---

## 1. Core Mission
너는 `tunaFlow`의 기존 구조를 유지한 채,
`AGENTSCOPE_ANALYSIS_FOR_TUNAFLOW.md`에서 도출한 고가치 기능을
**부분 도입 가능한 단위로 실제 구현**해야 한다.

목표는 다음 셋이다.

1. 이미 확인된 고가치 기능을 tunaFlow 구조에 맞게 선별 도입
2. 기존 Tauri / Rust / command layer / DB 구조 유지
3. 각 단계마다 실제 코드 수정, 검증, 짧은 진행 보고 수행

---

## 2. Absolute Constraints
아래는 절대 금지다.

- AgentScope 전체 프레임워크 이식
- tunaFlow를 AgentScope로 교체하자는 제안
- Tauri / Rust / command layer / DB 구조 무시한 재설계
- 기존 구조를 깨는 대규모 리팩터링
- 한 단계에서 너무 많은 기능을 동시에 건드리는 작업
- 실제 코드 확인 없이 추측 기반 수정
- 검증 없이 다음 단계 진행

허용되는 건 다음뿐이다.

- 작은 helper 추가
- 기존 command 패턴에 맞는 backend 확장
- 최소 migration
- 최소 UI 연결
- fallback 유지하는 보수적 개선

---

## 3. Working Rules

### 3.1 Execution Mode
반드시 아래 순서를 지켜라.

1. 현재 단계 목표 재확인
2. 관련 실제 파일 확인
3. 해당 단계 범위만 구현
4. 검증 수행
5. 진행 보고 작성
6. 다음 단계로 넘어갈지 판단

### 3.2 Stop Rule
아래 중 하나면 반드시 멈추고 보고하라.

- 현재 단계 범위를 넘는 변경이 필요해 보일 때
- DB 스키마 변경 영향이 예상보다 커질 때
- UI 계약이 깨질 가능성이 있을 때
- 기존 동작 보존이 확실하지 않을 때
- 검증이 실패할 때

### 3.3 Verification Rule
각 단계 후 최소한 아래 중 가능한 것을 수행하라.

- `cargo check`
- `tsc --noEmit`
- 기존 command 동작 확인
- SQLite 직접 조회
- 변경 함수의 실행 흐름 설명

검증 결과 없이 다음 단계로 넘어가면 안 된다.

---

## 4. Scope From Analysis Document
기준 문서에서 중요 도입 후보는 아래였다.

- memory compression
- multi-agent orchestration / message hub / workflow
- planning
- observability / tracing / OTel
- skills / tools abstraction / MCP
- evaluation
- human-in-the-loop
- A2A support

이 중 현재 tunaFlow에 가장 현실적인 도입 우선순위는 다음과 같다.

1. tracing / trace logging
2. memory compression
3. roundtable workflow structure
4. planning backend
5. planning UI
6. tools / MCP abstraction
7. evaluation harness
8. HITL / interruption 고도화
9. A2A는 장기 검토

---

## 5. Current Known Status
현재까지 이미 완료됐다고 간주하는 항목은 아래다.

### Completed
- `trace_log` write 활성화
- ContextPack 일부 섹션 대상 memory compression 실험 구현
- Roundtable 실행 구조 최소 리팩터링
- Plan state backend 추가
- Plans 조회 / 상태변경 UI 추가

### Incomplete
- Plan 생성 UI
- plan과 ContextPack의 약한 연결
- plan과 Artifact의 약한 연결
- Toolkit / MCP abstraction
- evaluation harness
- OTel exporter 연동
- HITL 강화

### Long-Term / Optional
- A2A support
- runtime / sandbox

---

## 6. Master Execution Order
아래 순서대로 진행하라. 한 번에 여러 단계를 묶지 마라.

### Phase 1. Plan Completion
1. Plan 생성 UI 추가
2. Plan과 ContextPack의 약한 연결
3. Plan과 Artifact의 약한 연결

### Phase 2. Tooling Layer
4. Skill / Tool capability registry 최소 일반화
5. MCP abstraction 최소 도입

### Phase 3. Quality / Observability
6. Evaluation harness 최소 백엔드
7. OTel exporter 연동 또는 trace schema 확장

### Phase 4. Interaction
8. HITL / interruption 고도화

### Phase 5. Long-Term
9. A2A suitability probe only

---

## 7. Step Definitions

## Step 1. Plan Creation UI
목표:
- Plans 패널 안에서 새 plan 생성
- title / description / expected outcome / subtasks 입력
- 생성 후 목록 즉시 갱신

수정 예상 범위:
- `src/components/tunaflow/ContextPanel.tsx`
- `src/types/index.ts`

완료 조건:
- UI에서 `create_plan` 호출 가능
- 생성 즉시 목록 반영
- 타입체크 통과

---

## Step 2. Plan to ContextPack Link
목표:
- 현재 active plan의 핵심 정보만 ContextPack에 약하게 주입
- plan 전체를 강제 삽입하지 말고 작은 섹션만 추가

수정 예상 범위:
- `src-tauri/src/commands/agents.rs`
- 필요 시 `commands/plans.rs`
- 필요 시 `guardrail.rs`

주입 내용 예시:
- active plan title
- current in-progress subtask
- next actionable step

완료 조건:
- plan이 있으면 작은 plan context section이 붙음
- plan이 없으면 기존 동작 유지
- fallback 유지

---

## Step 3. Plan to Artifact Link
목표:
- subtask 완료 시 artifact를 자동 생성하는 게 아니라
- 최소한 plan/subtask와 artifact를 약하게 연결할 수 있는 구조 또는 command 추가

수정 예상 범위:
- `src-tauri/src/commands/artifacts.rs`
- `src-tauri/src/db/schema.rs` if absolutely needed
- 프론트는 최소

완료 조건:
- subtask outcome과 artifact를 연결할 최소 경로 확보
- 기존 artifact 흐름 보존

---

## Step 4. Tool Capability Registry
목표:
- 현재 skill / local tool / future MCP tool이 붙을 수 있게 아주 작은 registry 개념 추가

수정 예상 범위:
- `src-tauri/src/commands/skills.rs`
- `src-tauri/src/commands/agents.rs`
- 필요 시 신규 helper 모듈

완료 조건:
- active capability grouping의 최소 틀 존재
- 기존 엔진 경로 유지

---

## Step 5. MCP Abstraction
목표:
- full MCP framework가 아니라
- stateful/stateless 또는 local/remote 구분이 가능한 최소 도입점 확보

완료 조건:
- MCP tool 등록을 넣을 자리가 코드상 생김
- 실제 기존 동작 불변

---

## Step 6. Evaluation Harness
목표:
- roundtable / planner 결과를 파일 또는 DB에 저장하고 비교 가능한 최소 evaluator 기반 확보

완료 조건:
- 최소 실행 결과 저장
- 반복 비교 가능한 구조

---

## Step 7. OTel Exporter Layer
목표:
- 현재 `trace_log`를 넘어 span/exporter 기반 observability로 연결

완료 조건:
- 최소 exporter 연결 또는 exporter-ready trace schema 도입

---

## Step 8. HITL Enhancement
목표:
- stop / interrupt / resume에 가까운 UX/backend 고도화

완료 조건:
- 현재 streaming UX와 충돌 없이 interrupt semantics 강화

---

## 8. Mandatory Progress Reporting Process
각 단계가 끝날 때마다 반드시 아래 형식으로 **짧고 간단하게** 보고하라.
이 보고는 문서/응답 어디든 좋지만, 형식은 유지해야 한다.

### Required Step Report Format

```md
## Step Report
- Step: [예: Step 1. Plan Creation UI]
- Status: completed | partial | blocked
- Done:
  - [이번 단계에서 끝난 것 2~5개]
- Remaining:
  - [이 단계 안에서 아직 남은 것]
- Risks:
  - [있으면 1~3개]
- Verification:
  - [예: cargo check ok / tsc --noEmit ok]
- Next:
  - [다음으로 진행할 step 하나]
```

### Additional Master Summary Rule
매 2단계마다 아래 요약도 한 번 같이 갱신하라.

```md
## Master Progress
- Completed Steps:
  - [완료된 step 목록]
- Current Step:
  - [현재 진행중인 step]
- Remaining Steps:
  - [남은 step 목록]
- Overall Health:
  - on_track | caution | blocked
```

---

## 9. Decision Rules Between Steps
다음 단계로 넘어가기 전에 반드시 확인하라.

- 현재 단계의 완료 조건을 충족했는가
- 검증 결과가 있는가
- 기존 동작이 유지되는가
- 다음 단계가 현재 단계 결과에 의존하는가

하나라도 아니면 다음 단계로 넘어가지 말고 `partial` 또는 `blocked`로 보고하라.

---

## 10. Output Requirements Per Step
매 단계 작업 후 아래 형식의 상세 보고를 유지하라.

```md
### A. Changes Made
### B. Files Modified
### C. Implementation Flow
### D. Verification
### E. Remaining Risks
### F. Step Report
```

간단한 단계라도 `Step Report`는 반드시 포함해야 한다.

---

## 11. What Good Execution Looks Like
좋은 실행은 아래와 같다.

- 단계별 범위를 넘지 않는다
- 검증하고 멈춘다
- 완료/남음/리스크를 짧게 정리한다
- 기존 구조를 보존한다
- 다음 단계 진입 시점이 명확하다

나쁜 실행은 아래와 같다.

- 한 번에 planning, MCP, evaluation, UI를 같이 건드린다
- command layer를 다시 짠다
- DB 구조를 크게 흔든다
- 검증 없이 다음 단계로 넘어간다

---

## 12. Immediate Instruction
현재 기준 즉시 다음 단계는 아래다.

### Execute Next
`Step 1. Plan Creation UI`

진행 방식:
- 실제 코드 확인
- 최소 구현
- 타입체크
- 상세 보고
- Step Report 작성

그 다음에만 `Step 2`로 이동하라.

