# tunaFlow Artifacts 메인 탭 승격 + Memo 보조 UX 계획

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-29
- 상태: 초안

## 목적

`Artifacts`와 `Memo`의 역할을 명확히 분리하고,
이에 맞게 UI 계층을 재정의한다.

핵심 방향은 아래와 같다.

- `Artifacts`는 메인 탭으로 승격
- `Memo`는 보조 기능으로 하향

## 역할 재정의

### Artifacts

정의:

- 저장/검색/재사용 가능한 **문서형 산출물 허브**

예:

- 설계안
- implementation brief
- handoff 문서
- architecture note
- decision record

즉 Artifacts는 단순 첨부나 즐겨찾기가 아니라,
**독립 문서로 재사용되는 결과물**이다.

### Memo

정의:

- 대화 중 "나중에 참고할 포인트"를 붙잡아 두는 **메시지 단위 메모**

예:

- 중요한 문장
- 참고 링크
- 짧은 요약
- 후속 작업 아이디어

즉 Memo는 독립 문서가 아니라,
원문 가까운 조각을 보존하는 장치다.

## 왜 이렇게 나누는가

현재 `Artifacts`와 `Memo`가 같은 수준의 패널처럼 보이면
둘의 차이가 흐려진다.

하지만 실제 역할은 다르다.

- Memo = 포스트잇
- Artifact = 보고서

따라서 UX 계층도 달라야 한다.

## 권장 UI 구조

### 메인 탭

- Chat
- Plan
- Artifacts
- Review
- Test

`Artifacts`를 메인 탭으로 승격하는 이유:

- 단순 참고 패널이 아니라 작업물 허브이기 때문
- Review/Test와 같은 작업 레벨이기 때문

### Memo UX

Memo는 메인 탭으로 두지 않는다.

권장 방식:

- 메시지 인라인 아이콘으로 저장
- 상단/상태바/툴바의 작은 아이콘으로 목록 진입
- 클릭 시 popover / drawer / modal 중 하나로 리스트 표시

## MVP 기준

### Phase 1. 정보 구조 정리

- 메인 탭에 `Artifacts` 추가
- `ContextPanel`/보조 패널에서 Artifacts의 중심 역할 제거
- `Memo`를 보조 진입점으로 재배치

### Phase 2. Memo list UX

- memo 아이콘 추가
- memo list view 제공
- 간단한 열람/삭제/이동 정도만 지원

### Phase 3. Artifact 수동 승격

- 메시지에서 "Artifact로 저장" 액션
- title/type/content 편집 가능한 최소 생성 흐름

### Phase 4. Artifact 자동 승격 후속

- agent role 기반 자동 문서 생성
- 예: architect → design brief

이 단계는 후순위다.

## 비목표

- Memo를 독립 문서 시스템으로 확장
- Artifacts를 실제 파일 트리와 합치기
- 자동 승격을 먼저 구현

## 완료 기준

1. `Artifacts`가 메인 탭에 존재
2. `Memo`는 보조 기능으로 진입
3. 둘의 역할 설명이 UI와 문서에서 일치
4. 사용자가 `Memo`와 `Artifact`를 혼동하지 않음

