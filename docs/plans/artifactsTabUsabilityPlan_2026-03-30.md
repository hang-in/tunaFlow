# tunaFlow Artifacts 탭 사용성 개선 계획

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-30
- 상태: 초안

## 목적

`Save as Artifact` 수동 승격이 붙었으므로,
다음 단계는 `Artifacts` 탭을 실제 문서 허브처럼 더 쉽게 탐색하고 관리하게 만드는 것이다.

이번 단계의 초점은 새 artifact 생성이 아니라
이미 저장된 artifact를 다시 찾고, 읽고, 상태별로 정리하는 경험을 개선하는 데 있다.

## 현재 상태

이미 구현된 것:

- assistant 메시지 → artifact 수동 승격
- RT 카드 → artifact 수동 승격
- Artifacts 메인 탭
- artifact status 변경

아직 부족한 것:

- type/status별 빠른 필터
- 최근성/제목 기준 정렬
- 긴 content를 읽기 위한 상세 보기
- 관련 대화/브랜치/플랜 맥락 재확인

## 핵심 판단

지금은 검색보다
`필터 + 정렬 + 상세 보기`가 먼저다.

이유:

1. artifact 수가 아직 아주 많지 않을 가능성이 크다
2. type/status별로만 나눠도 사용성이 크게 오른다
3. 상세 보기 없이 긴 artifact 본문은 탭에서 읽기 어렵다

## 목표

### 1. 빠른 필터

최소 필터:

- All
- Notes
- Specs
- Plans
- Review/Test 관련

또는:

- type
- status

중 하나라도 명확하면 충분하다.

### 2. 정렬

최소 정렬:

- 최신순
- 상태순 또는 type순

기본은 최신순이 자연스럽다.

### 3. 상세 보기

artifact 카드에서 일부만 보이고,
클릭 시 전체 내용을 더 편하게 읽을 수 있어야 한다.

형태:

- inline expand
- modal
- side detail

이번 단계에서는 modal 또는 expand가 현실적이다.

### 4. 맥락 표시

최소한 아래 중 일부는 보여주는 것이 좋다.

- created at
- type
- status
- source conversation

## 구현 범위

- `ArtifactsPanel`
- 필요 시 artifact detail modal
- filter/sort local state

## 비목표

- full-text search
- artifact 자동 승격
- artifact-plan 자동 연결
- export/publish

## 완료 기준

1. Artifacts 탭에서 저장된 artifact를 더 쉽게 찾을 수 있다
2. type/status 기준으로 빠르게 걸러볼 수 있다
3. 긴 artifact를 상세하게 읽을 수 있다

