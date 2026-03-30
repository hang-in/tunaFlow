# Compressed Memory Operational Polish Plan

상태: 중요 / 제안
작성: 2026-03-30

## 배경

Long-Term Memory Phase 1으로 `conversation_memory`와 `compressed-memory` ContextPack source가 들어왔다.

현재 의미:
- 오래된 대화를 구조화 요약으로 유지할 수 있다
- recent window를 무작정 늘리지 않고 continuity를 보조할 수 있다

하지만 아직 이 상태는 “구현됨”에 가깝고, 운영 가능한 memory layer로 보기엔 부족하다.

## 현재 남은 문제

### 1. 생성 상태가 보이지 않는다

- 압축이 성공했는지
- 아직 생성되지 않았는지
- 실패했는지
- 오래되어 재생성이 필요한지

를 사용자가 알기 어렵다.

### 2. provenance가 약하다

- 몇 개 메시지를 기반으로 만들었는지
- 언제 생성되었는지
- 어느 conversation/branch 기준인지

를 더 명확히 보여줄 필요가 있다.

### 3. 재압축 정책이 검증되지 않았다

- 12+ 메시지
- 마지막 압축 이후 6+ 새 메시지

라는 현재 규칙이 실제로 적절한지 검증이 부족하다.

### 4. 품질 검증이 약하다

- compressed memory가 실제 continuity를 얼마나 보존하는지
- artifact/plan과 충돌하지 않는지
- 노이즈를 늘리지 않는지

를 최소한의 시나리오로 확인해야 한다.

## 목표

compressed memory를 “있다” 수준에서 “운영 가능한 long-term memory 보조층” 수준으로 끌어올린다.

## 이번 단계에서 할 것

### 1. 상태 가시화

최소한 아래 상태를 보여준다.

- not_generated
- fresh
- stale
- failed

표시 위치는:
- Runtime
- Trace
- 또는 conversation memory 관련 가벼운 진단 표면

### 2. provenance 보강

최소 메타:
- created_at / updated_at
- source_count
- conversation or branch scope

가능하면:
- 최근 압축 시점 대비 새 메시지 수

### 3. 재압축 정책 보강

- 현재 규칙을 그대로 유지하더라도
- 언제 stale로 볼지, 언제 재생성할지 더 명확히 한다

### 4. 최소 검증 시나리오

다음 정도는 확인해야 한다.

1. 12개 이상 대화 후 압축 생성
2. 새 메시지 6개 추가 후 stale → 재압축
3. 다음 응답에서 compressed-memory가 실제 포함되는지
4. artifact/plan과 경쟁하지 않고 continuity 보조층으로 동작하는지

## 비목표

- vector retrieval
- importance scoring 도입
- global memory graph
- memory UI 대형 신설
- generic memory editor

## 성공 기준

- compressed memory의 생성/신선도/실패 여부를 알 수 있다
- source_count와 생성 시점을 보고 이 memory가 얼마나 믿을 만한지 판단할 수 있다
- 재압축 규칙이 최소한 현재 기준에서 설명 가능하다
- long-term memory가 단순 hidden feature가 아니라 운영 가능한 기능으로 한 단계 올라온다

## 후속

이 단계 다음은:

1. structured memory source 강화
2. conversation retrieval

순서가 맞다.
