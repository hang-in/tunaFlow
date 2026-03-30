# Agent Identity Framing Plan

상태: 제안
작성: 2026-03-30

## 문제

현재 tunaFlow는 `agent profile`, `engine`, `persona` 메타를 따로 가지고 있지만, 자연어 응답에서는 이 셋이 섞여 설명되는 경우가 있다.

대표 증상:
- 사용자가 `코덱스`라고 부르면 OpenCode가 자신을 정정하면서도 다시 `Claude Code(opencode)`처럼 흔들림
- profile 이름과 engine 이름이 번갈아 자기 이름처럼 쓰임
- persona가 자기 이름처럼 노출되거나, 반대로 전혀 설명되지 않음

이 문제는 기능 버그보다 **정체성 framing 규칙 부재**에 가깝다. 멀티에이전트 제품에서는 신뢰와 조작 가능성을 떨어뜨린다.

## 목표

에이전트가 자기 정체성을 설명할 때 아래 세 층을 일관되게 구분하도록 만든다.

1. `agent profile`
2. `engine`
3. `persona`

## 핵심 원칙

### 1. 사용자에게 보이는 1급 개념은 profile이다

- 사용자는 기본적으로 `Architect Claude`, `Reviewer Codex` 같은 profile을 선택한다
- 따라서 자기소개도 profile 기준으로 시작해야 한다

### 2. engine은 실행자 정보다

- `claude`, `codex`, `gemini`, `opencode`는 실행 엔진이다
- 필요할 때만 두 번째 정보로 설명한다

### 3. persona는 profile의 구성요소다

- persona는 role/policy 블록이다
- 자기 이름처럼 답하면 안 된다

## 권장 응답 규칙

### 기본 형식

- `현재 실행 중인 에이전트는 {profile_name}입니다.`
- 필요하면 이어서:
  - `엔진은 {engine}입니다.`
  - `persona는 {persona_name}입니다.`

### 사용자가 다른 이름으로 부를 때

- 틀린 호칭은 짧게 정정한다
- 정정할 때도 profile과 engine을 분리한다

예:
- `현재 실행 중인 에이전트는 Architect Claude입니다. 엔진은 claude입니다.`
- `저는 Codex가 아니라 OpenCode 엔진으로 실행 중인 General 프로필입니다.`

### 금지 형식

- `저는 Claude Code(opencode)입니다`
- `저는 Reviewer입니다` 같은 persona-only 자기소개
- `저는 claude입니다`처럼 engine만 자기 이름처럼 답하는 표현

## 구현 범위

1. runtime prompt assembly에서 self-identification 규칙을 공통 텍스트로 반영
2. 4개 엔진 모두 같은 규칙을 따르도록 공통 경로 우선 적용
3. 사용자가 identity를 물었을 때의 답변 품질을 우선 개선

## 비목표

- 새 agent profile 추가
- profile naming 체계 전체 재설계
- message meta UI 개편
- identity를 강제로 항상 답하게 만드는 것

## 우선 적용 지점

- 공통 prompt assembly 또는 engine 공통 section
- persona/policy 조립 계층
- 필요 시 follow-up/handoff prompt 경로

## 성공 기준

- 사용자가 `너 코덱스야?`, `누가 답하고 있어?` 같은 질문을 해도 profile/engine/persona가 섞이지 않는다
- 4개 엔진 모두 같은 규칙을 따른다
- profile이 자기소개 1순위로 유지된다

## 후속

이 단계가 끝나면 다음에 볼 수 있는 후속은:
- MessageMeta/Trace에 applied profile/engine/persona 표기 정합성 보강
- follow-up/handoff에서도 source agent identity를 더 명확히 남기는 작업
