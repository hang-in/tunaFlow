# tunaFlow RT 생성 설정 연결 계획

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-26 21:35 KST

## 목적

`CreateRoundtableDialog`를 통해 RT 생성 UX는 이미 도입되었지만,
현재는 생성 시 고른 participant / engine / model / mode 설정이
실제 첫 `roundtable_run` 실행으로 완전히 연결되어 있지 않다.

이 문서는:

1. 현재 구현 상태
2. 왜 지금 상태가 반쪽인지
3. 어떤 저장 경로와 연결 방식이 맞는지
4. 단계별 구현 순서

를 정리한다.

## 현재 상태

이미 구현된 것:

- Sidebar `Roundtables` 섹션에서 `+` 버튼으로 RT 생성 진입
- `CreateRoundtableDialog`에서:
  - 제목 입력
  - mode 선택
  - participant 추가/삭제
  - participant별 이름/engine/model 선택
- `createConversation(mode: "roundtable")`
- 생성 직후 `selectConversation(rtId)`로 RT 화면 진입

아직 부족한 것:

- dialog에서 고른 participant 구성이 실제 첫 `roundtable_run`에 반영되지 않음
- 현재는 `sessionStorage`에만 저장되고, `NewMessageInput`이 이를 읽어 적용하지 않음
- 기존 `ROUNDTABLE_PARTICIPANTS` 기본값 경로와 새 dialog 경로가 공존함

## 핵심 판단

현재 구현은:

- **RT 생성 UX는 완료**
- **RT 실행 설정 연결은 미완**

즉 사용자는 “설정했다”고 느끼지만,
실제 첫 실행은 아직 기존 기본 participant 경로를 타기 쉽다.

이 상태를 오래 두면:

1. 사용자 기대와 실행 결과가 어긋남
2. RT 생성 UX의 신뢰도가 떨어짐
3. 같은 RT conversation이 어떤 participant 구성을 갖는지 불명확해짐

## 목표 상태

RT conversation 하나는 최소한 아래 설정을 가질 수 있어야 한다.

- `label`
- `mode`
- `participants[]`
  - `name`
  - `engine`
  - `model`

그리고 이 설정은:

1. RT 생성 직후 저장됨
2. RT conversation 진입 후 `NewMessageInput` 또는 RT 실행 경로에서 읽힘
3. 첫 `roundtable_run` 기본 participant 값으로 사용됨
4. 필요하면 이후 수정 가능하게 확장 가능

## 저장 방식 후보

### A. sessionStorage 브릿지

장점:

- 구현이 빠름
- 1차 연결에는 간단함

단점:

- conversation-level source of truth가 아님
- 새로고침/복원/멀티윈도우/재진입에 약함
- 장기적으로 유지하기 좋지 않음

판단:

- 임시 1차 패치로는 가능
- 최종 저장 구조로는 부적합

### B. conversation metadata로 저장

예:

- conversations 테이블 컬럼 추가
- 또는 별도 settings/artifacts/memos 형태

장점:

- RT conversation 자체의 설정이 됨
- 재진입/복원/추적에 강함
- 향후 reviewer thread / eval / rerun에도 활용 가능

단점:

- DB/command 확장 필요

판단:

- 제품 방향상 가장 맞음

### C. artifact/memo에 저장

예:

- `task-brief`처럼 RT config artifact 생성

장점:

- 기존 저장 모델 재사용 가능

단점:

- 실행 시 기본 설정을 빠르게 읽는 용도로는 부자연스러움
- “설정”보다 “기록물”에 가까움

판단:

- 기록 보조로는 가능
- 1차 SSOT로는 비추천

## 권장 방향

### 1차

빠른 연결:

- 생성 시 participant/mode 구성을 RT conversation id 기준으로 저장
- `NewMessageInput`이 현재 RT conversation 선택 시 이를 읽어 기본 participant로 사용

저장 위치는 아래 둘 중 하나:

1. RT conversation 전용 lightweight persisted settings
2. 최소 임시 구현으로 `sessionStorage`를 쓰되, key를 `conversationId` 기준으로 엄밀히 연결

### 2차

정식화:

- RT config를 DB 또는 persisted store에 conversation-level metadata로 승격
- rerun / follow-up / eval에서 재사용 가능하게 함

## 권장 구현 순서

1. RT config 타입 정의
2. RT conversation 생성 시 config 저장
3. `NewMessageInput`이 RT conversation 선택 시 config 읽기
4. 첫 `sendRoundtable`이 기본 participant 대신 config participant를 사용
5. fallback:
   - config 없으면 기존 `ROUNDTABLE_PARTICIPANTS`

## 범위 제한

이번 단계에서는 하지 말 것:

- RT 평가 UI 동시 구현
- RT settings 편집 화면 전면 추가
- reviewer lane 통합
- docs 전체 대개편

## 기대 효과

이 작업이 끝나면:

1. RT 생성 UX와 실제 실행이 일치함
2. participant/model 설정이 장식이 아니라 실효성이 생김
3. 향후 RT preset / rerun / eval 기능의 기반이 생김

