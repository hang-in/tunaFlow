# Roundtable Participant Role / Blind UI Plan

상태: 중요 / 제안
작성: 2026-03-30

## 배경

Roundtable backend에는 이제 participant별로:

- `blind`
- `role`
- `max_tokens`

를 표현할 수 있는 구조가 들어갔다.

즉 실행 모델은:
- blind verifier
- role-based output cap

을 이미 지원한다.

하지만 현재 이 값들은 실질적으로 프론트에서 쉽게 설정할 수 있는 표면이 약하다.

따라서 다음 단계는 RT 참가자 설정 UI에서 이 실행 속성을 실제로 다룰 수 있게 하는 것이다.

## 목표

`CreateRoundtableDialog`와 RT participant 설정 흐름에서:
- participant role 선택
- blind verifier 토글
- 필요 시 max token override

를 설정할 수 있게 한다.

핵심은 backend capability를 실제 워크플로우로 연결하는 것이다.

## 왜 필요한가

### 1. 지금 상태는 기능이 있으나 사용성이 약하다

코드 수준에서는:
- `blind: true`
- `role: "verifier"`
- `maxTokens: 800`

같은 값이 가능하지만,
사용자가 쉽게 고르지 못하면 실사용 기능이라 보기 어렵다.

### 2. RT 설계 의도를 UI가 반영해야 한다

tunaFlow의 RT는 단순 multi-send가 아니라:
- proposer
- reviewer
- verifier
- synthesizer

같은 역할 차이를 전제로 더 정교한 토론 구조로 가고 있다.

그렇다면 participant UI도 단순 이름/엔진/모델만이 아니라,
이 역할 구조를 최소한 드러내야 한다.

### 3. blind verifier는 특히 명시적이어야 한다

blind verifier는 기본 participant와 다르게 동작한다.

따라서:
- 어떤 participant가 blind인지
- 어떤 participant가 verifier인지

가 설정과 UI에서 보이지 않으면 운용이 어렵다.

## 이번 단계에서 할 것

### 1. participant role 선택 UI

최소 역할:
- proposer
- reviewer
- verifier
- synthesizer

정도면 충분하다.

필요하면 미지정 상태도 유지한다.

### 2. blind verifier 토글

participant 단위로:
- `blind` on/off

를 고를 수 있게 한다.

권장 UX:
- verifier role 선택 시 blind 추천
- 하지만 자동 강제는 아직 하지 않는다

### 3. max token override 최소 노출

기본은 role별 default cap을 사용하고,
고급 사용자만 직접 override할 수 있게 하는 쪽이 맞다.

이번 단계에서는:
- 수치 입력을 숨겨진 advanced row로 두거나
- 최소 입력 필드만 둘 수 있다.

### 4. 표시 보강

RT 뷰 또는 participant chip에서:
- role
- blind 여부

를 최소 수준으로 읽을 수 있으면 좋다.

## 이번 단계에서 하지 않을 것

- lead decomposition
- verifier scoring
- role별 prompt 대규모 재설계
- hard max token enforcement

## 구현 원칙

- 설정 UI는 가볍게
- blind / role은 보여야 하지만 과도한 전문가용 패널처럼 만들지 말 것
- default cap은 role 기반으로 자동 적용하고, manual override는 보조 기능으로 둘 것

## 성공 기준

- RT 생성 시 participant role을 고를 수 있다
- blind verifier를 UI에서 설정할 수 있다
- 설정값이 실제 RT config와 실행에 반영된다
- role/blind 상태를 최소한으로 다시 확인할 수 있다

## 후속

이 단계 다음은:

1. role/blind 적용 검증
2. soft cap 준수율 확인
3. 그 후 lead decomposition 별도 milestone

순으로 이어진다.
