# Mode-Specific Section Heuristics Plan

상태: 중요 / 제안
작성: 2026-03-30

## 배경

Top Heavy Section Tuning으로 ContextPack의 큰 budget consumer를 1차로 줄였다.

현재는:
- recent context
- cross-session
- retrieval
- compressed memory

같은 section의 절대 cap을 줄여서 전체 input budget을 크게 낮췄다.

다음 단계는 이 cap을 다시 전역 상수처럼 다루는 것이 아니라,
`Lite / Standard / Full / Auto` mode별로 더 다르게 취급하는 것이다.

## 목표

ContextMode에 따라 section inclusion과 해상도를 더 세밀하게 조정한다.

핵심은:
- Lite는 더 공격적으로 줄이고
- Standard는 현재 균형을 유지하고
- Full은 더 풍부하되 structured memory 우선 원칙은 유지하는 것

이다.

## 왜 필요한가

### 1. 같은 cap을 모든 mode에 쓰면 mode 의미가 약해진다

지금은 mode가 있어도:
- threshold
- 포함 여부
- 일부 cap

정도만 다르고, section별 해상도는 아직 충분히 다르지 않다.

### 2. agent-first 기준에서는 mode가 “비용-품질 프로파일”이어야 한다

Lite / Standard / Full은 단순 라벨이 아니라:
- 어떤 memory를 더 신뢰하는지
- 어떤 section을 excerpt로 줄일지
- 어떤 section을 reference 수준으로만 둘지

가 달라야 한다.

### 3. 다음 vector path 재평가 전에 현재 mode 체계를 더 잘 써야 한다

지금은 새 retrieval/embedding을 넣기보다,
기존 memory source를 mode별로 더 잘 배치하는 편이 ROI가 높다.

## 이번 단계에서 할 것

### 1. mode별 section policy 고정

예시 방향:

#### Lite
- recent 위주
- structured memory 최소 subset
- retrieval 낮은 해상도
- compressed memory는 더 보수적
- rawq는 꼭 필요할 때만

#### Standard
- 현재 균형 유지
- retrieval/compressed memory 보조층 유지

#### Full
- structured memory를 더 넓게 허용
- retrieval / compressed memory가 살아남을 가능성 확대
- 다만 rawq/full snippets는 여전히 해상도 제한 유지

### 2. section 해상도도 mode별 차등화

section마다:
- full
- summary
- excerpt
- reference

중 어느 해상도를 기본으로 쓰는지 mode별로 다르게 둔다.

### 3. Auto mode 규칙 명확화

Auto는 단순 default가 아니라:
- 현재 budget
- context source availability
- recent activity

기준으로 Lite/Standard/Full 중 어느 프로파일에 가까운지 선택하는 쪽으로 정리한다.

### 4. trace에서 mode 차이를 확인 가능하게 유지

가능하면:
- 어떤 mode profile이 적용됐는지
- mode 때문에 어떤 section 해상도가 내려갔는지

를 읽을 수 있게 한다.

## 이번 단계에서 하지 않을 것

- vector retrieval 도입
- 새로운 memory source 추가
- 사용자 per-section 수동 설정 UI
- mode 체계 전면 재설계

## 구현 원칙

- mode는 단순 budget bucket이 아니라 context assembly profile이다
- structured memory 우선 원칙은 모든 mode에서 유지한다
- Lite는 단순히 “적게 넣는 모드”가 아니라 “더 집중된 모드”여야 한다
- Full도 무조건 다 넣는 모드가 아니라, 더 풍부하지만 여전히 agent-friendly해야 한다

## 성공 기준

- Lite / Standard / Full 차이가 section 해상도 수준에서도 분명해진다
- Lite에서 불필요한 section 부피가 더 줄어든다
- Full에서도 과한 noise 없이 더 많은 관련 기억이 살아남는다
- Trace에서 mode별 결과 차이를 읽을 수 있다

## 후속

이 단계 다음은:

1. Auto mode heuristic 보정
2. 필요 시 retrieval threshold 재조정
3. 그 후 vector/embedding path 재평가

순으로 이어진다.
