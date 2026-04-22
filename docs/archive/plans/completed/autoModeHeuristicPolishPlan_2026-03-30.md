# Auto Mode Heuristic Polish Plan

상태: 중요 / 제안
작성: 2026-03-30

## 배경

Mode-Specific Section Heuristics로:
- Lite
- Standard
- Full

은 이제 서로 다른 context assembly profile이 되었다.

하지만 최종 제품 관점에서 기본 모드는 여전히 `Auto`여야 한다.

즉 다음 단계는 사용자가 매번 mode를 직접 고르는 것이 아니라,
현재 요청 상황에 따라 Auto가 어느 profile에 가까워야 하는지 더 설명 가능하게 만드는 것이다.

## 목표

`Auto` mode를:
- 단순 default 라벨이 아니라
- 현재 작업 맥락을 보고 Lite / Standard / Full 중 하나에 가까운 profile을 선택하는 정책

으로 다듬는다.

핵심은:
- 왜 Auto가 이번 요청에서 Lite였는지
- 왜 Standard/Full로 올라갔는지

를 추론 가능하게 만드는 것이다.

## 왜 필요한가

### 1. 대부분의 사용자는 mode를 직접 조절하지 않아야 한다

tunaFlow는 고급 제어를 제공할 수 있지만,
기본 사용 흐름은:
- 프로젝트 선택
- 작업 지시
- 에이전트 실행

이어야 한다.

따라서 Context mode는 기본적으로 `Auto`가 처리하는 편이 맞다.

### 2. 지금은 mode가 좋아졌지만 Auto 기준은 아직 약하다

Lite / Standard / Full 자체는 분명해졌지만,
Auto가 어떤 조건에서 어느 쪽으로 기울어지는지까지는 아직 덜 명확할 수 있다.

### 3. agent-first 기준에서는 “필요할 때만 더 무거운 컨텍스트”가 맞다

짧은 follow-up에는 Lite에 가깝게,
구조화 memory가 중요한 작업에는 Standard/Full에 가깝게,
하도록 자동으로 조정되는 것이 더 효율적이다.

## 이번 단계에서 할 것

### 1. Auto decision input 명확화

Auto가 판단할 입력 신호를 정리한다.

후보:
- 현재 prompt 길이
- explicit handoff source 존재
- plan/findings/artifacts 유무
- retrieval hit 유무
- branch/RT 여부
- recent activity density

### 2. Auto → profile 매핑 규칙 보강

예:
- 짧은 follow-up + 적은 source = Lite 쪽
- 일반 작업 = Standard
- handoff + structured memory + retrieval이 모두 유의미 = Full 쪽

복잡한 ML이 아니라 설명 가능한 규칙 기반이면 충분하다.

### 3. Trace에서 Auto 판단 결과 읽기

가능하면:
- `Auto → Lite`
- `Auto → Standard`
- `Auto → Full`

처럼 이번 run에서 실제로 어떤 profile이 선택됐는지 보이게 한다.

### 4. Runtime surface 최소 표시

StatusBar/Trace에서 과하게 시끄럽지 않게,
Auto가 현재 어떤 profile로 동작했는지 최소한으로 읽게 한다.

## 이번 단계에서 하지 않을 것

- 사용자 mode UI 재설계
- mode 개수 추가
- vector retrieval 도입
- memory source priority 재설계

## 구현 원칙

- Auto는 black box가 아니라 설명 가능한 규칙이어야 한다
- 기본은 Standard에 가깝되, 상황에 따라 Lite/Full로 이동하게 한다
- 사용자가 자주 mode를 손대지 않아도 되게 만드는 것이 목표다

## 성공 기준

- Auto가 Lite/Standard/Full 중 어느 profile을 선택했는지 읽을 수 있다
- 짧은 작업에서는 더 가벼워지고, 복합 작업에서는 더 풍부해진다
- 사용자가 mode를 직접 고르지 않아도 대부분 적절한 결과가 나온다

## 후속

이 단계 다음은:

1. Auto heuristic threshold 추가 보정
2. 실제 dogfood 시나리오에서 mode distribution 확인
3. 그 후 vector/embedding path 재평가

순으로 이어진다.
