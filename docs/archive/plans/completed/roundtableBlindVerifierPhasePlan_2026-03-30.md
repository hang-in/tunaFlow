# Roundtable Blind Verifier Phase Plan

상태: 중요 / 제안
작성: 2026-03-30

## 배경

Roundtable Deliberative completion-order 개선으로 reduce 단계 straggler 병목은 줄였다.

하지만 멀티에이전트 토론의 다음 핵심 리스크는 `sycophancy`다.

즉:
- 마지막 participant가
- 앞선 participant들의 답을 먼저 보고
- 독립 검증 대신 후행 동조를 할 수 있다.

현재 tunaFlow RT는:
- Sequential에서는 뒤 participant가 앞 응답을 본다
- Deliberative에서도 prior transcript는 동일하게 본다

따라서 “cold verifier” 또는 “blind verifier” 역할을 코드 레벨에서 강제하지 않는다.

## 목표

Roundtable에 선택적 `blind verifier` 단계를 추가한다.

핵심은:
- 특정 participant가 다른 에이전트 응답을 보기 전에 먼저 독립 판단을 내리게 하거나
- 최소한 검증 단계에서는 prior/current transcript를 보지 않는 별도 실행 옵션을 두는 것이다.

## 왜 필요한가

### 1. 현재 구조는 verifier isolation이 없다

`build_round_prompt()`는 transcript/current_round를 넣어 같은 형식으로 participant prompt를 만든다.

즉 verifier라고 불러도:
- 실제로는 다른 응답의 영향을 받을 수 있다.

### 2. 코드 레벨 강제가 있어야 UX 의도와 실행이 일치한다

현재는 “Opus가 cold verifier 역할” 같은 운영 규칙을 둘 수는 있지만,
코드가 이를 강제하지 않으면 설정/참가자 순서에 따라 쉽게 깨진다.

### 3. lead decomposition보다 범위가 작고 효과가 직접적이다

blind verifier는:
- execution mode 확장 수준으로 처리 가능하고
- sycophancy 완화에 즉시 기여한다.

반면 lead decomposition은 별도 orchestration milestone이다.

## 이번 단계에서 할 것

### 1. verifier blindness를 표현할 최소 설정 추가

예시:
- participant 옵션으로 `blind: true`
- 또는 RT mode 하위 옵션으로 마지막 participant를 blind verifier로 취급

어떤 방식이든:
- 현재 코드와 충돌이 적고
- participant 단위로 제어 가능해야 한다.

### 2. blind participant prompt 구성 변경

blind verifier는:
- prior transcript 미포함
- current round peer refs 미포함

상태에서 topic만 받고 독립 판단하게 한다.

필요하면:
- "다른 참가자 응답을 보지 않고 먼저 판단하라"
같은 directive를 최소 수준으로 넣을 수 있다.

### 3. 표시/가시성 추가

가능하면 UI 또는 progress metadata에서:
- 어떤 participant가 blind verifier인지

최소 수준으로 드러나야 한다.

### 4. 기본 동작은 유지

blind verifier는 기본값이 아니라:
- opt-in
- 또는 명시적 participant 역할

로 두는 것이 안전하다.

## 이번 단계에서 하지 않을 것

- lead decomposition
- role-based output cap
- RT 전체 orchestration 재설계
- verifier scoring/judge 시스템 도입

## 구현 원칙

- 기존 RT semantics를 최대한 유지한다
- verifier isolation만 최소 확장으로 추가한다
- UI에 복잡한 새 개념을 많이 드러내지 않는다
- participant 순서에 의존하는 의도 대신, 명시적 blind rule을 코드로 보장한다

## 성공 기준

- blind verifier로 지정된 participant는 다른 participant 응답을 보지 않는다
- Sequential/Deliberative 기본 동작은 그대로 유지된다
- verifier isolation이 코드 레벨에서 보장된다
- progress/trace에서 blind verifier 여부를 최소한 식별할 수 있다

## 후속

이 단계 다음은:

1. role-based output cap
2. verifier scoring / disagreement surfacing
3. lead decomposition 별도 milestone

순으로 이어진다.
