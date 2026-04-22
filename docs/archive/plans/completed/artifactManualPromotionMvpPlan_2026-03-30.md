# tunaFlow Artifact 수동 승격 MVP 계획

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-30
- 상태: 초안

## 목적

채팅 결과를 단순 메시지로 끝내지 않고,
사용자가 의미 있는 assistant 결과를 `Artifact`로 수동 승격할 수 있게 만든다.

이번 단계의 목표는 자동 파이프라인이 아니라
작고 명확한 `Save as Artifact` 흐름을 만드는 것이다.

## 현재 상태

이미 있는 것:

- `Artifacts` 메인 탭
- artifact CRUD
- artifact type / status 모델
- `Review` / `Test`는 artifact 필터 뷰로 동작

아직 없는 것:

- 메시지에서 artifact를 직접 만드는 진입점
- “이 응답을 문서형 산출물로 저장”하는 명시적 UX
- `Artifacts`가 실제로 채팅 결과와 연결되는 수동 승격 흐름

## 핵심 판단

자동 승격보다 수동 승격이 먼저다.

이유:

1. 현재 공통 자동 artifact pipeline은 없다
2. 어떤 결과를 artifact로 볼지 사용자 판단이 아직 중요하다
3. 작은 수동 승격만 있어도 Artifacts 탭의 제품 가치가 바로 올라간다

## 목표

### 1. 메시지 액션에서 Artifact 저장 진입

후보:

- `Save as Artifact`
- `Promote to Artifact`

적용 대상:

- 기본적으로 assistant 메시지

### 2. 최소 생성 흐름

필수 입력:

- title
- type
- content (기본은 메시지 본문)

type 후보 예:

- `design-brief`
- `implementation-brief`
- `handoff-note`
- `decision-record`

현재 artifact type 시스템과 충돌하지 않게 작은 집합으로 시작한다.

### 3. 저장 후 Artifacts 탭에서 바로 확인 가능

핵심:

- 사용자가 저장 결과를 즉시 확인할 수 있어야 한다
- 수동 승격이 실제 워크플로처럼 느껴져야 한다

## 권장 UX

### 메시지 액션

- assistant 메시지 hover/action row에 버튼 추가
- 과한 강조보다 secondary action 정도가 적절

### 생성 UI

작은 modal 또는 popover:

- title
- artifact type select
- preview or editable content
- save / cancel

이번 단계에서는 compact modal이 가장 현실적이다.

## 범위

- `MessageActions`
- artifact 생성 modal/sheet
- store `createArtifact` 연결
- Artifacts 탭 반영

## 비목표

- agent role 기반 자동 승격
- review/test artifact 자동 분류
- artifact editor 대규모 확장
- 파일 export

## 완료 기준

1. assistant 메시지에서 artifact 수동 승격이 가능하다
2. title/type/content를 최소한으로 조정해 저장할 수 있다
3. 저장 후 Artifacts 탭에서 바로 확인 가능하다

