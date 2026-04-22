# tunaFlow Artifact 상세 보기 계획

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-30
- 상태: 초안

## 목적

`Artifacts` 탭에서 필터/정렬까지 붙었으므로,
다음 단계는 저장된 artifact를 전체 본문까지 편하게 읽는 상세 보기 경험을 만드는 것이다.

이번 단계의 목표는 검색이나 export가 아니라
문서형 artifact를 다시 읽고 상태를 확인하는 기본 읽기 경험을 붙이는 데 있다.

## 현재 상태

이미 구현된 것:

- artifact 수동 승격
- Artifacts 탭
- type 필터
- 정렬
- 요약 목록

아직 없는 것:

- 긴 본문 전체 보기
- 목록과 문서 읽기 경험의 분리
- 카드 목록에서 바로 보기 어려운 세부 메타 확인

## 핵심 판단

상세 보기는 지금 필요하다.

이유:

1. artifact가 이제 실제로 생성되기 시작했다
2. 카드 목록만으로는 긴 문서를 읽기 어렵다
3. Artifacts를 “문서 허브”로 느끼려면 읽기 뷰가 있어야 한다

## 목표

### 1. 상세 보기 진입

방법:

- 카드 클릭
- 또는 `Open` 액션

### 2. 본문 전체 읽기

표시:

- title
- type
- status
- created date
- 전체 content

### 3. 상태 조작 연결

가능하면:

- detail view 안에서 status 변경

이번 단계에서는 읽기 중심이고,
간단한 상태 변경 정도까지만 붙여도 충분하다.

## 권장 UX

### 형태

가장 현실적인 선택:

- modal detail view

이유:

- 현재 레이아웃을 크게 안 흔든다
- 문서 본문 읽기에 충분한 공간을 준다

대안:

- inline expand
- split detail pane

하지만 이번 단계는 modal이 가장 작고 안전하다.

### 읽기 경험

- content는 markdown/plain text 그대로 읽을 수 있게
- 긴 경우 scroll
- 메타는 상단에 compact row

## 구현 범위

- `ArtifactsPanel`
- artifact detail modal
- 상태 변경 연결 (가능하면)

## 비목표

- full editor
- artifact 검색
- export/publish
- artifact-plan 자동 연결

## 완료 기준

1. artifact 카드에서 상세 보기 진입 가능
2. 긴 artifact 본문을 전체로 읽을 수 있음
3. type/status/date 같은 기본 메타를 detail에서 확인 가능

