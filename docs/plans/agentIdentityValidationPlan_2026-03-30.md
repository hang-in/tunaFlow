# Agent Identity Validation Plan

상태: 제안
작성: 2026-03-30

## 배경

`Agent Identity Framing` 구현으로 공통 `## Identity` 블록이 모든 엔진 경로에 주입되기 시작했다.

하지만 이 단계에서 중요한 것은 코드 삽입 여부보다, 실제 응답이 아래 원칙을 지키는지 확인하는 것이다.

- profile이 자기소개 1순위인지
- engine은 필요 시 2순위 정보인지
- persona가 자기 이름처럼 노출되지 않는지
- 잘못된 호칭을 짧고 일관되게 정정하는지

## 목표

Claude / Codex / Gemini / OpenCode가 identity 관련 질문에 대해 일관된 형식으로 답하는지 검증한다.

## 검증 질문

최소 3개 질문을 공통으로 사용한다.

1. `너 코덱스야?`
2. `지금 누가 답하고 있어?`
3. `엔진이 뭐야?`

## 기대 결과

### 1. profile 우선

- `현재 실행 중인 에이전트는 Architect Claude입니다.`
- profile 이름이 1순위로 나온다

### 2. engine 2순위

- `엔진은 claude-code입니다.`
- 필요할 때만 별도로 설명한다

### 3. persona는 이름이 아니다

- `Reviewer persona` 같은 표현이 자기 이름처럼 나오지 않는다

### 4. 혼합 표현 금지

아래 같은 표현은 실패로 본다.

- `Claude Code(opencode)`
- `저는 claude입니다`
- `저는 Reviewer입니다`

## 범위

1. chat send 경로 기준 검증
2. 가능하면 4개 엔진 모두 최소 1회 이상 확인
3. profile/engine 혼동 여부를 기록

## 비목표

- 새 identity 규칙 구현
- message meta UI 변경
- 다국어 정책 확장

## 성공 기준

- 3개 질문에 대해 4개 엔진 모두 profile/engine/persona를 구분한다
- 잘못된 모델 호칭을 일관되게 정정한다
- 혼합 표현이 재발하지 않는다

## 후속

실패 시 다음 두 방향 중 하나로 이어진다.

1. 공통 identity 블록 문구 조정
2. 특정 엔진 전용 보강 규칙 추가
