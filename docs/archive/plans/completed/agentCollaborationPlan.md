# TUNAFLOW_AGENT_COLLABORATION_IMPLEMENTATION_PLAN.md

- Author: OpenAI Codex
- Created At: 2026-03-26 07:58 KST
- Project: `D:\privateProject\tunaFlow`
- Purpose: tunaFlow에 바로 붙일 수 있는 에이전트 간 소통 강화 아이디어 5개를 한 번에 무리하게 넣지 않고, 단계적으로 적용하기 위한 실행 계획

---

## 문서 목적

이 문서는 아래 5개 아이디어를 tunaFlow에 적용할 때,

1. Roundtable 결과를 `Shared Brief`로 자동 저장
2. ContextPack에 `Recent Agent Findings` 섹션 추가
3. Artifact를 handoff 문서처럼 활용
4. Plan subtask에 agent ownership 개념 추가
5. Follow-up Agent 액션 추가

한 번에 전부 밀어 넣지 않고, **리스크를 통제하면서 순차 적용**하기 위한 계획서다.

핵심 목적은 AgentScope를 흉내 내는 것이 아니라,
`tunaFlow` 안에서 이미 존재하는 `Roundtable / ContextPack / Plan / Artifact / UI`를 이용해
에이전트 간 전달 품질을 높이는 것이다.

---

## 왜 한 번에 다 적용하지 않는가

기술적으로는 가능할 수 있다. 하지만 현재 tunaFlow 구조에서는 아래 리스크가 크다.

- ContextPack이 과도하게 무거워질 수 있음
- Roundtable 결과물과 Artifact handoff 규칙이 동시에 바뀌면 회귀 원인 추적이 어려움
- Plan ownership을 DB/UI/backend에 동시에 넣으면 수정 범위가 급격히 커짐
- Follow-up UX까지 동시에 넣으면 사용 흐름 변화가 커져 실제 개선 효과를 분리해 보기 어려움

따라서 가장 현실적인 방식은:

- 설계는 한 번에 정리
- 구현은 3개 묶음으로 분할
- 각 묶음마다 검증 후 다음 단계로 진행

---

## 대상 아이디어 5개 요약

### 1. Shared Brief

Roundtable 종료 후 transcript 전체 대신,

- 합의점
- 주요 쟁점
- 반박 포인트
- 다음 액션

을 요약한 짧은 공유 문서를 자동 생성한다.

### 2. Recent Agent Findings

ContextPack 안에 최근 다른 agent가 남긴 핵심 결론을 요약 섹션으로 넣는다.

예:

- Claude: 구조적 리스크
- Codex: 구현 포인트
- Gemini: 대안 비교

### 3. Artifact Handoff

에이전트 작업 결과를 artifact로 남기고,
다음 agent 호출 시 해당 artifact를 ContextPack 또는 follow-up 입력에 포함한다.

### 4. Plan Ownership

Plan subtask에

- `owner_agent`
- 또는 `last_updated_by`

같은 최소 ownership 정보를 추가해 역할 분담 흔적을 남긴다.

### 5. Follow-up Agent UX

메시지나 artifact에서

- `Ask Claude to refine`
- `Ask Codex to implement`
- `Ask Gemini to critique`

같은 액션을 눌러, 관련 context를 묶어 다음 agent에게 넘기는 UX를 만든다.

---

## 전체 적용 순서

### Phase 1. Shared Context 강화

포함:

- Shared Brief
- Recent Agent Findings

### Phase 2. Handoff 구조 강화

포함:

- Artifact Handoff
- Plan Ownership

### Phase 3. UX 연결

포함:

- Follow-up Agent 액션

---

## Phase 1. Shared Context 강화

## 목표

다른 agent가 남긴 내용을 다음 agent가 **짧고 안정적으로 재사용**할 수 있게 한다.

## 포함 기능

### A. Roundtable → Shared Brief 자동 생성

Roundtable 종료 시 transcript 전체를 다시 쓰지 않고,
짧은 요약 문서를 memo 또는 artifact로 저장한다.

권장 저장 형태:

- type: `roundtable_brief`
- source: `roundtable`
- 내용:
  - summary
  - agreements
  - disagreements
  - next_steps

### B. ContextPack → Recent Agent Findings

ContextPack 조립 시 최근 agent 결과 중 중요한 요약만 섹션으로 추가한다.

권장 우선순위:

- 가장 최근 roundtable brief
- 최근 agent 관련 artifact
- 최근 agent-tagged memo

## 권장 구현 범위

- 기존 schema를 크게 바꾸지 않고 시작
- 가능하면 memo/artifact의 existing type/tag를 활용
- ContextPack에는 짧은 최대 길이 제한 적용

## 기대 효과

- 다음 agent가 긴 transcript 전체를 읽지 않아도 됨
- branch/session이 달라도 최근 결론을 공유 가능
- Roundtable이 일회성 토론이 아니라 handoff 단계가 됨

## 완료 조건

- roundtable 종료 후 shared brief 저장
- agent 호출 시 brief/findings 섹션이 ContextPack에 들어감
- 길이 초과 시 guardrail/truncate/compression 규칙 유지

## 리스크

- 요약 품질이 낮으면 정보 손실 가능
- findings를 너무 많이 넣으면 ContextPack 비대화

---

## Phase 2. Handoff 구조 강화

## 목표

에이전트 결과를 대화 로그가 아니라 **구조화된 작업 산출물**로 넘길 수 있게 한다.

## 포함 기능

### A. Artifact Handoff 강화

artifact를 단순 저장물이 아니라
다음 agent에게 넘기는 공식 handoff 문서처럼 활용한다.

권장 artifact 예시:

- `analysis`
- `implementation_proposal`
- `risk_review`
- `decision_note`

다음 agent 호출 시:

- 선택된 artifact
- 최근 관련 artifact

를 ContextPack 또는 follow-up payload에 포함한다.

### B. Plan Ownership

subtask 단위로 누가 맡았는지, 누가 마지막으로 갱신했는지 기록한다.

최소 필드 후보:

- `owner_agent`
- `last_updated_by`

초기에는 별도 큰 planner UI를 만들지 말고,

- backend 저장
- 간단한 UI badge

정도로 시작한다.

## 기대 효과

- 에이전트 간 handoff가 대화 로그 의존에서 벗어남
- plan과 artifact가 실제 협업 상태판 역할을 하게 됨
- 나중에 평가/eval과 연결하기 쉬워짐

## 완료 조건

- artifact를 follow-up 입력에 약하게 연결할 수 있음
- subtask에서 최소 ownership 정보 조회 가능
- UI에 최소 ownership 표시 또는 추적 가능

## 리스크

- ownership을 너무 무겁게 만들면 planner가 복잡해짐
- artifact 자동 포함 규칙이 과하면 context noise 증가

---

## Phase 3. UX 연결

## 목표

사용자가 복붙 없이도 한 agent의 결과를 다른 agent에게 넘길 수 있게 한다.

## 포함 기능

### Follow-up Agent 액션

메시지, artifact, plan, brief 중 하나를 기준으로:

- refine
- implement
- critique
- summarize

같은 후속 액션을 다른 agent에게 넘길 수 있게 한다.

예:

- Claude 결과 → Codex에게 구현 요청
- Codex 결과 → Gemini에게 리스크 검토 요청
- Roundtable brief → Claude에게 최종 정리 요청

## 최소 구현 방향

- 메시지 또는 artifact 옆에 follow-up 액션 추가
- 클릭 시:
  - base content
  - related artifact
  - active plan
  - recent findings
  를 조합해 다음 agent 입력으로 전달

## 기대 효과

- “에이전트 협업”이 실제 UI 플로우로 드러남
- 사용자가 수동 복붙 없이 chain 작업 가능

## 완료 조건

- 최소 1개 엔트리 포인트에서 follow-up 실행 가능
- context 전달 규칙이 명시적으로 정의됨
- 기존 chat/branch UX를 깨지 않음

## 리스크

- 너무 일찍 도입하면 context rule이 불안정할 수 있음
- 앞선 Phase가 약하면 UX만 생기고 품질은 낮을 수 있음

---

## 권장 세부 우선순위

가장 추천하는 실제 순서는 아래와 같다.

1. Shared Brief 자동 생성
2. Recent Agent Findings 섹션 추가
3. Artifact Handoff 연결
4. Plan Ownership 추가
5. Follow-up Agent 액션 추가

즉, 먼저 “정보 품질”을 높이고,
그다음 “handoff 구조”를 만들고,
마지막에 “UX”를 붙이는 순서다.

---

## 각 Phase별 검증 포인트

## Phase 1 검증

- roundtable 종료 후 brief가 저장되는가
- 다음 agent ContextPack에 brief/findings가 들어가는가
- ContextPack이 과도하게 길어지지 않는가

## Phase 2 검증

- artifact를 기준으로 handoff가 가능한가
- subtask ownership이 저장/조회되는가
- 기존 plan/artifact UI를 깨지 않는가

## Phase 3 검증

- follow-up 버튼/액션이 실제로 다음 agent 호출로 이어지는가
- 복붙 없이 handoff가 가능한가
- 사용 흐름이 과도하게 복잡해지지 않는가

---

## 단계별 보고 형식

각 Phase 또는 세부 작업이 끝날 때 아래 형식으로 보고한다.

```md
## Agent Collaboration Step Report

- Step:
- Status: completed / partial / blocked
- Done:
- Impact:
- Verification:
- Remaining:
- Risks:
- Next:
```

예시:

```md
## Agent Collaboration Step Report

- Step: Phase 1-A Shared Brief
- Status: completed
- Done: roundtable 종료 후 roundtable_brief memo 자동 저장
- Impact: 다음 agent가 transcript 대신 요약본을 읽을 수 있음
- Verification: cargo check / tsc / 저장 결과 확인
- Remaining: ContextPack findings 섹션 추가
- Risks: 요약 품질 튜닝 필요
- Next: Phase 1-B 진행
```

---

## 최종 완료 기준

이 계획이 “적용 완료”로 간주되는 상태:

1. Roundtable 결과가 brief 형태로 재사용 가능
2. ContextPack에 최근 다른 agent의 finding이 들어감
3. Artifact가 handoff 매체로 실제 활용됨
4. Plan subtask에 agent ownership 흔적이 남음
5. Follow-up Agent 액션으로 결과를 다른 agent에게 넘길 수 있음

---

## 결론

이 5개 아이디어는 한 번에 전부 넣는 것보다,

- Phase 1: Shared Context
- Phase 2: Handoff Structure
- Phase 3: UX

순으로 나눠 적용하는 것이 tunaFlow에 가장 잘 맞는다.

이 방식이면 기존 구조를 깨지 않으면서도,
세션이 달라도 “다른 agent가 무엇을 했는지 이어받는 느낌”을 강하게 만들 수 있다.
