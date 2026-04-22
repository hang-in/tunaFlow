# tunaFlow Artifact Provenance / Workflow Linkage 계획

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-30
- 상태: 초안

## 목적

`Artifacts`가 이제 생성되고 읽히기 시작했으므로,
다음 단계는 각 artifact가 어디서 왔고 어디로 이어질 수 있는지 더 명확히 보여주는 것이다.

이번 단계의 목표는 artifact를 고립된 문서가 아니라
conversation / branch / plan / subtask 흐름 안의 작업물로 보이게 만드는 데 있다.

## 현재 상태

이미 구현된 것:

- artifact 수동 승격
- Artifacts 탭 필터/정렬
- artifact detail modal
- artifact status 변경

아직 부족한 것:

- 어떤 메시지/대화에서 왔는지
- 어떤 branch/RT 맥락에서 생겼는지
- plan/subtask와 연결되었는지
- forward가 어떤 워크플로우 위치를 의미하는지

## 핵심 판단

이제 필요한 것은 새 artifact 기능보다
artifact의 출처와 연결 관계를 보여주는 것이다.

이유:

1. artifact가 많아질수록 “무엇인지”보다 “어디서 왔는지”가 중요해진다
2. Artifacts를 문서 허브로 만들려면 provenance가 필요하다
3. Plan/Review/Test와의 경계도 연결 관계를 보여줘야 더 명확해진다

## 목표

### 1. provenance 표시

최소 표시 후보:

- source conversation
- branch / RT 여부
- created from message

이번 단계에서 메시지 ID를 직접 노출할 필요는 없지만,
`Main Chat`, `Branch`, `RT` 같은 출처 수준은 보여주는 게 좋다.

### 2. workflow linkage 표시

가능하면:

- linked subtask
- linked plan
- review/test 성격 문서인지

이미 있는 관계가 있으면 UI에서 드러내고,
없으면 과장하지 않는다.

### 3. forward 의미 보강

현재 `→ Claude` 같은 액션이 있더라도,
artifact가 다음 워크플로우로 어떻게 쓰이는지 더 잘 보여줘야 한다.

예:

- follow-up source
- plan input
- review input

이번 단계에서는 적어도 source로 재사용된다는 점이 읽히면 충분하다.

## 권장 UX

### 목록/카드

- compact provenance line
- 예: `Main Chat · 2026-03-30`

### detail modal

메타 섹션에:

- source conversation / branch
- linked subtask
- linked workflow hint

## 구현 범위

- `ArtifactsPanel`
- artifact detail modal
- 이미 있는 relation data 재사용
- 필요한 최소 display metadata 연결

## 비목표

- artifact-plan 자동 생성
- artifact graph view
- full provenance audit trail
- source jump deep-link 대규모 구현

## 완료 기준

1. artifact의 출처가 이전보다 더 잘 보인다
2. plan/subtask/workflow 연결이 있으면 UI에서 읽힌다
3. artifact가 고립된 문서가 아니라 작업 흐름의 일부처럼 느껴진다

